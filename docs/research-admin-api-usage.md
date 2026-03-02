# Tuwunel Admin API Gap Analysis for Verji Migration

## Executive Summary

Verji's SDK (`Verji.ItOps.Matrix.Sdk`) defines **15 Synapse Admin API endpoints**.
After auditing four Verji codebases, only **9 are actually called** in production code.
5 endpoints are dead code in the SDK, and 1 (MakeRoomAdmin) is deferred.

Tuwunel has **zero HTTP admin endpoints** — all administration is via Matrix room commands.
The tuwunel docs note an HTTP admin API is "planned (low priority)".

**Scope for tuwunel implementation: 9 endpoints.**

---

## 1. Verji SDK Admin API Inventory

Source: `Verji.ItOps.Matrix.Sdk.Abstractions\Interface\Api\IAdminApiRaw.cs`

| # | Method | Synapse Endpoint | Purpose |
|---|--------|-----------------|---------|
| 1 | `GetDeleteRoomStatusByRoomId` | GET `/_synapse/admin/v2/rooms/{roomId}/delete_status` | Poll room-deletion progress |
| 2 | `GetDeleteRoomStatusByDeleteId` | GET `/_synapse/admin/v2/rooms/delete_status/{deleteId}` | Poll specific deletion job |
| 3 | `DeleteRoom` | DELETE `/_synapse/admin/v2/rooms/{roomId}` | Delete/purge a room |
| 4 | `GetAccount` | GET `/_synapse/admin/v2/users/{userId}` | Get user details |
| 5 | `GetRoomMembers` | GET `/_synapse/admin/v1/rooms/{roomId}/members` | List room members |
| 6 | `GetRoomDetails` | GET `/_synapse/admin/v1/rooms/{roomId}` | Room metadata |
| 7 | `ListRooms` | GET `/_synapse/admin/v1/rooms` | Paginated room listing |
| 8 | `GetRoomState` | GET `/_synapse/admin/v1/rooms/{roomId}/state` | Room state events |
| 9 | `ListAccounts` | GET `/_synapse/admin/v2/users` | Paginated user listing with filters |
| 10 | `PutAccount` | PUT `/_synapse/admin/v2/users/{userId}` | Modify user (password, displayname, admin, deactivate) |
| 11 | `CreatePassword` | PUT `/_synapse/admin/v2/users/{userId}` | Reset password (same endpoint as PutAccount) |
| 12 | `GetUserDevices` | GET `/_synapse/admin/v2/users/{userId}/devices` | List user devices |
| 13 | `DeleteUserDevices` | POST `/_synapse/admin/v2/users/{userId}/delete_devices` | Bulk-delete devices |
| 14 | `MakeRoomAdmin` | POST `/_synapse/admin/v1/rooms/{roomId}/verji/make_room_admin` | **Custom Verji endpoint** — grant room admin |
| 15 | `LoginAsUser` | POST `/_synapse/admin/v1/users/{userId}/login` | Impersonation token |

---

## 2. Actual Usage Per Project

### Cross-reference matrix

| # | Endpoint | itops-matrix | linkonboarding | VmxAccount | verji-itops | Status |
|---|----------|:---:|:---:|:---:|:---:|---|
| 1 | `GetDeleteRoomStatusByRoomId` | — | — | — | — | DEAD CODE |
| 2 | `GetDeleteRoomStatusByDeleteId` | — | — | — | — | DEAD CODE |
| 3 | `DeleteRoom` | 1 call (deprecated) | — | — | — | IN SCOPE |
| 4 | `GetAccount` | 3 callers | 1 caller | — | 1 caller | IN SCOPE |
| 5 | `GetRoomMembers` | 2 callers | — | — | 1 caller | IN SCOPE |
| 6 | `GetRoomDetails` | 1 caller | — | — | — | IN SCOPE |
| 7 | `ListRooms` | — | — | — | 2 callers | IN SCOPE |
| 8 | `GetRoomState` | 4 callers | — | — | 1 caller | IN SCOPE |
| 9 | `ListAccounts` | 1 caller | — | — | — | IN SCOPE |
| 10 | `PutAccount` | 3 callers | — | — | — | IN SCOPE |
| 11 | `CreatePassword` | — | — | — | — | DEAD CODE |
| 12 | `GetUserDevices` | via V2 Login SDK | — | — | — | IN SCOPE |
| 13 | `DeleteUserDevices` | — | — | — | — | DEAD CODE |
| 14 | `MakeRoomAdmin` | 1 caller | — | — | — | DEFERRED |
| 15 | `LoginAsUser` | tests only | — | — | — | DEAD CODE |

### User impersonation: does NOT use the admin API

The SDK provides two impersonation mechanisms. Neither uses the `LoginAsUser` admin endpoint:

1. **`LoginImpersonatedUser`** (v1) — uses **Application Service masquerading** via
   `POST /_matrix/client/v3/login` with type `m.login.application_service`. No admin API involved.
2. **`LoginImpersonatedUserWithExistingDevice`** (v2) — same Client-Server login, but first
   calls **`GetUserDevices`** (admin API: `GET /_synapse/admin/v2/users/{userId}/devices`)
   to find an existing device to reuse, avoiding device proliferation.

The admin API's `LoginAsUser` (`POST /_synapse/admin/v1/users/{userId}/login`) is only
referenced in test files — no production caller exists.

**`GetUserDevices` is therefore in scope** — it is called indirectly by the V2 login SDK.

### Impersonation call sites by project

| Project | File | Method | Purpose |
|---------|------|--------|---------|
| **itops-matrix** | `AddVsysAndTenantInfoToRoomCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to add tenant info |
| **itops-matrix** | `ChangeRoomNameCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate member to rename room |
| **itops-matrix** | `CleanUpMxConversationsCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to leave/forget rooms |
| **itops-matrix** | `RemoveUserFromRoomCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to leave/forget room |
| **itops-matrix** | `AddTenantInfoToRoomCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to set room state |
| **itops-matrix** | `AddUserToAdminRoomCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate invitee + inviter |
| **itops-matrix** | `LogoutMatrixDevicesCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate with specific deviceId |
| **itops-matrix** | `MatrixServiceConnector.cs` | LoginImpersonatedUserWithExistingDevice (V2) | Get hierarchy via existing device — **uses GetUserDevices admin API** |
| **itops-matrix** | `RoomMembersPersonIdProvider.cs` | GetAppServiceAccessTokenImpersonatingUserInRoom | Fallback impersonation for member listing |
| **VmxAccount** | `AccountConnector.cs` | LoginImpersonatedUser (V1) | Get user's rooms for account overview |
| **VmxAccount** | `CorrectInviteRoleToStandardUserCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to leave admin room on role downgrade |
| **verji-itops** | `CreateHierarchyAnalysisCommandHandler.cs` | GetAppServiceAccessTokenAsUser | Impersonate to get DM rooms + room list |

**VmxAccount** and **verji-itops** use only the V1 path (no admin API).
**itops-matrix** has one V2 caller (`MatrixServiceConnector`) that hits `GetUserDevices`.

### Detailed call sites

#### verji-itops-matrix (Verji.ItOps.Matrix.Esb.Handlers + SDK wrappers)

| File | Method(s) Called | Purpose |
|------|-----------------|---------|
| `AddVsysAndTenantInfoToRoomCommandHandler.cs` | GetRoomState, MakeRoomAdmin | Add vsys user as room admin; verify power levels |
| `ChangeRoomNameCommandHandler.cs` | GetRoomMembers | Find a member to impersonate for room rename |
| `LogoutMatchingDevicesCommandHandler.cs` | ListAccounts | Paginate all users to find matching devices |
| `RenameDmRoomCommandHandler.cs` | GetRoomState | Check if DM room already has a name |
| `UpdateSynapseEmailCommandHandler.cs` | GetAccount, PutAccount | Read current 3PIDs, update email |
| `UpdateMSpaceForRoomsAndTenantSpaceCommandHandler.cs` | GetRoomState | Read tenant info + parent space IDs |
| `VerjiAccountSdk.cs` (SDK wrapper) | GetAccount, PutAccount | Account deactivation |
| `VerjiSdkV2 Account.cs` (SDK wrapper) | GetAccount, PutAccount | Account deactivation (V2 wrapper) |
| `VerjiSdkV2 RoomMetadata.cs` (SDK wrapper) | GetRoomDetails, GetRoomState | Room metadata retrieval |
| `VerjiSdkV2 SystemManagedSpaces.cs` (SDK wrapper) | DeleteRoom | Delete personal space (deprecated, TODO for removal) |
| `VerjiLoginSdk.cs` (SDK wrapper) | GetRoomMembers | Find user to impersonate in room |

#### verji-linkonboarding (Verji.LinkOnboarding.Esb.Handlers)

| File | Method(s) Called | Purpose |
|------|-----------------|---------|
| `OnboardingValidationHelpers.cs` | GetAccount (V2, V1 fallback) | Verify Matrix user exists and is not deactivated during onboarding |

#### verji-itops (Verji.ItOps.Esb.Handlers)

| File | Method(s) Called | Purpose |
|------|-----------------|---------|
| `CreateHierarchyAnalysisCommandHandler.cs` | ListRooms, GetRoomMembers, GetAccount, GetRoomState | Enumerate all rooms, members, power levels; check deactivation status |
| `ItOpsConnector.cs` | ListRooms | Get total room count for hierarchy analysis scope |

#### VmxAccount

No direct admin API usage. Uses client-server Matrix APIs only.

---

## 3. Tuwunel Current State

### No HTTP Admin API

Tuwunel provides admin functionality exclusively through **admin room commands** (text
messages sent to a special admin room). There are no `/_synapse/admin/` HTTP routes.

### Existing admin commands that overlap with in-scope endpoints

| Endpoint | Tuwunel Admin Command | Gap |
|---|---|---|
| DeleteRoom | `!admin rooms moderation` / delete-room | No HTTP interface |
| GetAccount | `!admin users` subcommands | No HTTP interface; less structured output |
| ListAccounts | `!admin users list-users` | No HTTP interface; no pagination/filter params |
| PutAccount (deactivate) | `!admin users deactivate` | No HTTP interface |
| ListRooms | `!admin rooms list-rooms` | No HTTP interface |
| GetRoomDetails | `!admin rooms info` | No HTTP interface |
| GetRoomMembers | — | **Not available as admin command** |
| GetRoomState | — | **Partially via debug commands** |

---

## 4. Gap Analysis (in-scope endpoints only)

### Category A — Functionality exists, needs HTTP wrapper

These have tuwunel admin-command equivalents. We need to expose them as HTTP endpoints
with Synapse-compatible JSON request/response shapes.

1. **GetAccount** — user lookup exists
2. **ListAccounts** — user listing exists (needs pagination/filter support)
3. **PutAccount** — deactivate + reset-password exist (need combined endpoint)
4. **ListRooms** — room listing exists
5. **GetRoomDetails** — room info exists
6. **DeleteRoom** — room deletion logic exists

### Category B — Partially exists, needs extension + HTTP wrapper

7. **GetRoomState** — accessible via debug/query commands; needs proper admin route
8. **GetRoomMembers** — room member data exists internally; needs admin-level query + HTTP
9. **GetUserDevices** — device data is in the DB; needs admin-level query + HTTP

---

## 5. Implementation Plan

### Approach: Synapse-compatible HTTP Admin API in tuwunel

Add a `/_synapse/admin/` HTTP route tree in tuwunel that mirrors the 9 Synapse endpoints
Verji actually uses.

- **Zero changes needed in Verji SDK** — existing `IAdminApiRaw` interface works as-is
- **Forward-compatible** — other Synapse admin tooling also works
- **Incremental** — ship endpoints one at a time

All endpoints would live under a new module, e.g. `src/api/admin/`.
Authentication: require a valid access token for a server admin user (same as Synapse).

### Phase 1 — User management (highest value — used by 3 projects)

| Endpoint | Callers | Complexity |
|----------|---------|------------|
| GET `/_synapse/admin/v2/users/{userId}` | itops-matrix, linkonboarding, verji-itops | Low — wrap existing user lookup |
| PUT `/_synapse/admin/v2/users/{userId}` | itops-matrix | Medium — combine deactivate + update fields |
| GET `/_synapse/admin/v2/users` | itops-matrix | Medium — needs pagination + filter support |
| GET `/_synapse/admin/v2/users/{userId}/devices` | itops-matrix (via V2 Login SDK) | Medium — device enumeration |

### Phase 2 — Room read operations (used by hierarchy analysis + room management)

| Endpoint | Callers | Complexity |
|----------|---------|------------|
| GET `/_synapse/admin/v1/rooms` | verji-itops | Low — wrap existing room listing |
| GET `/_synapse/admin/v1/rooms/{roomId}` | itops-matrix | Low — wrap existing room info |
| GET `/_synapse/admin/v1/rooms/{roomId}/members` | itops-matrix, verji-itops | Medium — admin-level member query |
| GET `/_synapse/admin/v1/rooms/{roomId}/state` | itops-matrix, verji-itops | Medium — expose room state to admin |

### Phase 3 — Room mutation

| Endpoint | Callers | Complexity |
|----------|---------|------------|
| DELETE `/_synapse/admin/v2/rooms/{roomId}` | itops-matrix (deprecated usage) | Medium — wire up deletion with Synapse-compatible request/response |

### Deferred (not needed for migration)

| Endpoint | Reason |
|----------|--------|
| `MakeRoomAdmin` | Custom Verji endpoint — deferred, will revisit |
| `GetDeleteRoomStatusByRoomId` | Dead code — no callers |
| `GetDeleteRoomStatusByDeleteId` | Dead code — no callers |
| `CreatePassword` | Dead code — no callers |
| `DeleteUserDevices` | Dead code — no callers |
| `LoginAsUser` | Dead code — no production callers (tests only) |

---

## 6. Recommendation

**Go with Synapse-compatible HTTP admin API in tuwunel, 9 endpoints across 3 phases.**

Priority order based on cross-project impact:

1. **GetAccount** (3 projects depend on it) — unblocks linkonboarding + verji-itops + itops-matrix
2. **GetRoomState + GetRoomMembers + ListRooms** (2 projects each) — unblocks hierarchy analysis
3. **PutAccount + ListAccounts + GetRoomDetails + GetUserDevices** (1 project) — completes itops-matrix support
4. **DeleteRoom** (1 caller, deprecated) — lowest priority, may not be needed if caller is removed

---

## 7. Open Questions

- Does tuwunel already store device data in a queryable way? (relevant if GetUserDevices is needed later)
- Is there an existing authentication middleware in tuwunel's HTTP layer we can reuse for admin auth?
- Should the admin API be behind a feature flag or always available?
- Can room deletion be synchronous in tuwunel (eliminating need for delete-status polling)?
- The `DeleteRoom` caller in `VerjiSdkV2 SystemManagedSpaces.cs` is marked deprecated — can it be removed from Verji side instead of implementing the endpoint?
