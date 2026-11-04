# Changelog

## v0.4.2 — 2026-11-04

- `auth logout --forget` says what it does not end: a running `govee mcp --http` keeps the token it started with, and one on `$GOVEE_MCP_TOKEN` never had an item to remove.
- cli-common v0.29.0, up from v0.25.0, which brings the Windows config and self-update fixes.

## 0.4.1 — 2026-11-03

- **Security fix:** the API key and the account login no longer follow a redirect off their host.
  reqwest keeps `Govee-API-Key` on every hop, and a 307 or 308 re-sends a body, which at login holds
  the password and the code. A redirect to another host is now an error that says so, and a write
  refused this way is reported as a failure, not as an unknown outcome.
- **Security fix:** `govee api` refuses a plain `http` URL off this machine with exit 2, before it
  reads the key. The key goes with the request, so a full `http://` URL sent it in the clear.
- A write's deadline is now cli-common's `SendDeadline`, with the same 60 s and both clocks. Its
  refusal is an `other` error in place of `upstream`, since Govee did not fail, and it tells the caller
  to check the current state before calling again.
- cli-common v0.25.0, up from v0.23.0.

## 0.4.0 — 2026-11-03

- `govee mcp`: tools for Claude. `govee_devices`, `govee_device_state` and `govee_scenes` read;
  `govee_power` and `govee_light` change one light or plug, named in full, and a client that
  honors their annotations asks you before each call. Each returns the state read back from Govee
  and says whether it shows the change. They run over stdio for Claude Code and Claude Desktop, and
  with `--http` on 127.0.0.1 behind a bearer token for Claude on the web and mobile through your own
  Cloudflare Worker and tunnel, reads only unless `--allow-writes`. No tool can remove a device,
  rearrange rooms, change an account, a schedule or firmware, or reach `govee api`. A write that
  reaches its command 60 s or more after the call began sends nothing and says so, so a keychain
  prompt answered late cannot switch a device.
- `$GOVEE_API_KEY`, which already came before the keychain, is how the Claude Desktop extension
  passes its key.
- Releases are cut by pushing a tag, no longer by merging a version bump. They attach
  `govee-<version>.mcpb`, a Claude Desktop extension, and a Windows x86_64 tarball, and a release is
  published only once every asset is there.
- `self-update` says to restart a running `govee mcp --http` and to install the new `.mcpb`.
- `auth logout --forget` also removes the connector token, and plain `auth logout` says it keeps it.
  A token it cannot remove still lets it clear the config, and it then says so and exits nonzero.
- A failure that got no answer from Govee no longer prints its URL.
- cli-common v0.23.0, up from v0.15.1. The minimum Rust version is 1.89.

## 0.3.1 — 2026-11-03

- `NO_COLOR=1` no longer fails every command with a usage error (exit 2):
  any non-empty `NO_COLOR` disables color, per no-color.org. Built on
  cli-common v0.15.1 (was v0.8.0), whose `CommonArgs` read `NO_COLOR` as a
  clap boolean.
- The patched TLS stack (rustls 0.23.46, rustls-webpki 0.103.16,
  webpki-roots 1.0.11) and tokio 1.54.1, and rand 0.8.8, from the lockfile.
- `make install` builds from `Cargo.lock` (`cargo install --locked`).

## 0.3.0 — 2026-09-16

- `toggle set <device> <toggle> on|off` switches any toggle a device exposes
  (`toggle list` names them): on a Govee floor lamp with several parts that
  is `ripple`, `side` and `bottom`, matched by any unique part of the
  instance name. `toggle gradient|dreamview` stay as shorthands.
- **Breaking:** `segment color|brightness` take `--segments 0-3,7|all` with
  `--hex`/`--red --green --blue` or `--brightness`, checked against the
  device's declared segment count and sent in as many calls as its per-call
  limit needs, and a failure part-way names the segments already set. The
  0.2 raw-JSON positional form lives on as the hidden `--value` for one
  major version. `segment-control/v1` keeps its keys (`segment_color` /
  `segment_brightness: "set"`) and gains `instance`, `segments`, `calls`
  and `hex` / `brightness` on both paths (additive, so still v1).
- `Device::set_gradient` / `set_dreamview` are gone from the library
  surface; `set_toggle(instance, on)` is the one path.
- Device note (H60B0 floor lamp): the eight segments address the side bar;
  the bottom flood and top ripple lights follow the whole-lamp colour
  (`light color`), and each part switches with its toggle.

## 0.2.0 — 2026-09-10

### Fixed (review of #20)

- `light color --hex` rejects non-ASCII input as a usage error instead of panicking on a byte boundary.
- The account session's bearer token is redacted in `Debug` output.
- The CI security job no longer requests the unused `security-events` scope.

**Breaking.** `govee` now conforms to
[piekstra-cli spec v1](https://github.com/piekstra/cli-common/blob/main/DESIGN.md)
and is built on the shared `pk-cli-*` crates (cli-common v0.8.0): output
and error contract, secrets, config, self-update, the confirmation gate,
and the reference-resolution ladder all come from there.

### Breaking changes

- **Text output is the default; `--json` selects JSON.** 0.1 printed JSON by
  default and took `-t/--table` for text. `--table` is still accepted (hidden,
  a no-op) for this major version; update scripts to pass `--json`.
- **JSON shapes are schema-tagged.** Every `--json` document carries
  `"schema": "<name>/v1"`, and every list is `{"items": [...]}` (0.1 printed
  bare arrays for `devices list` and `devices search`). Keys inside records
  are unchanged. Errors are `{"error": {"code", "message"}}` on stdout (0.1:
  `{"error": "<type>", "message"}` on stderr).
- **Exit codes follow the family contract:** 0 ok · 2 usage · 3 auth · 4 not
  found · 5 upstream (API error, rate limit, HTTP) · 6 confirmation required.
  0.1 used 2 for auth, 3 for not-found, 4 for rate-limited, 1 for the rest.
- **Keychain service renamed** from `govee-cli` to `piekstra.govee` (items
  `api_key` and `account`, unchanged). Run `govee auth login` once at a
  terminal after upgrading: it moves both entries instead of asking for the
  key again (`auth login-account` does the same for the account session).
  An entry you already stored by hand under the new service wins, and the
  legacy copy is retired either way. Until one of those runs, credentialed
  commands report exit 3 with a pointer at `auth login`.
- **`auth logout` no longer removes the API key.** It clears the account
  session; `auth logout --forget` removes the key and the config file (SPEC
  semantics). `auth logout-account` is unchanged.
- **`auth login` verifies before storing and refuses to overwrite** a stored
  key without `--overwrite`. It takes the key from a no-echo prompt,
  `--stdin`, or `--from-env <VAR>`; `--no-verify` skips the live check and
  `--non-interactive` never prompts. 0.1 read `$GOVEE_API_KEY` as the value to
  store; use `--from-env GOVEE_API_KEY` for that now.
- **`auth status` is offline** and emits `auth-status/v1` (0.1 fetched the
  device list). It reports `key_source` (`env`/`keychain`) and an
  `account_session` object as extra fields.
- **Device references resolve by the family ladder** (exact name, exact id
  case-insensitively, case-insensitive name, unique partial name); a tie at
  any tier is exit 4 naming the candidates, and an empty reference is exit
  2. `devices search` no longer echoes the query in its DTO.
- **`rooms devices` follows the documented `device-rooms/v1`** (smart-home/v1
  profile): `name` is omitted when the app reports none, never null.
- Package renamed `govee` → `govee-cli` (the binary is still `govee`; the
  library crate keeps the name `govee`). Release assets are now
  `govee-<target-triple>.tar.gz` + `.sha256` for `aarch64-apple-darwin`,
  `x86_64-apple-darwin` and `x86_64-unknown-linux-gnu`; the aarch64-Linux
  and Windows builds are gone.

### Added

- Global `--json`, `-q/--quiet`, `--no-color` (and `$NO_COLOR`),
  `--config <PATH>` / `$GOVEE_CONFIG`.
- `auth set-credential` (raw keychain write, `--stdin`/`--from-env`,
  `--overwrite`), `config path|show|set|unset` (`username`), `self-update
  [--check] [-y]`, `completions <shell>`, `info` (`cli-info/v1`), and
  `api <METHOD> <PATH> [--data JSON]` (raw Platform API passthrough).
- Arguments are validated before any credential is read: out-of-range
  brightness/temperature/sensitivity, bad hex colours, malformed segment
  JSON and an unsupported `api` method are exit 2 without a keychain prompt.
  A room write without `--force` in a non-interactive run is exit 6 before
  the network.
- `$GOVEE_API_KEY` still takes precedence over the keychain at run time.
- `devices list`/`scene list`/`toggle list`/`music list` gained the `ls`
  alias.
- Offline test suite (`tests/cli_surface.rs`, `tests/fixture_shapes.rs`)
  with scrubbed fixtures and a tracked-files PII scan; `make verify`; CI with
  clippy `-D warnings`, cargo-audit and gitleaks; automatic releases on
  version bump.
- `AGENTS.md`, `SECURITY.md`, `CONTRIBUTING.md`, `docs/api.md` (both APIs,
  the room-write shapes, the emailed-code login, and the traps).

### Removed

- The `tabled`, `dialoguer` and direct `keyring` dependencies (replaced by
  the shared crates); `version.txt`.

## 0.1.0

Initial release: device control over the Platform API (`devices`, `power`,
`light`, `scene`, `toggle`, `segment`, `music`), Govee Home account login with
emailed code, `rooms list|devices|move|create|rename|delete`.
