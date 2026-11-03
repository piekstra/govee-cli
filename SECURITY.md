# Security

`govee` holds two credentials, both only in the OS keychain under
`piekstra.govee`; nothing is written to disk and nothing is logged, at any
verbosity:

- the **Platform API key** (`auth login`), which can list and control every
  device on the Govee account — and, through the raw `api` passthrough, call
  anything the developer API exposes;
- the **Govee Home account session** (`auth login-account`), a bearer token
  for the app's private API that can read and rearrange the account's rooms.
  The account password is used once to obtain it and never stored.

`govee auth logout` clears the account session; `--forget` also removes the
API key, the connector token (`mcp-http-token`, below) and the config file. Revoke a key server-side from the Govee Home app
(Profile > Settings > Apply for API Key) — a revoked key is exit 3 here.

`$GOVEE_API_KEY`, when set, is used instead of the keychain. Treat the
environment of any process that sets it accordingly.

The config file (`~/.config/govee/config.json`) records the account email and
whether a key is stored; it never contains a secret.

## The MCP server

`govee mcp` serves five tools to an MCP client (Claude Code, Claude Desktop) over stdio; the
server opens no port unless run with `--http` (below). **What the tools return goes to Claude on
every path**, as part of your conversation: your device names and kinds, rooms when an account
is signed in, what each device reports, and with `response_format: "detailed"` its id, which
for Govee contains the device's MAC address.

Three tools read. `govee_power` and `govee_light` change one device each, whose full name or id
the call must carry, whose Govee type must be a light or a socket, and which must not be a group. They are not read-only and
carry `anthropic/requiresUserInteraction`, so a client that honors them asks before every call. They
can send only a power, brightness, color, color-temperature or scene command, and return the state
read back. A tool cannot reach `govee api`, room writes, toggles, segments, music modes or any
credential write. `src/mcp/tests.rs` holds the definitions to that, and `tests/mcp_stdio.rs` the
requests the real binary makes. An approval is the client's: `--http` with `--allow-writes` serves
them to whoever holds the token, and a client that ignores the annotation does not ask.

A name in a result can come from anyone who has named a device on the account, so the server tells
Claude to treat names as data and to change a device only on the user's direct request.

The server reads the config and the key on each call and holds neither between calls, so a new key
applies on the next call. A tool never prompts for a key or creates a keychain item: a missing or
rejected key in the keychain is an error asking the user to run `govee auth login` in a terminal,
and one from `$GOVEE_API_KEY`, which wins over the keychain, asks them to correct it where it is
set, such as the Claude Desktop extension's settings. On macOS a binary macOS has not granted raises
a keychain prompt on a call, and a server left unattended waits on it. A write that reaches its
command 60 s or more after the call began, by the wall clock or by a clock that stops while a Mac
sleeps, sends nothing and says so, so a prompt answered late cannot switch a device. The Claude Desktop extension
does not read the keychain for the key, since it is given one, though it does read the account
session where `auth login-account` has signed in.

`govee mcp --http --port <PORT>` serves over HTTP for the user's own Cloudflare Worker, which reaches
it through the user's own Cloudflare Tunnel (cli-common's `connector/`). It binds 127.0.0.1 only, with
no flag for another address, and refuses to start without a bearer token of 32 or more characters,
from `GOVEE_MCP_TOKEN` or the keychain item `mcp-http-token` (`piekstra.govee`), which `govee mcp
--new-token` makes from 256 random bits and replaces only with `--overwrite`. Every request must
carry the token (compared in constant time) and a `Host` of `127.0.0.1` or `localhost`, and must not
carry an `Origin`; there are no CORS headers. It serves the three reads only, and a write is neither
listed nor callable, unless started with `--allow-writes`. The Govee key stays on your computer: the
Worker sends tool calls and receives results, and never sees it.

**Tool results also cross Cloudflare on this path**, through your tunnel and your Worker, on their
way to Claude, and the Worker holds a copy of the token as `HOME_GOVEE_TOKEN`. Who holds what, and
what a leaked token can do, is in the connector's
[threat model](https://github.com/piekstra/cli-common/blob/main/connector/README.md#threat-model).
With `--allow-writes`, a leaked token can also turn your lights and plugs on and off.

`govee auth logout --forget` removes the API key and `mcp-http-token`. A server already running keeps
the token it started with and goes on answering, without a key, until it is stopped; plain `auth
logout` clears only the account session. To revoke the connector's access and keep serving, run `govee
mcp --new-token --overwrite` and restart the server (`launchctl kickstart -k gui/$(id
-u)/com.piekstra.govee-mcp-http`); the old token works until the restart. To stop serving, run
`launchctl bootout gui/$(id -u)/com.piekstra.govee-mcp-http` and stop the tunnel.

To report a vulnerability, open a private security advisory on the GitHub
repository rather than a public issue.
