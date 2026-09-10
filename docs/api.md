# The two Govee APIs, the credential paths, and the traps

What `govee` talks to, how it authenticates, and the places these APIs
mislead you. The Platform API is documented by Govee; the app API is not —
everything in that half is reverse-engineered and dated so the next person
can tell what may have rotted.

## Platform API (documented, `src/api/client.rs`)

```
https://openapi.api.govee.com/router/api/v1
Govee-API-Key: <key>
Content-Type: application/json
```

Get a key in the Govee Home app: Profile > Settings > Apply for API Key.
Every response is an envelope; the HTTP status is usually 200 and the real
status is inside:

```json
{ "code": 200, "message": "success", "data": … }     // or "msg", or "payload"
```

`code` 401/403 → the key is bad (exit 3); 429 → rate limited (exit 5);
anything else non-200 → exit 5 with the message. Rate limits: 10,000
requests/day per account plus per-minute limits, reported in the
`X-RateLimit-Remaining` and `API-RateLimit-Remaining` response headers
(`--verbose` prints them).

| Endpoint | Body | Returns |
|---|---|---|
| `GET /user/devices` | — | `data: [{sku, device, deviceName, type, capabilities: [{type, instance, parameters}]}]` |
| `POST /device/state` | `{requestId, payload: {sku, device}}` | `payload.capabilities[]` with a `state.value` each |
| `POST /device/control` | `{requestId, payload: {sku, device, capability: {type, instance, value}}}` | echo of the capability |
| `POST /device/scenes` | `{requestId, payload: {sku, device}}` | `payload.capabilities[]` — `lightScene` and `snapshot` instances with `parameters.options[] {name, value: {paramId, id}}` |
| `POST /device/diy-scenes` | same | `diyScene` options, same shape |

`requestId` is any unique string; `govee` sends a UUID v4.

Capability values the commands send (`src/models/device.rs`):

- `on_off/powerSwitch`: `1` / `0`
- `range/brightness`: `1..=100`
- `color_setting/colorRgb`: one packed integer `r*65536 + g*256 + b`
- `color_setting/colorTemperatureK`: `2000..=9000`
- `toggle/gradientToggle`, `toggle/dreamViewToggle`: `1` / `0`
- `dynamic_scene/lightScene`, `dynamic_scene/snapshot`: `{paramId, id}` taken
  verbatim from the scene list's `value`
- `segment_color_setting/segmentedColorRgb`: `{"segment": [i, …], "rgb": <packed>}`;
  `segmentedBrightness`: `{"segment": [i, …], "brightness": 1..=100}` — the
  user passes this JSON as-is; `segment info` shows the device's field
  definitions
- `music_setting/musicMode`: `{"musicMode": <id>, "sensitivity": 0..=100, "autoColor": 1}`;
  the mode ids come from `parameters.fields[fieldName == "musicMode"].options`,
  not from top-level `options` like the other enums

Capabilities are discovered per device at run time; a command refuses
(exit 2) when the device lacks the `type/instance` it needs.

**Traps**

- Scene names may contain a non-breaking space (`Milky Way`). Matching
  normalizes all Unicode whitespace to a plain space and lowercases.
- Device ids are colon-separated hex (`AA:BB:…`, eight octets). Google Home
  identifies the same device as `<SKU>_<MAC>`; `rooms devices` emits that
  form as `id`.
- Group devices (`BaseGroup`, `SameModeGroup` SKUs) appear in the list with
  their own ids and no capabilities of note.
- A `/device/state` for an offline device answers 200 with stale values; the
  `online` capability in the same payload says so.

## Govee Home app API (private, `src/api/app.rs`)

```
https://app2.govee.com
appVersion: 7.4.10
clientId: <32 hex chars, stable per installation>
clientType: 1
iotVersion: 0
timestamp: <unix millis>
User-Agent: GoveeHome/7.4.10 (com.ihoment.GoVeeSensor; build:8; iOS 26.5.0) Alamofire/5.11.0
Authorization: Bearer <token>            (after login)
```

Responses carry their own `status` (200 on success; 401/403 → exit 3 with a
pointer at `auth login-account`) and `message`.

**Trap — old `appVersion`s are rejected.** The header set mirrors the iOS
app of 2026-08; a version Govee has retired gets a non-200 status with a
generic message. Bump the constants together when that happens.

### Login — email + password + emailed code (verified 2026-08)

1. `POST /account/rest/account/v2/login` with
   `{"email", "password", "client": <clientId>}`.
   - `status: 200` → `client.token` (the bearer) and `client.accountId`.
   - `status: 454` → Govee wants a verification code.
   - `status: 455` → the code was wrong or expired.
2. On 454: `POST /account/rest/account/v1/verification` with
   `{"type": 8, "email"}` → Govee emails a code (`status: 200`).
3. Log in again with `"code": "<digits>"` added to the body of step 1.

**Trap — the code is bound to the `clientId`.** A new client id per attempt
makes every code "wrong". `govee` generates the id once, stores it in the
keychain item *before* the first request, and reuses it on `--code`. The
session item is `{token, account_id, client_id, email}` under `piekstra.govee`
/ `account`; the config file records the email so the read is gated.

Since 2026-05 Govee asks for the code on every login from a new client, so a
headless login is always two invocations (`code_sent`, then `--code`).

### Device list — `POST /device/rest/devices/v1/list` (body `{}`)

```
groups[]                 {groupId, groupName}          — the app's rooms
devices[]
  sku, device, deviceName
  groupId                 the room; 0 or an unknown id = no room
  deviceExt.deviceSettings   a JSON *string*; parse it
    wifiName / wifiSoftVersion   present ⇒ Wi-Fi (cloud) device
    bleName                      Bluetooth name (also present on Wi-Fi devices)
  deviceExt.lastDeviceData   a JSON string with the last reported state
```

Connectivity rule (`app_devices`): a non-empty `wifiName` or any
`wifiSoftVersion` ⇒ `wifi`; otherwise `bluetooth`. Bluetooth-only devices are
the ones the Platform API (and Google Home) never list; `devices list` shows
them from this call with `type: bluetooth-only`.

**Trap — two rooms may share a name.** Membership is keyed by `groupId`,
never by `groupName`; `rooms move` and `rename` resolve the room first and
refuse an ambiguous name (exit 4 with the candidates).

### Room writes — decoded from the Android app, verified live 2026-09-09

All three carry the app's legacy envelope `{"transaction": <millis as string>,
"key": "", "view": 0}` plus the fields below, and answer `{status, message,
data}`.

| Command | Call | Body | Notes |
|---|---|---|---|
| `rooms create` | `POST /bff-app/v1/devices/groups` | `groupName` | returns `data.groupId` |
| `rooms move`, `rooms rename` | `PUT /bff-app/v1/group/edit` | `groupId`, `groupName`, `devices: [{device, sku}, …]` (`transaction` sent as `""`) | the room's **complete** membership — the app's "Edit the Room" screen |
| `rooms delete` | `PUT /bff-app/v1/devices/groups/manage` | `groupIds: [keep…]`, `deleteGroupIds: [id]` | the **complete** remaining room list, in order |

**Traps**

- `group/edit` replaces the membership: a device omitted from `devices` is
  removed from the room. A move is therefore "the target room's current
  members plus this one", computed from a *fresh* device list read after the
  confirmation prompt (the prompt may have blocked for minutes). The server
  removes the device from its previous room itself.
- `groups/manage` is the same shape of hazard for rooms: send every room you
  want to keep. `govee` refuses to delete a room that still holds devices
  (exit 2), and re-checks after the prompt.
- Room names are limited to 22 characters (the app enforces it client-side;
  the server accepts longer names it then truncates in places). `govee`
  enforces 1–22 up front (exit 2).
- Write responses prove nothing. Every write is followed by a fresh device
  list read; a write the server accepted but did not apply is reported as
  exit 5 ("accepted the write but … on read-back").
- These rooms are the Govee app's. Google Home keeps its own room per device
  and only honours Govee's `roomHint` when the device is new to it; moving a
  device here does not move it in Google Home. `rooms devices` emits
  `device-rooms/v1` (`id` = `<SKU>_<MAC>`, `room`, `cloud`, `connectivity`)
  so `ghome audit --expect -` can compare the two.

## Keychain layout and the 0.1 migration

`piekstra.govee` holds `api_key` (the Platform key, as-is) and `account` (the
session blob above). Version 0.1 kept the same two items under the service
`govee-cli`; `auth login` and `auth login-account` move them on first use
(read old → write new → delete old) and record the result in the config,
after which the legacy service is never probed again. Read paths never probe
it: a keychain read from a freshly built binary is a macOS prompt, and the
config gate exists so that a machine with nothing configured exits 3 without
one.
