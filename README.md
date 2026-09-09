# Govee CLI

A cross-platform command-line interface for controlling Govee smart home devices via the [Govee Platform API](https://developer.govee.com/).

## Installation

### From source

```bash
cargo install --git https://github.com/piekstra/govee-cli
```

### From releases

Download the latest binary from [GitHub Releases](https://github.com/piekstra/govee-cli/releases).

## Setup

1. Get your Govee API key from the Govee Home app: **Profile > Settings > Apply for API Key**
2. Store the key:

```bash
govee auth login
```

Or set the `GOVEE_API_KEY` environment variable.

## Usage

### Device Management

```bash
# List all devices
govee devices list

# Get device details
govee devices get "Living Room Lamp"

# Show device capabilities
govee devices caps "Living Room Lamp"

# Search by name
govee devices search "lamp"
```

### Power Control

```bash
govee power on "Living Room Lamp"
govee power off "Living Room Lamp"
govee power toggle "Living Room Lamp"
govee power status "Living Room Lamp"
```

### Light Control

```bash
# Set brightness (1-100)
govee light brightness "Living Room Lamp" 75

# Set color by RGB
govee light color "Living Room Lamp" --red 255 --green 0 --blue 128

# Set color by hex
govee light color "Living Room Lamp" --hex "#FF0080"

# Set color temperature (2000-9000K)
govee light temp "Living Room Lamp" 4000

# Get current state
govee light state "Living Room Lamp"
```

### Scenes

```bash
# List available scenes
govee scene list "Living Room Lamp"

# List DIY scenes
govee scene list-diy "Living Room Lamp"

# Activate a scene
govee scene activate "Living Room Lamp" "Sunset"
```

### Output Formats

All commands output JSON by default (machine-readable). Add `--table` for human-readable output:

```bash
govee devices list --table
```

### Verbose Mode

Add `--verbose` to see HTTP requests and rate limit info:

```bash
govee devices list --verbose
```

### Rooms (Govee Home account)

The Platform API has no rooms; the Govee Home app's own API does. Sign in to
the account once (Govee emails a verification code):

```bash
op read "op://Private/auth.govee.com/password" | govee auth login-account --stdin --email you@example.com
# → status "code_sent"; then, with the code from the email:
op read "op://Private/auth.govee.com/password" | govee auth login-account --stdin --email you@example.com --code 123456

govee rooms list                    # rooms with device counts
govee rooms devices                 # device-rooms/v1 (ids are SKU_MAC, as Google Home sees them), for `ghome audit --expect -`
govee rooms move "Old Lamp" --room Storage   # asks first; --force to skip
govee rooms create Storage
govee rooms rename "Guest Bedroom" Gym
govee rooms delete Loft             # only when empty
```

Room writes use the app's own endpoints (`POST /bff-app/v1/devices/groups`,
`PUT /bff-app/v1/group/edit` with the room's complete membership, `PUT
/bff-app/v1/devices/groups/manage`), decoded from the Android app, and every
write is read back from the device list before it is reported. They act on
the Govee app's rooms only; Google Home files devices on its own (see
`ghome`).

## Authentication

The CLI supports two methods for providing your Govee API key:

1. **OS Keychain** (recommended): `govee auth login` stores the key securely in your OS keychain (macOS Keychain, Windows Credential Store, or Linux Secret Service)
2. **Environment variable**: Set `GOVEE_API_KEY` (takes priority over keychain)

## Rate Limits

The Govee Platform API has a limit of 10,000 requests per day per account, plus per-minute limits. Use `--verbose` to monitor your remaining quota.

## Supported Devices

The CLI works with any device supported by the Govee Platform API. Device capabilities are detected dynamically, so new devices work automatically. See the [wiki](https://github.com/piekstra/govee-cli/wiki) for device-specific documentation.

## License

GPL-3.0
