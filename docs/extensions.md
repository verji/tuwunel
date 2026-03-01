# Extension System

Tuwunel supports a plugin-style extension system that allows third-party code to
intercept and react to Matrix events without modifying core server code. The
design is inspired by Synapse's Module API and uses compile-time (link-time)
registration so that extensions are discovered automatically when linked into
the server binary.

## Architecture Overview

The extension system is built on three pillars:

1. **Dependency inversion** -- Extension traits (`EventInterceptor`,
   `ExtensionApi`) are defined in `tuwunel_core`. The concrete implementation
   of `ExtensionApi` lives in `tuwunel_service`. Extensions depend only on
   `tuwunel_core`, never on service internals.

2. **Link-time registration via `inventory`** -- Extensions register themselves
   using `inventory::submit!`. When the server binary links in an extension
   crate, its `ExtensionEntry` is automatically collected at program startup.
   No manual registration code or config file entries are needed.

3. **`tokio::task_local!` for request context** -- HTTP handlers set a
   `HookContext` (containing sender, room, client IP, device ID) into a
   task-local variable. Hook sites in the event pipeline read this context
   without requiring changes to internal function signatures.

```
                    +-----------------+
                    |  tuwunel_core   |
                    |  (traits only)  |
                    +--------+--------+
                             |
              +--------------+--------------+
              |                             |
   +----------v----------+     +-----------v-----------+
   |  tuwunel_service     |     |  hello-extension      |
   |  (ExtensionApiImpl)  |     |  (EventInterceptor)   |
   +-----------+----------+     +-----------+-----------+
               |                            |
               +------------+---------------+
                            |
                 +----------v----------+
                 |  tuwunel-custom     |
                 |  (server binary)    |
                 +---------------------+
```

## Core Types

All public types live in `tuwunel_core::extension` (`src/core/extension.rs`).

### `EventOrigin`

Describes where an event came from:

```rust
pub enum EventOrigin {
    /// Event sent by a local user via a client API.
    Local {
        device_id: Option<OwnedDeviceId>,
        client_ip: Option<IpAddr>,
        is_appservice: bool,
    },

    /// Event received from a remote server via federation.
    Federation { origin_server: OwnedServerName },

    /// Event generated internally (admin commands, migrations, etc.).
    Internal,
}
```

When `HOOK_CTX` is not set (e.g. events from admin commands, federation, or
internal services), the origin defaults to `Internal`.

### `HookContext`

Context passed to every hook invocation:

```rust
pub struct HookContext {
    pub sender: OwnedUserId,
    pub room_id: OwnedRoomId,
    pub origin: EventOrigin,
}
```

Convenience methods:

| Method | Returns | Description |
|--------|---------|-------------|
| `device_id()` | `Option<&OwnedDeviceId>` | Device ID if `Local` origin |
| `client_ip()` | `Option<IpAddr>` | Client IP if `Local` origin |
| `is_local()` | `bool` | `true` when origin is `Local` |
| `is_federated()` | `bool` | `true` when origin is `Federation` |

### `EventDecision`

Returned by before-hooks to control event flow:

```rust
pub enum EventDecision {
    Allow,
    Block(String),  // reason shown as 403 Forbidden
}
```

### `EventInterceptor` (trait)

The main trait extensions implement:

```rust
#[async_trait]
pub trait EventInterceptor: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &str;

    /// Called before a locally-originated event is signed and persisted.
    /// Can inspect/modify the PduBuilder or block the event entirely.
    async fn before_local_event(
        &self,
        ctx: &HookContext,
        builder: &mut PduBuilder,
    ) -> Result<EventDecision>;

    /// Called after an event has been persisted and its side-effects applied.
    /// Errors are logged but do not fail the event.
    async fn after_event(
        &self,
        ctx: &HookContext,
        event: &PduEvent,
    ) -> Result;
}
```

### `ExtensionApi` (trait)

Read-only API available to extensions for querying server state:

```rust
#[async_trait]
pub trait ExtensionApi: Send + Sync {
    /// Get a state event from a room. Returns None if not found.
    async fn room_state_get(
        &self,
        room_id: &RoomId,
        event_type: &StateEventType,
        state_key: &str,
    ) -> Option<PduEvent>;

    /// Check if a user is a server admin.
    async fn user_is_admin(&self, user_id: &UserId) -> bool;

    /// Get all joined members of a room.
    async fn get_room_members(&self, room_id: &RoomId) -> Vec<OwnedUserId>;

    /// Get this server's name.
    fn server_name(&self) -> &ServerName;

    /// Check if a user belongs to this server.
    fn user_is_local(&self, user_id: &UserId) -> bool;
}
```

### `ExtensionEntry`

Registration record collected by `inventory` at link time:

```rust
pub struct ExtensionEntry {
    pub name: &'static str,
    pub create: fn(
        &serde_json::Value,        // configuration (currently empty object)
        Arc<dyn ExtensionApi>,     // server API handle
    ) -> Result<Box<dyn EventInterceptor>>,
}
```

## Hook Sites

### Before-hook: `build_and_append_pdu`

**File:** `src/service/rooms/timeline/build.rs`

Called at the top of `build_and_append_pdu`, **before** the event is hashed and
signed. This is the only point where the `PduBuilder` can still be inspected or
modified.

- Interceptors are called **in registration order**.
- If any interceptor returns `EventDecision::Block(reason)`, the event is
  rejected with HTTP 403 Forbidden and the reason string.
- If the `HOOK_CTX` task-local is not set, a fallback context with
  `EventOrigin::Internal` is used.

### After-hook: `append_pdu`

**File:** `src/service/rooms/timeline/append.rs`

Called after `append_pdu_effects()` has completed (search indexing, membership
updates, relation tracking, etc.) and **before** appservice notification.

- Interceptors are called **in registration order**.
- Errors are **logged but not propagated** -- an extension failure cannot break
  event persistence.
- Receives the final `PduEvent` (immutable, already persisted).

### Context population: `send_message_event_route`

**File:** `src/api/client/send.rs`

The `send_message_event_route` handler is the only API endpoint currently
instrumented. It extracts the client IP via `InsecureClientIp`, constructs a
`HookContext` with `EventOrigin::Local`, and wraps the `build_and_append_pdu`
call in `HOOK_CTX.scope(...)`.

All other code paths (federation, admin commands, internal events) do not set
`HOOK_CTX`. Hook sites fall back to `EventOrigin::Internal` when the task-local
is absent.

## Extension Lifecycle

1. **Link time** -- When the server binary links an extension crate, the
   `inventory::submit!` block registers an `ExtensionEntry` into a global
   collection.

2. **Services build** -- During `Services::build()`, after all core services
   are constructed and the `Arc<Services>` is available:
   - An `ExtensionApiImpl` is created, backed by real services.
   - `inventory::iter::<ExtensionEntry>` is iterated.
   - Each entry's `create` function is called with a config value and the API
     handle.
   - Successfully created interceptors are stored in
     `Services::interceptors` (an `OnceLock<Vec<Box<dyn EventInterceptor>>>`).

3. **Runtime** -- On each event, hook sites check `interceptors.get()` and
   call each interceptor's methods.

4. **Shutdown** -- Interceptors are dropped when `Services` is dropped.

## Writing an Extension

### 1. Create the crate

```
extensions/my-extension/
  Cargo.toml
  src/lib.rs
```

**Cargo.toml:**

```toml
[package]
name = "my-extension"
version = "0.1.0"
edition = "2024"

[dependencies]
tuwunel-core.workspace = true
async-trait.workspace = true
inventory.workspace = true
serde_json.workspace = true
tracing.workspace = true
ruma.workspace = true

[lints]
workspace = true
```

The crate must live inside the tuwunel workspace (add `"extensions/*"` to
`[workspace] members` in the root `Cargo.toml` if not already present).

### 2. Implement `EventInterceptor`

```rust
use std::sync::Arc;
use async_trait::async_trait;
use tuwunel_core::{
    Result,
    extension::{
        EventDecision, EventInterceptor, ExtensionApi,
        ExtensionEntry, HookContext,
    },
    matrix::pdu::{PduBuilder, PduEvent},
};

struct MyExtension {
    api: Arc<dyn ExtensionApi>,
}

#[async_trait]
impl EventInterceptor for MyExtension {
    fn name(&self) -> &str { "my-extension" }

    async fn before_local_event(
        &self,
        ctx: &HookContext,
        builder: &mut PduBuilder,
    ) -> Result<EventDecision> {
        // Inspect ctx.sender, ctx.room_id, ctx.client_ip(), etc.
        // Inspect or mutate builder.content, builder.event_type, etc.
        // Return EventDecision::Block("reason".into()) to reject.
        Ok(EventDecision::Allow)
    }

    async fn after_event(
        &self,
        ctx: &HookContext,
        event: &PduEvent,
    ) -> Result {
        // Read event.event_id, event.sender, event.room_id, etc.
        // Use self.api to query server state.
        Ok(())
    }
}
```

### 3. Register with `inventory`

```rust
inventory::submit! {
    ExtensionEntry {
        name: "my-extension",
        create: |_config, api| {
            Ok(Box::new(MyExtension { api }))
        },
    }
}
```

The `create` function receives:
- `config: &serde_json::Value` -- currently an empty JSON object; reserved for
  future per-extension configuration.
- `api: Arc<dyn ExtensionApi>` -- handle for querying server state.

### 4. Link into a custom server binary

Create a custom binary that depends on both `tuwunel` and your extension:

**server/Cargo.toml:**

```toml
[package]
name = "tuwunel-custom"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "tuwunel-custom"
path = "src/main.rs"

[dependencies]
tuwunel = { path = "../src/main", default-features = false }
tuwunel-core = { path = "../src/core", package = "tuwunel_core" }
my-extension = { path = "../extensions/my-extension" }
clap.workspace = true
log.workspace = true
tokio.workspace = true
tracing.workspace = true
webpki-root-certs.workspace = true

[features]
default = [
    "brotli_compression",
    "element_hacks",
    "gzip_compression",
    "media_thumbnail",
    "release_max_log_level",
    "url_preview",
    "zstd_compression",
]
# ... feature forwarding to tuwunel/...
```

**server/src/main.rs:**

```rust
extern crate my_extension; // force the linker to include the extension

use std::sync::atomic::Ordering;
use tuwunel_core::Result;

fn main() -> Result {
    let args = tuwunel::args::parse();
    let runtime = tuwunel::runtime::new(Some(&args))?;
    let server = tuwunel::Server::new(Some(&args), Some(runtime.handle()))?;
    tuwunel::exec(&server, runtime)?;

    #[cfg(unix)]
    if server.server.restarting.load(Ordering::Acquire) {
        tuwunel::restart::restart();
    }

    Ok(())
}
```

The `extern crate` is essential -- without it, the Rust linker may optimize
away the extension crate since nothing directly calls into it. The
`inventory::submit!` block uses a linker trick (a `#[used]` static) that only
works if the crate is actually linked.

### 5. Build and run

```bash
cargo build -p tuwunel-custom
./target/debug/tuwunel-custom --config tuwunel.toml
```

Expected startup logs:

```
INFO Loaded extension: my-extension
INFO Loaded 1 extension(s)
INFO Services startup complete.
```

## Example: hello-extension

The repository includes a sample extension at
`extensions/hello-extension/` that demonstrates all features:

**Before-hook behavior:**
- Logs the sender, room, client IP, and device ID for every locally-sent event.
- If the message body contains the string `EXTENSION_BLOCK`, the event is
  blocked with 403 Forbidden.
- Server admins bypass the block.

**After-hook behavior:**
- Logs the persisted event ID, sender, room ID, and current member count.

### Testing the hello-extension

1. Send a normal message via any Matrix client -- logs appear, message goes
   through.
2. Send a message containing `EXTENSION_BLOCK` -- blocked with 403 (unless the
   sender is an admin).
3. After any event persists, logs show the event ID and room member count.
4. Events from the `send_message_event` endpoint include client IP and device
   ID in the logs. Events from other paths show `origin: Internal`.

## What Extensions Can Do

| Capability | Before-hook | After-hook |
|---|---|---|
| Inspect event metadata | sender, room, origin, IP, device | sender, room, origin, event_id |
| Read event content | `builder.content` (raw JSON) | `event` (full PduEvent) |
| Modify event content | `builder.content = ...` | No (event is persisted) |
| Block event | `EventDecision::Block(reason)` | No (fire-and-forget) |
| Query room state | `api.room_state_get(...)` | `api.room_state_get(...)` |
| Check admin status | `api.user_is_admin(...)` | `api.user_is_admin(...)` |
| List room members | `api.get_room_members(...)` | `api.get_room_members(...)` |
| Check user locality | `api.user_is_local(...)` | `api.user_is_local(...)` |
| Get server name | `api.server_name()` | `api.server_name()` |

## Design Decisions

### Why `inventory` instead of dynamic loading?

`inventory` provides zero-touch, link-time registration with no unsafe code in
extensions. Dynamic loading (`libloading`/`dlopen`) requires ABI stability
guarantees, careful symbol management, and `unsafe` blocks. Since extensions are
compiled alongside the server, link-time registration is simpler and safer.

### Why `task_local!` instead of changing function signatures?

The `build_and_append_pdu` function is called from dozens of places (admin
commands, federation, appservices, client API). Adding a `HookContext` parameter
to every call site would be a massive, invasive change. `task_local!` lets
HTTP handlers attach context that flows naturally through async call chains,
with a graceful fallback (`EventOrigin::Internal`) when no context is set.

### Why `OnceLock` instead of a field initialized in the constructor?

The `Services` struct is constructed as an `Arc<Services>`, but extensions need
an `Arc<Services>` reference (via `ExtensionApiImpl`) to query server state.
This is a circular dependency: `Services` contains interceptors, but
interceptors need `Services`. `OnceLock` breaks the cycle by allowing
interceptors to be initialized *after* the `Arc<Services>` is created but
*before* `start()` is called.

### Why are after-hooks fire-and-forget?

After-hooks run after the event is persisted. Failing the event at this point
would create an inconsistent state (event in the database but an error returned
to the client). Errors in after-hooks are logged and ignored to maintain
consistency.

## Current Limitations

- **Only `send_message_event_route` sets `HOOK_CTX`** -- Other endpoints
  (state events, redactions, federation) use the `Internal` fallback. Adding
  `HOOK_CTX` to more endpoints is straightforward.
- **No per-extension configuration** -- The `config` parameter passed to
  `create` is currently an empty JSON object. A future iteration could load
  per-extension config from the server's TOML file.
- **Read-only `ExtensionApi`** -- Extensions cannot send events, modify room
  state, or perform write operations. The API surface can be expanded as
  needed.
- **No priority/ordering control** -- Interceptors run in the order
  `inventory` collects them (link order). There is no explicit priority system.
- **In-workspace only** -- Extension crates must currently live inside the
  tuwunel workspace. A future production layout would use an outer workspace
  with a sync script.

## File Reference

| File | Role |
|------|------|
| `src/core/extension.rs` | Public traits and types |
| `src/service/extension_api.rs` | `ExtensionApiImpl` backed by real services |
| `src/service/services.rs` | `OnceLock<Vec<...>>` field and initialization loop |
| `src/service/rooms/timeline/build.rs` | Before-hook call site |
| `src/service/rooms/timeline/append.rs` | After-hook call site |
| `src/api/client/send.rs` | `HOOK_CTX` population from HTTP request |
| `extensions/hello-extension/` | Sample extension |
| `server/` | Custom server binary that links extensions |
