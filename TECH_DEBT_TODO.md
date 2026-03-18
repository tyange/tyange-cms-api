# Tech Debt TODO

## Replace OpenSSL-backed web push dependency

- Current web push stack uses `web-push` -> `ece` -> OpenSSL, which requires local OpenSSL setup on Windows and can block `rust-analyzer` / `cargo check` in fresh environments.
- Investigate replacing `web-push` with `web-push-native` plus direct `reqwest` sending.
- Expected migration shape:
  - Replace `web-push` dependency in `Cargo.toml`
  - Refactor push send path in `src/rss_push.rs`
  - Build HTTP push request via `web-push-native::WebPushBuilder`
  - Send resulting request with `reqwest`
  - Preserve current revoke handling for `404` / `410` responses
- Why this is desirable:
  - Reduce or remove OpenSSL dependency from the push pipeline
  - Make Windows and CI setup simpler
  - Keep the existing RSS push feature while improving portability
