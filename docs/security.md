# Security

## Credential boundary

- `USERVOICE_API_KEY` is read only from the process environment. It is never accepted as a CLI argument, serialized to disk, logged, or included in MCP output.
- `AccessToken` wraps the token in `Arc<str>` with `Debug`/`Display` redacted to `[REDACTED]`.
- The token value is only passed to reqwest's `bearer_auth()` call inside `ReqwestUserVoiceClient`.
- `.env.test` must have mode `0600` and is never read by the tool itself.

## Network boundary

- Outbound calls use TLS to `https://<tenant>.uservoice.com/api/v2/admin/...`.
- HTTP is permitted only for loopback hosts (127.0.0.1, localhost, ::1) in test mode.
- MCP transport is stdio only. No inbound HTTP server, OAuth callback, or browser automation.
- Corporate SSO, SAML assertions, RuToken, and browser session cookies are never accessed.

## Mutation safety

- Every mutation requires a preview phase that returns a `change_id` with 15-minute TTL.
- Apply rejects stale previews via optimistic `updated_at` freshness checks.
- Apply requires literal `confirmation="apply"` and a caller-generated `idempotency_key`.
- Partial writes are completed in the ledger without compensation or rollback.
- Idempotency keys are SHA-256 hashed before storage; the original key is never logged.

## Local state

- The state directory (`USERVOICE_STATE_DIR`) contains only:
  - Preview records (change_id, kind, fingerprint, timestamps)
  - Idempotency records (status, fingerprint, redacted result)
  - Collection checkpoints (continuation tokens)
- No access token, feedback body, comment text, or email address is stored locally.
- Ledger files use atomic write (write to `.tmp`, then rename).

## Logging

- Structured tracing outputs to stderr only.
- Logs contain: operation name, opaque idea ID, outcome, duration, rate-limit state, counts.
- Logs never contain: token values, email addresses, message/comment bodies, or raw API responses.
- The program never prints `.env.test` contents.

## Testing

- Default CI and `cargo test --workspace` use fixture data only.
- Live smoke tests are `#[ignore]`d and require explicit `--ignored` flag.
- Test fixtures use `example.invalid` domains and non-existent tokens.
