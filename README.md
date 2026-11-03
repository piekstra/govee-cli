# govee — Govee smart-home devices from the terminal

`govee` controls Govee lights and strips from the command line, for people
and for agents: power, brightness, colour, scenes, segments, toggles and music mode
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
cargo install --git https://github.com/piekstra/govee-cli --locked
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

govee toggle list "Bedroom Lamp"                  # what this device can switch
govee toggle set "Bedroom Lamp" side on           # any toggle by (part of) its name
govee toggle set "Bedroom Lamp" ripple off
govee toggle gradient "Office Lamp" on            # shorthands
govee toggle dreamview "Office Lamp" off

govee segment info "Office Lamp"                  # how many segments, how many per call
govee segment color "Office Lamp" --segments 0-3,7 --hex FF1493
govee segment color "Office Lamp" --segments all --red 255 --green 0 --blue 128
govee segment brightness "Office Lamp" --segments 4,5 --brightness 80

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

## Use with Claude

`govee mcp` serves five tools over MCP, so Claude can see your Govee devices and, when you ask,
change one light or plug. There is no hosted service: the tools run on your computer, in your
copy of `govee`, and call Govee's API with your key.

| Tool | Does | Changes a device |
| --- | --- | --- |
| `govee_devices` | What do I have? Name, kind, room, and what the tools can change on it, narrowed by `name` or `room` | no |
| `govee_device_state` | Is it on? Online, power, brightness, color, color temperature, sensor readings | no |
| `govee_scenes` | What can this light show? Its scenes and saved snapshots, narrowed by `name` | no |
| `govee_power` | Turn one light or plug on or off | **yes** |
| `govee_light` | Set one light's brightness, color, color temperature or scene | **yes** |

The two that change a device are not read-only and carry
`anthropic/requiresUserInteraction`, so a client that honors them asks you before every call. Each
takes one device by its full name or its id. Part of a name is refused however unique, so "all"
cannot reach the hallway light, and there is no list of devices, wildcard or group. Each changes a
device whose Govee type is a light or a socket; a heater, kettle or humidifier has the same power
switch and is refused. Each returns the state read back from Govee, about one and three seconds
later, with `confirmed`: `true` only when that state shows the change. Govee does not report the
running scene or fresh values for an offline device, so those stay `false`, and a `note` says why.
That is not a failure, and Claude is told not to send the command again.

No tool can remove a device, rearrange or rename rooms, change your account, a schedule or a
device's firmware, or share access, and none reaches `govee api`. Toggles, segments and music modes
are left to the CLI. Govee's group devices are not offered either, since the Platform API lists them
with no capabilities to control.

Results are concise by default (`response_format: "detailed"` adds a device's id and SKU, and every
other value Govee reports), 25 records a page with a cursor for more. A field Govee left out is
absent and never a default, and an answer of the wrong shape is an error. Each page reads Govee
again, so a device added between pages can repeat or be skipped: for a count, read one call with
`limit` up to 200. Device, room and scene names are yours or Govee's; the server tells Claude to
treat them as data, not instructions.

The tools are worded for a model, so they differ from `--json`, which passes Govee's names through:
`name` for `deviceName`, `device_id` for `device`, `power: "on"` for `powerSwitch: 1`, `color:
"#RRGGBB"` for the packed integer, and `kind: "light"` for `devices.types.light`. A device's id is
left out unless you ask for detailed. `govee_light` refuses a scene name that matches two scenes,
where `govee scene activate` takes the first.

Rooms and Bluetooth-only devices come from the Govee Home account, when `govee auth login-account`
has signed it in; that is Govee's private app API, and a stale session leaves the devices in place
with a `note`. Each call that needs the list reads Govee at least once, and a change reads it three
to four times, against the 10,000 a day of your key.

**Give it a key first.** Run `govee auth login` in a terminal (see [Setup](#setup)). Claude Code and
`--http` use that key from the OS keychain (service `piekstra.govee`). When the key is missing or
rejected the tools say so and ask you to run it. They never ask for it themselves.

**Claude Code.** Add the `govee-cli` plugin from the
[piekstra plugin marketplace](https://github.com/piekstra/claude-plugins):

```console
$ claude plugin marketplace add piekstra/claude-plugins
$ claude plugin install govee-cli@piekstra-plugins
```

or register the server yourself:

```console
$ claude mcp add govee -- govee mcp
```

**Claude Desktop.** Download `govee-<version>.mcpb` from the
[latest release](https://github.com/piekstra/govee-cli/releases/latest) and open it. It asks for your
API key, keeps it in Claude Desktop's settings and passes it to the bundled `govee` as
`$GOVEE_API_KEY`. A key that is wrong or revoked is changed in the extension's settings, and the
tools say so. A non-empty `$GOVEE_API_KEY` wins wherever the server starts, so `govee auth login`
does not replace it. On a computer where `govee auth login-account` has signed in, the extension's
copy reads that session from the keychain for rooms, and macOS asks once whether to allow it.

Any other MCP client that launches a local server can run `govee mcp`.

**Claude on the web and mobile.** claude.ai reaches tools through a remote connector, so `govee`
serves the tools from your computer to a connector you deploy into your own Cloudflare account:

```console
$ govee mcp --new-token            # once: a bearer token, saved in the keychain and printed for the Worker
$ govee mcp --http --port 8760     # serve on 127.0.0.1 only
```

The connector, [cli-common's `connector/`](https://github.com/piekstra/cli-common/tree/main/connector#tools-from-your-computer-home-modules),
reaches that port through a Cloudflare Tunnel in your account; its README walks through the tunnel,
the VPC service, a launchd agent that keeps the server running, and the `HOME_GOVEE_TOKEN` secret,
with the threat model. `--http` binds 127.0.0.1 only, refuses to start without the token, and refuses
any request whose `Host` is not loopback or that carries an `Origin`.

It serves the three reads only. `--allow-writes` adds `govee_power` and `govee_light`, and then
anyone holding the token can change your lights through your Worker, with no prompt unless the client
asks for one; leave it off unless you want that. Your Govee key stays on your computer, but tool
results do not: your device names, rooms and states pass through Cloudflare and reach Claude
([`SECURITY.md`](SECURITY.md)). While the computer sleeps, the tools answer that it is offline.

After `govee self-update`, restart the server: a running one keeps the old code, and its launchd agent
restarts it only when it exits. Its start line names the version.

```console
$ launchctl kickstart -k gui/$(id -u)/com.piekstra.govee-mcp-http
```

`self-update` does not reach a Claude Desktop extension, which runs its own copy. Install the new
release's `.mcpb` too.

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
(`--config <PATH>` or `$GOVEE_CONFIG` overrides). The settable keys are
`username`, the Govee account email `auth login-account` defaults to, and
`base_url`, where the Platform API is, which sends your API key there and is for
a stub in tests. The file also records whether a key is stored in the keychain, so a machine with
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
