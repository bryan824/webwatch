# Known Issues

## 2026-06-21 — Remote CloakBrowser over HTTPS needs WebSocket TLS

- Symptom: webwatch showed `renderer_enabled=true` and `renderer_configured=true`, but checks failed with `browser CDP connect failed: wss://cloakbrowser.kirinjade.com/...: URL error: TLS support not compiled in`.
- Cause: `reqwest` could discover `https://.../json/version`, but `tokio-tungstenite` was built without a TLS feature, so the returned `wss://.../devtools/browser/...` URL could not be opened.
- Fix: enable `tokio-tungstenite`'s `rustls-tls-webpki-roots` feature in `Cargo.toml`.
- Proof/regression: `cargo check`, `cargo test`, a `/targets/dry-run` data-URL render through `https://cloakbrowser.kirinjade.com`, and `GET /targets/cos-midi-shirt-dress-white-6-8/status` all passed after rebuilding/reloading.

## 2026-06-21 — COS page needs a longer initial render settle

- Symptom: after WebSocket TLS was fixed, the COS target initially failed with `browser CDP protocol failed: Runtime.evaluate: {"code":-32000,"message":"Inspected target navigated or closed"}`.
- Cause: the page was still navigating when the first render step ran with the default 750ms settle.
- Fix: set the COS target render plan to `wait_ms = 5000` before the selector/scenario steps.
- Proof/regression: the COS target status check completed with `engine_used = browser_cdp`, no `last_error`, and both configured size scenarios executed.
