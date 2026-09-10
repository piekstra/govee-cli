# Contributing

1. Read [AGENTS.md](AGENTS.md) — it is the house style and the safety rules.
2. `make verify` must be green before a PR. Tests are offline; nothing may
   touch the keychain or the network beyond the two tests that accept "no
   network" as a pass.
3. New API findings (an endpoint, a wire shape, a trap) go in `docs/api.md`
   with a date and a source, so nobody re-derives them.
4. Fixtures are scrubbed: keep the structure exact, replace every identifying
   value with the dummies described in `tests/fixtures/README.md`.
5. A new command validates its arguments before any credential is read,
   emits one `schema`-tagged DTO under `--json`, and maps every failure onto
   the family exit codes. A new write confirms (`--force`, exit 6
   non-interactively, before the network) and reads its result back.
6. Keep the `pk-cli-*` crates doing the shared work (output, errors, secrets,
   config, self-update); Govee-specific logic is the only thing that belongs
   here.
