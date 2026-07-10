# Verification Notes

## 2026-06-22 — Absolute recent-run timestamps

Scope verified: Watch detail Recent runs now shows absolute local date/time with timezone in a consistent 24-hour format instead of relative labels.

Commands:

- `cd web && npm run check` — TypeScript gate for the WatchDetail/format updates.
- `cd web && npm test` — includes `formatAbsolute` coverage proving consistent 24-hour absolute timestamp output and null/invalid fallbacks.
- `cd web && npm run build` — production Vite build for embedded SPA assets.
- `cargo test` — backend/embedded asset regression suite remains green: 78 passed, 2 ignored live tests.

## 2026-06-21 — Ops integration diagnostics

Scope verified: webwatch exposes safe server-side diagnostics for Discord webhook delivery and CloakBrowser/CDP renderer connectivity, and the Operations UI can trigger both checks without exposing webhook or CDP URLs.

Commands:

- `cargo test --test http_fixture -- --nocapture` — red/green coverage for the new diagnostic endpoints and UI/API route split; verifies Discord success, missing webhook diagnostic, failed-webhook URL redaction, HTTP CDP discovery, websocket connect, `Browser.getVersion` probe, and direct `/operations` SPA serving.
- `git diff --check` — no whitespace/conflict-marker issues.
- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — backend regression suite remains green: 74 passed, 2 ignored live tests.
- `cd web && npm run check` — TypeScript gate for the Operations UI/API type updates.
- `cd web && npm test` — Vitest regression suite remains green.
- `cd web && npm run build` — production Vite build for embedded SPA assets.
- Browser QA at `http://127.0.0.1:3000/operations` — verified the Operations page renders the Integrations controls without console/network/page errors, and the CDP test button reports `renderer connection verified`.

Accepted risks:

- The Discord test sends a real message when configured; this is intentional for end-to-end webhook verification.
- Diagnostic responses redact URL-like tokens but existing target check errors elsewhere may still include endpoint details; this slice only hardens the new Ops diagnostics surface.

## 2026-06-21 — Remote CloakBrowser HTTPS config fix

Scope verified: local webwatch config reaches `https://cloakbrowser.kirinjade.com`, follows CloakBrowser `/json/version` discovery to a `wss://` browser websocket, and the COS target uses the remote renderer without CDP TLS/connect errors.

Commands:

- `curl -i -L --max-time 15 'https://cloakbrowser.kirinjade.com/json/version?fingerprint=webwatch-smoke'` — remote CloakBrowser discovery returns JSON and a `wss://cloakbrowser.kirinjade.com/.../devtools/browser/...` URL.
- `cargo check` — compile gate after enabling `tokio-tungstenite` WebSocket TLS.
- `cargo test` — backend regression suite remains green: 69 passed, 2 ignored live tests.
- `curl -sS -X POST http://127.0.0.1:3000/targets/dry-run ...data URL...` — remote renderer returned `matched: true` and `engine_used: browser_cdp`, proving HTTPS discovery plus WSS connection works against a deterministic page.
- `curl -sS -X POST http://127.0.0.1:3000/targets/reload` — reloaded the local COS target update and reported it changed.
- `curl -sS http://127.0.0.1:3000/targets/cos-midi-shirt-dress-white-6-8/status` — live COS check completed with `engine_used: browser_cdp`, both size scenarios executed, and no `last_error`.
- `curl -sS http://127.0.0.1:3000/ops` — ops status shows `renderer_available: true`, `no_match: 1`, `error: 0`, and no recent errors.

Accepted risks:

- The COS target currently renders successfully but does not match `ADD TO BAG`; this is a product/selector/availability result, not renderer unavailability.
- The local `targets.toml` and `config.toml` are operator config files; only `targets.toml` was edited/reloaded locally because the DB is authoritative after first seed.

## 2026-06-18 — Frontend production redesign implementation

Scope verified: production-oriented frontend/backend contracts for persisted watch config read/update, evidence dry-run, ops status, recent check history, Solid edit flow, dry-run UI, ops page, README truth, and restored frontend check/test scripts.

Commands:

- `cd web && npm run check` — TypeScript gate for the Solid frontend.
- `cd web && npm test` — Vitest unit coverage for condition wire conversion and validation.
- `cd web && npm run build` — production Vite build for embedded SPA assets.
- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — includes API coverage for target detail, full update without duplicate creation, dry-run, ops status, and recent check history.
- `cargo check` — compile gate.

Accepted risks:

- Dry-run artifacts reuse the existing snapshot export mechanism under `dry-run-*` ids; persisted target state and alerts are not mutated.
- Visual selector/picker, screenshot diffs, bulk actions, snooze/ack alert policy, and real LLM-backed rule generation remain intentionally deferred by the redesign spec.

## 2026-06-15 — CloakBrowser renderer Slice 1

Scope verified: renderer config/policy model, legacy `[browser]` compatibility alias, `render_json` persistence/API round-trip, validation that rejects renderer steps/scenarios until the operations slice.

Commands:

- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — unit/integration coverage for config parsing/validation, persistence round-trip, API create round-trip, and existing monitor behavior.
- `cargo check` — compile gate.
- `cd web && npm run build` — frontend build still succeeds with the expanded status API shape.

Accepted risks:

- Renderer operation execution is intentionally not implemented in Slice 1; non-empty render steps/scenarios are rejected until the operations slice.
- CloakBrowser packaging and live browser checks are not verified in Slice 1.

## 2026-06-16 — CloakBrowser renderer Slice 2

Scope verified: first-party `RendererService`, CDP websocket discovery from CloakBrowser-style `/json/version`, direct `ws://` endpoint support, CDP response/event handling, command/navigation timeouts, and rendered HTML extraction. `browser::check_with_browser` now delegates to the renderer service while preserving the existing `EngineUsed::BrowserCdp` evaluator path.

Commands:

- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — includes fake CDP tests for query-preserving discovery, direct websocket endpoints, event skipping before responses, HTML extraction, command timeout, and navigation timeout.
- `cargo check` — compile gate.
- `cd web && npm run build` — frontend build remains green.

Accepted risks:

- The renderer service is still created on demand by the legacy browser fallback; Slice 3 will move orchestration to a shared check runner and enforce configured render policy across scheduled/manual checks.
- No live CloakBrowser container or retail page was exercised in Slice 2; verification uses deterministic fake HTTP/CDP servers.

## 2026-06-16 — CloakBrowser renderer Slice 3

Scope verified: check orchestration now lives in `check::check_target`, scheduled/manual checks share the scheduler-owned `RendererService`, and target render policies are enforced: `http_only`, `auto`, and `render_first`.

Commands:

- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — includes policy tests proving HTTP-only does not render, auto falls back to fake CDP for a JS shell, and render-first skips an unreachable HTTP URL and uses fake CDP.
- `cargo check` — compile gate.
- `cd web && npm run build` — frontend build remains green.

Accepted risks:

- Renderer steps/scenarios remain rejected until Slice 4.
- Live CloakBrowser and retail pages are still deferred to later verification slices.

## 2026-06-16 — CloakBrowser renderer Slice 4

Scope verified: render operation validation/execution, scenario execution and aggregation, and scenario-aware condition evidence.

Commands:

- `git diff --check` — no whitespace/conflict-marker issues.
- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — includes config tests for Jellycat-style steps/scenarios and max scenario count, plus fake-CDP tests for scenario aggregation where only one variant matches.
- `cargo check` — compile gate.
- `cd web && npm run build` — frontend build remains green after adding optional scenario fields to condition results.

Accepted risks:

- Operation execution is verified with deterministic fake CDP, not a real browser DOM. Live CloakBrowser and retail pages remain deferred to later verification slices.
- V1 `select` supports native `<select>` elements only; custom dropdowns may require a future operation or selector strategy.

## 2026-06-16 — CloakBrowser renderer Slice 5

Scope verified: deployment/docs now present CloakBrowser `cloakserve` as the optional packaged browser service, keep generic CDP endpoint documentation, and include Best Buy/Jellycat target examples.

Commands:

- `cargo fmt -- --check` — Rust formatting gate after docs/deployment changes.
- `cargo clippy --all-targets -- -D warnings` — lint gate remains green.
- `cargo test` — regression coverage remains green.
- `cargo check` — compile gate remains green.
- `cd web && npm run build` — frontend build remains green.
- `DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/example WEBWATCH_API_TOKEN=dev docker compose config` — compose renders for default webwatch service.
- `DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/example WEBWATCH_API_TOKEN=dev docker compose --profile browser config` — compose renders CloakBrowser service, healthcheck, and localhost-only CDP port.

Accepted risks:

- Docker images were not built or started in this slice; verification checked compose rendering only.
- CloakBrowser image tag is configurable through `CLOAKBROWSER_IMAGE`; the default uses the official Docker image name without a digest because the project has not selected a pinned release digest yet.
- Best Buy/Jellycat examples are documented starting points. Live selectors may need adjustment during Slice 6 against the real pages.

## 2026-06-16 — CloakBrowser renderer Slice 6

Scope verified: real CloakBrowser Docker backend, target fingerprint routing, browser-level CDP websocket attachment, health renderer reporting, and live Best Buy/Jellycat target cases.

Implementation fixes made while verifying live CDP:

- Target `fingerprint_seed` is now applied to HTTP CDP discovery by replacing/adding CloakBrowser's `fingerprint` query parameter while preserving other endpoint query parameters.
- Browser-level websocket endpoints from `/json/version` now work: the renderer creates, attaches to, and closes a page target with flattened CDP sessions before issuing `Page.*`/`Runtime.*` commands.
- If `Page.navigate` times out but the plan has bounded follow-up steps, the renderer records evidence, attempts `Page.stopLoading`, and lets the explicit wait steps prove or fail the page state. Plans with no follow-up steps still fail on navigation timeout.
- `/health` now reports `renderer_enabled`, `renderer_configured`, and `renderer_backend` without failing when the renderer is disabled.
- Added ignored live checks in `tests/live_renderer.rs` for reproducible CloakBrowser validation.

Commands:

- `docker run -d --name webwatch-cloak -p 127.0.0.1:9222:9222 cloakhq/cloakbrowser cloakserve --idle-timeout=300` — starts the approved Dockerized CloakBrowser backend on localhost-only CDP.
- `curl -fsS 'http://127.0.0.1:9222/json/version?fingerprint=smoke'` — verified CDP discovery returns `Browser: Chrome/146.0.7680.177` and a CloakBrowser websocket URL.
- `WEBWATCH_LIVE_CDP_ENDPOINT='http://127.0.0.1:9222' cargo test --test live_renderer -- --ignored --nocapture --test-threads=1` — 2 passed:
  - Best Buy open-box AirTag URL rendered through `browser_cdp` and matched page text `AirTag`.
  - Jellycat Bartholomew Bear rendered through `browser_cdp`, selected Tiny and Huge scenarios separately, and matched availability only for the Huge scenario with selector `#form-action-addToCart[value="Add to Bag"]`.
- `git diff --check` — no whitespace/conflict-marker issues.
- `cargo fmt -- --check` — Rust formatting gate.
- `cargo clippy --all-targets -- -D warnings` — lint gate across library, binary, and tests.
- `cargo test` — 61 passed, 2 ignored live tests.
- `cargo check` — compile gate.
- `cd web && npm run build` — frontend build remains green.
- `DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/example WEBWATCH_API_TOKEN=dev docker compose --profile browser config` — compose renders CloakBrowser service, healthcheck, and localhost-only CDP port.
- `kubectl apply --dry-run=client --validate=false -f deploy/kubernetes/webwatch-cloakbrowser.yaml` — Kubernetes sidecar example parses as Secret, ConfigMap, PVC, Deployment, and Service.

Accepted risks:

- The Best Buy rendered page did not expose stable `Open-Box` text during live verification, so the reproducible live check proves the specified open-box URL renders real product content by matching `AirTag` rather than asserting open-box availability semantics.
- Live retail pages are inherently brittle; the live checks are ignored by default and require `WEBWATCH_LIVE_CDP_ENDPOINT` plus internet access.
- CloakBrowser Docker image is still referenced by tag/name, not digest-pinned.
