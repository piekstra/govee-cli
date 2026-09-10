# Changelog

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
