# Govee CLI - Development Guide

## Project Overview

Rust CLI (`govee`) for controlling Govee smart home devices via the official Govee Platform API.

## Build & Run

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo run -- --help            # Run with args
cargo run -- devices list      # Example command
```

## Architecture

```
src/
  api/          HTTP client for Govee Platform API
  auth/         API key management (keychain + env var)
  cli/          Command handlers (clap-based)
  models/       Device, capability, and type models
  config.rs     Runtime configuration
  error.rs      Error types with exit codes
  lib.rs        Module wiring and dispatch
  main.rs       Entry point
  resolve.rs    Device resolution by name/ID
```

## Key Patterns

- **Dynamic capabilities**: The Govee API returns what each device can do at runtime. Commands validate capabilities before sending control requests.
- **Device resolution**: Users can refer to devices by name (exact, case-insensitive, or partial match) or device ID.
- **JSON-first output**: All commands output JSON to stdout. Errors go to stderr as JSON. Use `--table` for human-readable output.
- **Exit codes**: 0=success, 1=general error, 2=auth error, 3=device not found, 4=rate limited.

## API Reference

- Base URL: `https://openapi.api.govee.com/router/api/v1`
- Auth: `Govee-API-Key` header
- Rate limits: 10K/day, per-minute limits
- See wiki for full API documentation

## Adding a New Device Type

1. Add SKU to `DeviceType` enum in `models/device_type.rs`
2. Add SKU prefix to `SKU_MAP`
3. Update `category()` and `display_name()` methods
4. No other changes needed - capabilities are detected dynamically
