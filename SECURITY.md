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
API key and the config file. Revoke a key server-side from the Govee Home app
(Profile > Settings > Apply for API Key) — a revoked key is exit 3 here.

`$GOVEE_API_KEY`, when set, is used instead of the keychain. Treat the
environment of any process that sets it accordingly.

The config file (`~/.config/govee/config.json`) records the account email and
whether a key is stored; it never contains a secret.

To report a vulnerability, open a private security advisory on the GitHub
repository rather than a public issue.
