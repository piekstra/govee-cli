# govee — Govee smart-home devices from the terminal

`govee` controls Govee lights and strips from the command line, for people
and for agents: power, brightness, colour, scenes, segments and music mode
over the [Govee Platform API](https://developer.govee.com/), plus the rooms
the Govee Home app keeps (which the Platform API has no notion of) over the
app's own private API.

Conforms to [piekstra-cli spec v1](https://github.com/piekstra/cli-common)
(`--json` everywhere, standard exit codes, keychain-only secrets).

**Unofficial.** Not affiliated with or endorsed by Govee. Device control uses
the documented developer API; the `rooms` commands and `auth login-account`
speak the private API behind the Govee Home app, reverse-engineered and
documented in [docs/api.md](docs/api.md). Govee can change or close that at
any time. Use it on your own account, at your own risk.

## Install

```console
cargo install --git https://github.com/piekstra/govee-cli
# or a release binary: https://github.com/piekstra/govee-cli/releases
govee self-update            # later: replace the binary with the latest release
```

## Setup

Get an API key in the Govee Home app (**Profile > Settings > Apply for API
Key**), then store it:

```console
govee auth login             # no-echo prompt; verified against the API, then stored
govee auth status --json
```

Headless: `op read "op://…/credential" | govee auth login --stdin`, or
`govee auth login --from-env GOVEE_API_KEY`. The key lives only in the OS
keychain under `piekstra.govee`; nothing is written to disk. `$GOVEE_API_KEY`,
when set, is used instead of the keychain (env > keychain), for
`op run --`-style invocations.

**Upgrading from 0.1?** Run `govee auth login` once at a terminal: it moves
the key (and any account session) from the old keychain entry instead of
asking for it again. See [CHANGELOG.md](CHANGELOG.md).

## Usage

```console
govee devices list                       # every device; rooms too once an account is signed in
govee devices get "Office Lamp"          # exact name, id, or unique partial
govee devices search lamp
govee devices caps "Office Lamp"         # capabilities with their parameters

govee power on "Office Lamp"
govee power off "Office Lamp"
govee power toggle "Office Lamp"
govee power status "Office Lamp"

govee light brightness "Office Lamp" 75          # 1-100
govee light color "Office Lamp" --hex "#FF0080"
govee light color "Office Lamp" --red 255 --green 0 --blue 128
govee light temp "Office Lamp" 4000              # 2000-9000 K (alias: color-temp)
govee light state "Office Lamp"

govee scene list "Office Lamp"
govee scene list-diy "Office Lamp"
govee scene list-snapshots "Office Lamp"
govee scene activate "Office Lamp" Sunset        # case-insensitive, partial match
govee scene activate-snapshot "Office Lamp" Evening

govee toggle gradient "Office Lamp" on
govee toggle dreamview "Office Lamp" off
govee toggle list "Office Lamp"

govee segment info "Office Lamp"
govee segment color "Office Lamp" '{"segment":[0,1,2],"rgb":16711680}'
govee segment brightness "Office Lamp" '{"segment":[0,1,2],"brightness":80}'

govee music list "Office Lamp"
govee music set "Office Lamp" Rhythm --sensitivity 60
```

Device control is reversible, so it never prompts. Bad values (brightness
0, 9500 K, a malformed hex colour) are rejected with exit 2 before any
credential is read.

### Rooms (Govee Home account)

The Platform API has no rooms; the Govee Home app's own API does. Sign in to
the account once (Govee emails a verification code):

```console
op read "op://Private/auth.govee.com/password" | govee auth login-account --stdin --email you@example.com
# → status "code_sent"; then, with the code from the email:
op read "op://Private/auth.govee.com/password" | govee auth login-account --stdin --email you@example.com --code 123456

govee rooms list                                 # rooms with device counts
govee rooms devices                              # device-rooms/v1, for `ghome audit --expect -`
govee rooms move "Old Lamp" --room Storage       # asks first; --force to skip
govee rooms create Storage
govee rooms rename "Guest Bedroom" Gym
govee rooms delete Loft                          # only when empty
```

Room writes prompt for confirmation unless `--force`; non-interactive runs
(`--json`, a pipe, an agent) must pass `--force` or they stop with exit 6
before touching anything. Every write is read back from the device list
before it is reported as done. They act on the Govee app's rooms only; Google
Home files devices on its own (see
[google-home-cli](https://github.com/piekstra/google-home-cli)).

`rooms devices` emits ids as `<SKU>_<MAC>`, the id Google Home sees for
Govee devices, so the two line up:

```console
govee rooms devices --json | ghome audit --expect - --problems
```

### Raw API

```console
govee api GET /user/devices
govee api POST /device/state --data '{"requestId":"1","payload":{"sku":"H60B0","device":"AA:BB:…"}}'
```

Paths are relative to `https://openapi.api.govee.com/router/api/v1`; the
response envelope is printed as-is (`api-response/v1`).

## Output

Text is the default: key/value blocks for one resource, pipe tables for
lists. `--json` emits exactly one schema-tagged DTO per command
(`device-list/v1`, `device/v1`, `power/v1`, `light-state/v1`,
`scene-list/v1`, `room-list/v1`, `device-rooms/v1`, …); errors become
`{"error": {"code", "message"}}` on stdout with the message repeated on
stderr. `-q` silences diagnostics, `-v` prints requests and rate-limit
headers (never the key).

## Exit codes

0 ok · 2 usage · 3 auth (run `auth login`, or `auth login-account` for
rooms) · 4 not found · 5 upstream (Govee down, rate limit, rejected write) ·
6 confirmation required (a room write without `--force` in a non-interactive
run).

## Configuration

`govee config path|show|set|unset` manages `~/.config/govee/config.json`
(`--config <PATH>` or `$GOVEE_CONFIG` overrides). The one settable key is
`username`, the Govee account email `auth login-account` defaults to. The
file also records whether a key is stored in the keychain, so a machine with
nothing configured fails fast (exit 3) without a keychain prompt.

## Rate limits

The Platform API allows 10,000 requests per day per account plus per-minute
limits; a 429 is exit 5. `--verbose` shows the remaining quota headers.

## Supported devices

Anything the Platform API lists. Capabilities are read from the API at run
time, so new devices work without a code change; `devices caps` shows what a
device offers. See the [wiki](https://github.com/piekstra/govee-cli/wiki) for
device notes.

## Related

- [google-home-cli](https://github.com/piekstra/google-home-cli) — audits
  and fixes where Google Home filed the same devices.
- [cli-common](https://github.com/piekstra/cli-common) — the shared spec and
  crates.

## License

GPL-3.0
