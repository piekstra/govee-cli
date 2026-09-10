# CLAUDE.md

The canonical agent guide for this repo is **[AGENTS.md](AGENTS.md)** — read it
first. It covers build/test/lint, layout, conventions, and the safety rules.

Claude Code specifics:

- **Gate on `make verify`.** Don't report a change as done until it's green
  (fmt + clippy `-D warnings` + tests + smoke). Tests are fully offline and
  never touch the keychain.
- **Never run `auth login-account` to "test" it.** It emails a real
  verification code to the owner's Govee account. `auth login` verifies a
  key against the live API; only run it when the owner asks.
- **Writes act on a real account.** `rooms move|create|rename|delete` change
  the owner's Govee Home rooms; device control commands change real lights.
  Don't run them to "test"; the logic is unit-tested against fixtures.
- **Secrets:** the API key and the account bearer live in the OS keychain
  (`piekstra.govee`). Never print them, put them on argv, or write them to a
  file. A freshly built binary reading the keychain is a macOS prompt — keep
  tests and local checks on the config-gated, credential-free paths.
- **"Deployed" means released + installed.** A change isn't live until the
  release workflow ships it and the binary is installed or `self-update`d.
- **Public repo, private home.** No real emails, device ids, keys, or tokens
  in any diff — fixtures included (dummies only).
