# Test fixtures — Govee wire shapes

Scrubbed samples of the two upstreams `govee` speaks, loaded by
`tests/fixture_shapes.rs` so the parsers are tested against the documented
shape rather than against string literals in test code.

- `platform-devices.json` — the `data` array of a Platform API
  `GET /user/devices` response (the CLI unwraps the `{code, message, data}`
  envelope first). **Synthetic**, assembled 2026-09-10 from the developer
  documentation's capability shapes; the `parameters` blocks are abbreviated.
- `app-device-list.json` — a Govee Home app `POST /device/rest/devices/v1/list`
  response reduced to the fields the CLI reads: `groups[]` and, per device,
  `sku`/`device`/`deviceName`/`groupId` and the `deviceExt.deviceSettings`
  JSON string that decides Wi-Fi vs Bluetooth-only. **Synthetic**, same date.

## Scrubbing policy (enforced by `tests/fixture_shapes.rs`)

Every identity-bearing value must be an obvious dummy:

- device ids: repeating-pattern MACs (`AA:BB:CC:DD:EE:FF:00:11`), never a
  real one;
- emails: `@example.com` only;
- no API keys (UUID-shaped), account ids, client ids, or bearer tokens
  anywhere — a real capture must have them removed;
- Wi-Fi network names: `ExampleNet`.

Raw captures go in `/captures/` (git-ignored), never in this directory.
