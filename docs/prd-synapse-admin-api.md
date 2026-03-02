# PRD: Synapse-Compatible Admin API for Tuwunel

**Author:** Thor Arne Johansen
**Date:** 2026-03-02
**Branch:** `taj/tuwunel-admin-api`
**Status:** Draft

---

## 1. Problem Statement

Verji's operational tooling (`verji-itops-matrix`, `verji-linkonboarding`, `verji-itops`)
depends on the Synapse Admin API for user management, room inspection, and room lifecycle
operations. Tuwunel does not expose any HTTP admin endpoints — all administration is
performed through Matrix room commands — making it incompatible with Verji's existing SDK
and automation infrastructure.

Migrating to tuwunel requires closing this gap without modifying Verji's SDK
(`Verji.ItOps.Matrix.Sdk`).

---

## 2. Goal

Implement a **Synapse-compatible HTTP admin API** in tuwunel covering the 9 endpoints
that Verji actively uses in production. The endpoints must be wire-compatible with
Synapse's request/response JSON schemas so the existing `IAdminApiRaw` Refit interface
works unchanged.

### Non-goals

- Full Synapse admin API parity (only the 9 used endpoints)
- Admin API for federation management, media management, or server config
- The 6 dead-code endpoints in the SDK (see Appendix A)
- The custom `MakeRoomAdmin` Verji endpoint (deferred)

---

## 3. Background: Actual Usage Audit

An audit of four Verji codebases identified which of the SDK's 15 admin endpoints have
production callers:

| Endpoint | itops-matrix | linkonboarding | VmxAccount | verji-itops |
|----------|:---:|:---:|:---:|:---:|
| GetAccount | 3 callers | 1 caller | — | 1 caller |
| PutAccount | 3 callers | — | — | — |
| ListAccounts | 1 caller | — | — | — |
| GetUserDevices | 1 caller (via V2 Login SDK) | — | — | — |
| ListRooms | — | — | — | 2 callers |
| GetRoomDetails | 1 caller | — | — | — |
| GetRoomMembers | 2 callers | — | — | 1 caller |
| GetRoomState | 4 callers | — | — | 1 caller |
| DeleteRoom | 1 caller (deprecated) | — | — | — |

VmxAccount has no direct admin API usage — it uses Application Service masquerading
via the Client-Server API (`POST /_matrix/client/v3/login` with type
`m.login.application_service`).

The V2 Login SDK's `LoginImpersonatedUserWithExistingDevice` calls `GetUserDevices`
before logging in to reuse an existing device. This is the only admin API call in the
impersonation flow.

---

## 4. Requirements

### 4.1 Endpoints

All endpoints require Bearer token authentication and the authenticated user must be a
server administrator.

#### 4.1.1 GET `/_synapse/admin/v2/users/{userId}` — Query User Account

Returns account details for a single user.

**Response body:**
```json
{
  "name": "@user:example.com",
  "displayname": "User",
  "threepids": [
    { "medium": "email", "address": "user@example.com", "added_at": 1586458409743, "validated_at": 1586458409743 }
  ],
  "avatar_url": "mxc://example.com/abcde",
  "is_guest": 0,
  "admin": 0,
  "deactivated": 0,
  "erased": false,
  "shadow_banned": 0,
  "creation_ts": 1560432506,
  "appservice_id": null,
  "user_type": null,
  "locked": false
}
```

**Verji usage:** Check if user exists, check deactivation status, read 3PIDs (email).
The SDK tries V2 first, falls back to V1. Tuwunel only needs V2.

**Fields that must be populated:**
- `name`, `displayname`, `avatar_url` — from user profile service
- `deactivated` — from `services.users.is_deactivated()`
- `threepids` — from third-party ID storage (if available)
- `admin` — from `services.admin.user_is_admin()`
- `is_guest` — from user account type
- `creation_ts` — from user creation timestamp

**Fields that can be stubbed initially:** `erased`, `shadow_banned`, `consent_*`,
`external_ids`, `user_type`, `locked`, `last_seen_ts`, `appservice_id`.

**Service layer support:** `services.users.exists()`, `services.users.is_deactivated()`,
`services.users.displayname()`, `services.users.avatar_url()`,
`services.admin.user_is_admin()`.

---

#### 4.1.2 PUT `/_synapse/admin/v2/users/{userId}` — Modify Account

Updates user account fields. All request body fields are optional.

**Request body:**
```json
{
  "password": "new_password",
  "displayname": "New Name",
  "avatar_url": "mxc://example.com/new",
  "threepids": [{ "medium": "email", "address": "new@example.com" }],
  "admin": false,
  "deactivated": true
}
```

**Response:** `200 OK` with `{}` (user modified) or `201 Created` with `{}` (user created).

**Verji usage:**
- `UpdateSynapseEmailCommandHandler` — updates `threepids` (email)
- `VerjiAccountSdk` / `VerjiSdkV2` — sets `deactivated: true` for account deactivation

**Minimum viable fields:** `password`, `displayname`, `avatar_url`, `threepids`,
`admin`, `deactivated`. Other fields (`user_type`, `locked`, `external_ids`) can
return 400 if set.

**Service layer support:** `services.users.set_password()`,
`services.users.set_displayname()`, `services.users.set_avatar_url()`,
`services.users.deactivate_account()`.

---

#### 4.1.3 GET `/_synapse/admin/v2/users` — List Accounts

Paginated user listing with optional filters.

**Query parameters:**
| Parameter | Type | Default | Notes |
|---|---|---|---|
| `from` | integer | 0 | Pagination offset |
| `limit` | integer | 100 | Max per page |
| `user_id` | string | — | Filter by user ID substring |
| `name` | string | — | Filter by localpart or displayname |
| `guests` | string | `"true"` | Exclude guests if `"false"` |
| `deactivated` | string | `"false"` | Include deactivated if `"true"` |
| `order_by` | string | `"name"` | Sort field |
| `dir` | string | `"f"` | `"f"` or `"b"` |

**Response:**
```json
{
  "users": [
    { "name": "@user:example.com", "displayname": "User", "admin": 0, "deactivated": 0, "is_guest": 0, "avatar_url": null, "creation_ts": 1560432668000, "erased": false, "shadow_banned": 0, "user_type": null, "locked": false }
  ],
  "next_token": "100",
  "total": 200
}
```

**Verji usage:** `LogoutMatchingDevicesCommandHandler` paginates all users with `user_id`
filter.

**Service layer support:** `services.users.stream()` / `services.users.list_local_users()`.
Pagination and filtering must be built on top — tuwunel's current user stream has no
offset/limit or substring filter. Two approaches:
1. Collect into Vec and slice (simple, fine for small deployments)
2. Add database-level pagination (better for scale)

---

#### 4.1.4 GET `/_synapse/admin/v2/users/{userId}/devices` — List User Devices

**Response:**
```json
{
  "devices": [
    { "device_id": "QBUAZIFURK", "display_name": "android", "last_seen_ip": "1.2.3.4", "last_seen_ts": 1474491775024, "user_id": "@user:example.com" }
  ],
  "total": 2
}
```

**Verji usage:** Called by `LoginImpersonatedUserWithExistingDevice` in the V2 Login SDK
to find an existing device before logging in via Application Service masquerading.

**Service layer support:** `services.users.all_devices_metadata(user_id)` returns a
`Stream<Item = Device>` with `device_id` and `display_name`. Last-seen IP/timestamp
may require additional lookups depending on what `Device` struct contains.

---

#### 4.1.5 GET `/_synapse/admin/v1/rooms` — List Rooms

**Query parameters:**
| Parameter | Type | Default |
|---|---|---|
| `from` | integer | 0 |
| `limit` | integer | 100 |
| `order_by` | string | — |
| `dir` | string | `"f"` |
| `search_term` | string | — |

**Response:**
```json
{
  "rooms": [
    { "room_id": "!abc:example.com", "name": "Room", "canonical_alias": "#room:example.com", "joined_members": 10, "joined_local_members": 5, "version": "10", "creator": "@admin:example.com", "encryption": null, "federatable": true, "public": true, "join_rules": "invite", "guest_access": null, "history_visibility": "shared", "state_events": 42, "room_type": null }
  ],
  "offset": 0,
  "total_rooms": 150,
  "next_batch": "100"
}
```

**Verji usage:** `CreateHierarchyAnalysisCommandHandler` paginates all rooms.
`ItOpsConnector` fetches with `limit=1` to get `total_rooms` count.

**Service layer support:** `services.rooms.metadata.iter_ids()` streams all room IDs.
Per-room metadata requires calls to `state_accessor` (name, alias, join_rules, etc.)
and `state_cache` (member counts). Pagination must be built on top.

---

#### 4.1.6 GET `/_synapse/admin/v1/rooms/{roomId}` — Room Details

Same fields as List Rooms, plus `topic`, `avatar`, `joined_local_devices`, `forgotten`.

**Verji usage:** `VerjiSdkV2 RoomMetadata` fetches room info.

**Service layer support:** Same as List Rooms, applied to a single room. Additional:
`services.rooms.state_accessor.get_name()`, `.get_avatar()`, topic from state.

---

#### 4.1.7 GET `/_synapse/admin/v1/rooms/{roomId}/members` — Room Members

**Response:**
```json
{
  "members": ["@foo:example.com", "@bar:example.com"],
  "total": 2
}
```

**Verji usage:**
- `ChangeRoomNameCommandHandler` — find a member to impersonate for room rename
- `VerjiLoginSdk` — find any member for impersonation
- `CreateHierarchyAnalysisCommandHandler` — enumerate all members for hierarchy analysis

**Service layer support:** `services.rooms.state_cache.room_members(room_id)` returns
`Stream<Item = &UserId>`. Collect and count.

---

#### 4.1.8 GET `/_synapse/admin/v1/rooms/{roomId}/state` — Room State

**Response:**
```json
{
  "state": [
    { "type": "m.room.create", "state_key": "", "content": {}, "sender": "@admin:example.com", "event_id": "$abc", "origin_server_ts": 1560432506000, "room_id": "!room:example.com" }
  ]
}
```

**Verji usage (heaviest endpoint — 5 call sites):**
- Check power levels (`m.room.power_levels`)
- Check if vsys user is in room
- Detect `is_direct` flag on member events
- Read tenant info custom state event
- Read parent space IDs

**Service layer support:**
`services.rooms.state_accessor.room_state_full(room_id)` returns a stream of
`(Arc<(StateEventType, Arc<StateKey>)>, Arc<Pdu>)`. Each `Pdu` can be serialized
to the standard Matrix event JSON format.

---

#### 4.1.9 DELETE `/_synapse/admin/v2/rooms/{roomId}` — Delete Room

**Request body:**
```json
{
  "new_room_user_id": "@admin:example.com",
  "room_name": "Content Violation Notification",
  "message": "This room has been removed",
  "block": false,
  "purge": true,
  "force_purge": false
}
```

**Response:** `200 OK` with `{ "delete_id": "..." }`

In Synapse, deletion is asynchronous. Tuwunel can implement it synchronously and return
a synthetic `delete_id`. The status-polling endpoints (`GetDeleteRoomStatusByRoomId`,
`GetDeleteRoomStatusByDeleteId`) are not used by Verji and are out of scope.

**Verji usage:** `VerjiSdkV2 SystemManagedSpaces` — delete personal space. Marked
deprecated with TODO for removal. **Lowest priority.**

**Service layer support:** Room deletion logic exists in the admin command handler.
Needs to be refactored into a callable service function.

---

### 4.2 Authentication & Authorization

All endpoints require:
1. `Authorization: Bearer <access_token>` header
2. The token must resolve to a valid user session
3. The user must be a server administrator (`services.admin.user_is_admin()`)

Return `401 Unauthorized` if no valid token, `403 Forbidden` if not admin.

This matches Synapse's behavior. Tuwunel's existing auth pipeline extracts the sender
from Bearer tokens. The admin check is a single async call.

### 4.3 Error Responses

Follow Synapse's error format:
```json
{ "errcode": "M_FORBIDDEN", "error": "You are not a server admin" }
{ "errcode": "M_NOT_FOUND", "error": "User not found" }
```

Standard Matrix error codes: `M_FORBIDDEN`, `M_NOT_FOUND`, `M_UNKNOWN`,
`M_INVALID_PARAM`, `M_MISSING_PARAM`.

---

## 5. Architecture

### 5.1 Design Decision: Separate Workspace Crate

The admin API is implemented as a **separate workspace crate** (`tuwunel_synapse_admin`)
rather than a module inside `tuwunel_api`. This maximizes isolation from upstream tuwunel
and minimizes merge conflicts when rebasing on upstream updates.

**Rationale:** Upstream tuwunel is unlikely to accept Synapse admin API compatibility
as a core feature. By keeping the implementation in its own crate, we ensure:

- **Merge-friendly fork:** Only ~5 lines across 3 upstream files are modified. The entire
  admin API lives in a directory upstream doesn't have, so it can never conflict.
- **Feature-gated:** A Cargo feature (`synapse_admin`) controls compilation. When disabled,
  zero admin API code is compiled.
- **Runtime-toggled:** A config flag (`allow_synapse_admin_api`) controls whether routes
  are registered, even when compiled in.
- **Clean separation:** The crate has its own `Cargo.toml`, dependencies, and test suite.

### 5.2 Crate Layout

```
src/synapse_admin/               # NEW crate — auto-joins workspace via members = ["src/*"]
├── Cargo.toml                   # depends on tuwunel_core, tuwunel_service
├── mod.rs                       # pub fn routes(Router) -> Router
├── auth.rs                      # AdminUser axum extractor
├── error.rs                     # Synapse-compatible error responses
├── user/
│   ├── mod.rs
│   ├── get.rs                   # GET /v2/users/{userId}
│   ├── put.rs                   # PUT /v2/users/{userId}
│   ├── list.rs                  # GET /v2/users
│   └── devices.rs               # GET /v2/users/{userId}/devices
└── room/
    ├── mod.rs
    ├── list.rs                  # GET /v1/rooms
    ├── details.rs               # GET /v1/rooms/{roomId}
    ├── members.rs               # GET /v1/rooms/{roomId}/members
    ├── state.rs                 # GET /v1/rooms/{roomId}/state
    └── delete.rs                # DELETE /v2/rooms/{roomId}
```

### 5.3 Upstream Touchpoints (minimized)

Only 3 upstream files are modified, totaling ~5 lines of diff:

**1. `src/main/Cargo.toml`** — optional dependency + feature
```toml
[features]
synapse_admin = ["dep:tuwunel-synapse-admin"]

[dependencies]
tuwunel-synapse-admin = { path = "../synapse_admin", optional = true }
```

**2. Root `Cargo.toml`** — feature propagation
```toml
[features]
synapse_admin = ["tuwunel/synapse_admin"]
```

**3. `src/main/server.rs`** (or equivalent startup path) — conditional route mount
```rust
#[cfg(feature = "synapse_admin")]
if services.server.config.allow_synapse_admin_api {
    router = tuwunel_synapse_admin::routes(router, &services);
}
```

The config field `allow_synapse_admin_api` is read by our crate from the same TOML file
via a separate Figment extract, avoiding changes to `src/core/config/mod.rs`.

### 5.4 Rebase Model

```
upstream main:  A ── B ── C ── D ── E       (upstream evolves)
                 \
our fork:         X ── Y                     (our commits)
```

- **Commit X** = "Add `tuwunel_synapse_admin` crate" — entirely new directory, never conflicts
- **Commit Y** = "Wire admin API into main" — ~5 lines in 3 files, minimal conflict surface

On every rebase, only commit Y can conflict, and only if upstream modifies the exact
lines we touch in those 3 files.

### 5.5 Route Registration

The Synapse admin API is not part of the Matrix spec and has no Ruma types. The crate
uses plain **axum routes** with serde structs for request/response serialization:

```rust
// src/synapse_admin/mod.rs
pub fn routes(router: Router<State>, services: &Services) -> Router<State> {
    router
        .route("/_synapse/admin/v2/users/:user_id", get(user::get_account))
        .route("/_synapse/admin/v2/users/:user_id", put(user::put_account))
        .route("/_synapse/admin/v2/users", get(user::list_accounts))
        .route("/_synapse/admin/v2/users/:user_id/devices", get(user::get_devices))
        .route("/_synapse/admin/v1/rooms", get(room::list_rooms))
        .route("/_synapse/admin/v1/rooms/:room_id", get(room::get_details))
        .route("/_synapse/admin/v1/rooms/:room_id/members", get(room::get_members))
        .route("/_synapse/admin/v1/rooms/:room_id/state", get(room::get_state))
        .route("/_synapse/admin/v2/rooms/:room_id", delete(room::delete_room))
}
```

### 5.6 Admin Auth Guard

A reusable axum `FromRequestParts` extractor within the crate:

```rust
/// Extracts the authenticated admin user from the request.
/// Returns 401 if no valid token, 403 if not admin.
pub(crate) struct AdminUser {
    pub user_id: OwnedUserId,
    pub device_id: OwnedDeviceId,
}
```

Implementation:
1. Reads `Authorization: Bearer ...` header
2. Looks up the token via `services.users` → `(user_id, device_id)`
3. Checks `services.admin.user_is_admin(&user_id)`
4. Returns `AdminUser` or rejects with 401/403

### 5.7 Config

The crate reads its own config section from the same tuwunel TOML file using a
separate Figment extract, keeping `src/core/config/mod.rs` untouched:

```toml
# tuwunel.toml
allow_synapse_admin_api = true   # default: false
```

```rust
// src/synapse_admin/config.rs
#[derive(Deserialize)]
pub struct SynapseAdminConfig {
    #[serde(default)]
    pub allow_synapse_admin_api: bool,
}
```

---

## 6. Implementation Plan

### Phase 1 — Foundation + User Endpoints

**Unblocks:** linkonboarding, verji-itops, itops-matrix

1. Create `src/synapse_admin/` crate with `Cargo.toml`
2. Add `synapse_admin` feature flag to root + main crate
3. Implement `AdminUser` auth extractor
4. Implement config extraction (`allow_synapse_admin_api`)
5. Implement GET `/_synapse/admin/v2/users/{userId}` (GetAccount)
6. Implement GET `/_synapse/admin/v2/users` (ListAccounts)
7. Implement PUT `/_synapse/admin/v2/users/{userId}` (PutAccount)
8. Implement GET `/_synapse/admin/v2/users/{userId}/devices` (GetUserDevices)
9. Wire routes into `src/main/server.rs` behind feature + config guard

### Phase 2 — Room Read Endpoints

**Unblocks:** hierarchy analysis in verji-itops

10. Implement GET `/_synapse/admin/v1/rooms` (ListRooms)
11. Implement GET `/_synapse/admin/v1/rooms/{roomId}` (GetRoomDetails)
12. Implement GET `/_synapse/admin/v1/rooms/{roomId}/members` (GetRoomMembers)
13. Implement GET `/_synapse/admin/v1/rooms/{roomId}/state` (GetRoomState)

### Phase 3 — Room Mutation

**Low priority — sole Verji caller is deprecated**

14. Implement DELETE `/_synapse/admin/v2/rooms/{roomId}` (DeleteRoom)

---

## 7. Testing Strategy

### 7.1 Integration Tests

Each endpoint should have integration tests that:
- Verify correct JSON response shape against Synapse's documented format
- Verify 401 for unauthenticated requests
- Verify 403 for non-admin users
- Verify 404 for nonexistent users/rooms
- Verify pagination parameters work correctly (ListAccounts, ListRooms)

### 7.2 Compatibility Tests

Run the existing Verji SDK test suite (`Verji.ItOps.Matrix.Sdk.Test`) against a tuwunel
instance to verify wire compatibility. Key test classes:
- `TestLoginAsUser.cs` (can be adapted for GetUserDevices)
- Any handler integration tests that mock `IAdminApiClient`

### 7.3 Manual Validation

Use `curl` to verify each endpoint matches Synapse's response format by comparing
side-by-side with a Synapse instance.

---

## 8. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Synapse response format has undocumented fields the SDK depends on | SDK calls fail | Audit `GetAccountResult`, `ListRoomsResponse` etc. DTOs for all fields accessed; test with real SDK |
| tuwunel doesn't store some Synapse fields (e.g. `consent_*`, `shadow_banned`) | Missing data in responses | Return sensible defaults (null/false/0); document which fields are stubbed |
| ListAccounts pagination at scale | Slow for large user counts | Start with collect-and-slice; add DB-level pagination if needed |
| Room state serialization differs from Synapse | Callers fail to parse events | Use tuwunel's existing Pdu→JSON serialization which follows Matrix spec |
| Admin auth model differs (Synapse uses DB flag, tuwunel uses room membership) | Edge cases in admin detection | Functionally equivalent — both are checked via `user_is_admin()` |
| Upstream tuwunel refactors service APIs our crate depends on | Build breaks on rebase | Pin to specific service trait signatures; keep handler logic thin (delegate to services, don't duplicate logic) |
| Separate crate can't access `pub(crate)` internals in `tuwunel_service` | Blocked on missing public API | Contribute missing `pub` accessors upstream, or use the admin command service layer as intermediary |

---

## 9. Open Questions

1. **Three-party IDs (threepids):** Does tuwunel store email/phone associations? If not,
   `threepids` in GetAccount/PutAccount can return `[]` / ignore input. The main Verji
   caller (`UpdateSynapseEmailCommandHandler`) would need an alternative approach.

2. **User creation timestamp:** Is `creation_ts` stored? If not, can we derive it from
   the user's first event?

3. **Last-seen metadata:** Does the `Device` struct in tuwunel include `last_seen_ip`
   and `last_seen_ts`? Needed for GetUserDevices.

4. **Room deletion refactor:** The existing delete-room logic is in the admin command
   handler (text-based). It needs to be extracted into a service function callable from
   HTTP. How coupled is it to the admin room command system?

5. ~~Custom axum routes vs Ruma~~ — **Resolved:** Plain axum routes, since the Synapse
   admin API is not part of the Matrix spec and has no Ruma types.

---

## Appendix A: Out-of-Scope Endpoints

| Endpoint | Reason |
|----------|--------|
| `GetDeleteRoomStatusByRoomId` | Dead code — no production callers |
| `GetDeleteRoomStatusByDeleteId` | Dead code — no production callers |
| `CreatePassword` | Dead code — same endpoint as PutAccount, no separate callers |
| `DeleteUserDevices` | Dead code — no production callers |
| `LoginAsUser` | Dead code — only in tests; impersonation uses AppService masquerading |
| `MakeRoomAdmin` | Custom Verji endpoint — deferred to future iteration |

## Appendix B: Verji Impersonation Architecture

Verji's user impersonation does NOT use the admin API. Two mechanisms exist:

**V1 — `LoginImpersonatedUser`:**
```
POST /_matrix/client/v3/login
Authorization: Bearer <appservice_token>
{ "type": "m.login.application_service", "identifier": { "type": "m.id.user", "user": "<target>" } }
```
Pure Client-Server API. No admin endpoint involved.

**V2 — `LoginImpersonatedUserWithExistingDevice`:**
Same as V1, but first calls `GET /_synapse/admin/v2/users/{userId}/devices` to find an
existing device ID to pass as `device_id` in the login request. This avoids creating
a new device on every impersonation.

Only `MatrixServiceConnector.cs` in itops-matrix uses the V2 path.
