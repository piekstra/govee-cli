# AGENTS.md

Guidance for AI coding agents (and humans) working in this repo. Tool-agnostic;
`CLAUDE.md` points here.

## What this is

`govee` — a Rust CLI over two Govee upstreams: the public **Platform API**
(device control: `openapi.api.govee.com`) and the Govee Home app's **private
API** (rooms and the account login: `app2.govee.com`). A thin, Govee-specific
layer over the shared [`cli-common`](https://github.com/piekstra/cli-common)
`pk-cli-*` crates (auth shapes, config, secrets, self-update, output, exit
codes, the confirmation gate, reference resolution). This repo owns only the
two clients, the device models, and the commands.

## Build, test, lint

```console
make verify     # fmt-check + clippy -D warnings + tests + smoke — the CI gate
make test
make install    # cargo install + re-sign so keychain grants survive
```

Run `make verify` before considering a change done — it's exactly what CI runs.

## Layout

- `src/lib.rs` — `run`: offline commands first (`config`, `self-update`,
  `completions`, `info`), then the async runtime for everything that talks
  to Govee. `src/main.rs` is `parse` + `output::fail`.
- `src/cli/mod.rs` — the clap tree (`CommonArgs` + `--config` + the hidden
  0.1 `--table`) and `Ctx` (the credential gates). One module per command
  group under `src/cli/`; `src/cli/output.rs` is the one local composition
  over `pk_cli_core::output` (a list with scalar context).
- `src/auth/` — `api_key` (env → config gate → keychain), `account` (the app
  session as one JSON keychain item, `get_json`/`set_json`), `legacy` (the
  one-time move from 0.1's `govee-cli` service via
  `CredentialStore::migrate_from`).
- `src/api/client.rs` — the Platform API (async reqwest, `Govee-API-Key`
  header, the `{code, message, data}` envelope). `src/api/app.rs` — the app
  API: login with emailed code, device list with rooms/connectivity, the room
  writes, and the Platform/app merge.
- `src/models/` — device info, capabilities, SKU → type table, and the
  value validators (`validate_brightness` etc.).
- `src/resolve.rs` — the Platform device list and `resolve_device` over
  `pk_cli_core::resolve::pick` (the family ladder; ties are exit 4).
- `src/config.rs` — the on-disk config: `username` and the
  `api_key_in_keychain` marker.
- `src/error.rs` — `AppError` (vendor layer) and its `From` into
  `pk_cli_core::CliError`.
- `tests/` — offline surface tests, fixture contract tests, unit tests; see
  `tests/fixtures/README.md`.
- `docs/api.md` — both APIs, the credential paths, and every trap found so far.

## Conventions (do not break these)

- **`--json` on every command**, one DTO tagged `"schema": "<name>/v1"`.
  Text is the default (key/value blocks, pipe tables). Human output → stdout;
  diagnostics → stderr. Keep both paths in sync; a breaking DTO change bumps
  the `/vN` suffix. `-t/--table` is a hidden no-op kept for one major version.
- **Exit codes:** 0 ok · 2 usage · 3 auth · 4 not found · 5 upstream · 6
  confirmation required. Every vendor error goes through `AppError` →
  `CliError`; never `process::exit` elsewhere.
- **Validate before the keychain.** Argument checks (`validate` in each
  command module) and `pk_cli_core::confirm::require_confirmable` run before
  `Ctx::api()` / `Ctx::require_account()`, so `--help`, bad input, and a
  headless write without `--force` never prompt or hang. The `confirm`
  prompt itself comes after the reads that produce the names in it.
- **The keychain is read only when the config says there is something to
  read.** `api_key_in_keychain` gates the API key; `username` gates the
  account session. `$GOVEE_API_KEY` bypasses the keychain entirely. The
  legacy `govee-cli` service is probed only from `auth login` /
  `auth login-account`, never from a read path.
- **Secrets** come from the keychain (`piekstra.govee`), `--stdin`,
  `--from-env`, or a no-echo prompt — never argv, never logs, never a file.
  `--verbose` prints URLs, status codes and rate-limit headers, never the key
  or the bearer.
- **Control doesn't prompt; rooms do.** Device control is reversible and
  runs without confirmation. Room writes (`rooms move|create|rename|delete`)
  prompt unless `--force`, exit 6 non-interactively *before* any network
  call, and read the device list back before reporting success — the app's
  write responses prove nothing.
- **`rooms devices` is the `smart-home/v1` profile's `device-rooms/v1`**
  (cli-common DESIGN.md §1.8): `id` = `<SKU>_<MAC>`, `name` omitted when
  unknown (never null), `room` omitted when the app files the device in no
  room (never null; the profile relaxed `room` within v1, so no `/v2` bump),
  `source: "govee"`, `cloud`, `connectivity`. `ghome audit --expect -` joins
  on it and reports a roomless row as `unfiled`; don't reshape it without
  the profile.
- **Room writes send whole collections.** `PUT /group/edit` carries a room's
  complete membership and `groups/manage` the complete remaining room list;
  both are computed from a fresh read *after* the prompt, never from the
  pre-prompt snapshot.

## Tests

Offline, always. No test may read the OS keychain: `cargo test` produces an
ad-hoc-signed binary that macOS treats as a new identity, so a credentialed
command would prompt once per keychain item on every run. The surface tests
use a nonexistent `GOVEE_CONFIG` (no marker, no username) and unset
`GOVEE_API_KEY`, so every credentialed command stops at exit 3 before the
keychain. Two tests reach the network on purpose (`self-update --check`, and
`devices list` with a bogus env key) and accept "no network" as a pass.
Live checks go through the installed binary by hand.

## Safety & privacy (public repo)

- Nothing tracked in git may carry a real email, device id, API key, account
  id, client id, Wi-Fi name, or bearer token. `tests/cli_surface.rs` scans
  `git ls-files` for those shapes. Fixtures use repeating-pattern MACs and
  `@example.com`.
- Runtime output legitimately carries all of that; that's the tool's job.
- `auth login-account` triggers a real verification email from Govee. Don't
  run it in tests or CI; the login flow's parsing is unit-tested offline.
- Room writes act on the owner's real Govee Home account. Don't run them to
  "test"; the membership/room-list logic is unit-tested against fixtures.

## Definition of done

`make verify` green, CI green, the change dogfooded through the installed
binary, `--json` and human output in sync, `docs/api.md` matching reality,
and no secrets or personal data anywhere in the diff.
