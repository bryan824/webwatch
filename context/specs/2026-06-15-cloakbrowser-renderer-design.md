# CloakBrowser Renderer Design

Date: 2026-06-15

## Goal

Make webwatch able to evaluate JavaScript-rendered and interaction-dependent product pages through a first-party, narrow browser renderer. The selected product direction is **CloakBrowser only for packaged browser operation**, while preserving a generic CDP seam so webwatch remains a Rust monitor rather than a browser-automation framework.

The feature must eventually prove two live cases:

1. Best Buy open-box AirTag 4-pack page: JavaScript-rendered product/offer content.
2. Jellycat Bartholomew Bear page: choose size/variant and evaluate availability separately per size.

## Current state

- `src/evaluator.rs` performs HTTP first and falls back to `browser::check_with_browser` only when HTTP evaluation returns `BrowserRequired` and global `config.browser.cdp_url` is present.
- `src/browser.rs` is already a small CDP client: connect to a configured websocket, `Page.navigate`, fixed sleep, then read `document.documentElement.outerHTML`.
- `src/config.rs` exposes only global `BrowserConfig { cdp_url, wait_ms }`.
- `docker-compose.yml` currently packages Lightpanda behind a `browser` profile.

This is the right foundation but too implicit: browser rendering is global, fixed-wait only, no target policy exists, no CDP discovery from `http://.../json/version` exists, and no interaction/scenario model exists for variant pages.

## Decision

Use CloakBrowser as the only first-class packaged browser backend.

Why:

- CloakBrowser is Chromium-based, so it closes more site-compatibility gaps than Lightpanda for modern commerce pages.
- It provides `cloakserve`, a CDP server/multiplexer that can run as a sidecar service and expose `/json/version` plus websocket proxying.
- It supports fingerprint seeds, per-connection identity parameters, proxy/geoip/headed settings, and persistent profiles, which are the likely knobs for protected retail pages.
- Lightpanda is fast and attractive, but its beta Web/API coverage makes it an optimization path, not the primary product path for the two target cases.
- `agent-browser` remains useful as a local debugging companion, but it should not be in the production runtime chain.

The runtime contract is still **CDP**, not a Python/Node/Playwright API. webwatch talks directly to `cloakserve`/CDP.

## Non-goals

- Do not add `agent-browser`, Playwright, Puppeteer, Python, or Node to the webwatch runtime.
- Do not bundle the CloakBrowser binary inside the webwatch image.
- Do not solve CAPTCHA challenges, perform checkout/cart operations, log into accounts, or bypass authentication.
- Do not expose CDP outside the local machine/cluster.
- Do not add arbitrary JavaScript scripting to user config in v1.
- Do not make all targets render by default; cheap HTTP checks remain the default.
- Do not keep Lightpanda as a first-class packaged backend in this design. A generic CDP URL is enough escape hatch.

## Licensing and distribution constraint

CloakBrowser wrapper source is MIT, but the distributed Chromium binary has a separate binary license. The design must avoid redistributing or embedding the binary in webwatch artifacts.

Allowed product shape:

- document CloakBrowser as an optional dependency,
- provide Docker Compose / Kubernetes examples that reference the official `cloakhq/cloakbrowser` image,
- allow users to set an external CDP endpoint.

Avoid:

- copying the CloakBrowser binary into the webwatch image,
- publishing a combined image with the binary preinstalled,
- offering hosted browser rendering to third parties without reviewing CloakBrowser OEM/SaaS terms.

## Target model

Excellent six-month target:

```text
Target check
  -> HTTP fetch/evaluate if policy allows
  -> BrowserRenderer if target requires rendering or HTTP is inconclusive
       -> discover CDP endpoint from CloakBrowser cloakserve
       -> create/attach isolated page/session
       -> navigate
       -> run safe declarative steps
       -> snapshot rendered HTML
       -> repeat per scenario when needed
  -> pure evaluator evaluates HTML snapshot(s)
  -> status/evidence records engine + scenario/variant evidence
```

webwatch owns the monitor semantics. CloakBrowser owns browser fidelity.

## Configuration

### Global renderer config

Replace/deprecate `[browser]` with `[renderer]`, while keeping `[browser]` as a compatibility alias during migration.

```toml
[renderer]
enabled = true
backend = "cloakbrowser"
endpoint = "http://cloakbrowser:9222"
max_concurrency = 1
navigation_timeout_ms = 30000
operation_timeout_ms = 10000
settle_ms = 750
```

Rules:

- `backend` initially accepts only `cloakbrowser` and `cdp`.
- `cloakbrowser` is a documented preset over CDP discovery and diagnostics.
- `endpoint` accepts either:
  - `http://host:9222` or `http://host:9222?fingerprint=...` for `/json/version` discovery, or
  - direct `ws://.../devtools/browser/...` websocket URLs.
- `max_concurrency` must gate all browser renders.
- Normal `/health` should report renderer configuration/availability separately but not fail the app when the renderer is disabled.

### Per-target render policy

```toml
[[targets]]
id = "bestbuy-airtag-open-box"
name = "Best Buy AirTag Open-Box"
url = "https://www.bestbuy.com/product/apple-airtag-4-pack-1st-generation-2021-silver/JJGCQ8XFQH/sku/6461349/openbox?condition=good"

[targets.render]
policy = "render_first" # http_only | auto | render_first
fingerprint_seed = "bestbuy-airtag-open-box"
wait_ms = 3000
```

Policy semantics:

- `http_only`: never use renderer.
- `auto`: HTTP first; render only when static evaluation is inconclusive or detects a JS shell.
- `render_first`: skip HTTP and render immediately. Use this for protected or interaction-dependent pages.

Any target with render steps or scenarios is renderer-required even if policy is omitted.

## Declarative operations

Add a small DSL for browser operations. It should compile into internal CDP `Runtime.evaluate` calls, but config should not contain arbitrary JS.

Supported v1 operations:

```toml
[[targets.render.steps]]
op = "wait_for"
selector = "body"
timeout_ms = 20000

[[targets.render.steps]]
op = "click"
selector = "button, [role='button']"
text = "Accept"
settle_ms = 750

[[targets.render.steps]]
op = "select"
selector = "select[name*='Size']"
option_text = "Huge"
settle_ms = 750

[[targets.render.steps]]
op = "wait_for_text"
text = "Bartholomew Bear"
timeout_ms = 20000
```

Operation rules:

- Element-targeting operations use CSS selectors only.
- `wait_for_text` is the only text-only operation in v1.
- `click` and `select` support optional text filters.
- refuse dangerous text/actions in v1: add to cart, checkout, place order, buy now, payment, submit order.
- operations are bounded by `operation_timeout_ms`.
- each step contributes diagnostic evidence on failure.

## Scenarios for variants

Jellycat requires checking availability after choosing sizes. Model this as scenarios, not separate unrelated targets.

```toml
[[targets]]
id = "jellycat-bartholomew-bear"
name = "Jellycat Bartholomew Bear"
url = "https://us.jellycat.com/bartholomew-bear/"

[targets.render]
policy = "render_first"
scenario_match = "any" # any | all
fingerprint_seed = "jellycat-bartholomew-bear"

[[targets.render.steps]]
op = "wait_for_text"
text = "Bartholomew Bear"
timeout_ms = 20000

[[targets.render.scenarios]]
id = "medium"
label = "Medium"

[[targets.render.scenarios.steps]]
op = "select"
selector = "select[name*='Size'], select[id*='Size']"
option_text = "Medium"
settle_ms = 1000

[[targets.render.scenarios]]
id = "huge"
label = "Huge"

[[targets.render.scenarios.steps]]
op = "select"
selector = "select[name*='Size'], select[id*='Size']"
option_text = "Huge"
settle_ms = 1000

[[targets.conditions]]
id = "available"
kind = "selector_exists"
selector = "button[type='submit']:not([disabled]), button[data-add-to-cart]:not([disabled])"
```

Scenario behavior:

- Each scenario starts from a fresh page load unless a later performance slice proves shared-page scenarios safe.
- Base render steps run first, then scenario-specific steps.
- Existing conditions evaluate against each scenario snapshot.
- `scenario_match = "any"` means the target matches if any scenario satisfies all conditions.
- Evidence must identify the scenario/variant that matched or failed.

## Data model changes

Add to `Target`:

- `render: RenderPlan` with default empty plan.

Add types:

- `RenderPlan`
- `RenderPolicy`
- `RenderStep`
- `RenderScenario`
- `ScenarioMatch`

Extend condition/status evidence:

- optional `scenario_id`
- optional `scenario_label`

Persistence:

- add `render_json` to stored targets.
- prefer additive migration over destructive schema reset.

## Renderer module design

Create `src/renderer/` and move/split the current `src/browser.rs` code into it.

Suggested modules:

```text
src/renderer/mod.rs
src/renderer/config.rs       # normalized renderer/target render config helpers, if useful
src/renderer/cdp.rs          # CdpClient, command timeout, discovery
src/renderer/service.rs      # RendererService, semaphore, render orchestration
src/renderer/steps.rs        # safe operation DSL executor
src/renderer/scenarios.rs    # scenario execution and aggregation
```

Core contracts:

```rust
pub struct RenderRequest {
    pub target_id: String,
    pub url: String,
    pub plan: RenderPlan,
}

pub struct RenderedSnapshot {
    pub final_url: String,
    pub html: String,
    pub scenario_id: Option<String>,
    pub scenario_label: Option<String>,
    pub evidence: Vec<String>,
}

pub struct RendererService { /* shared config + semaphore */ }
```

CDP requirements:

- discover websocket via `/json/version` for CloakBrowser `http://...` endpoints.
- preserve endpoint query parameters such as `fingerprint`, `proxy`, `geoip`, `timezone`, `locale`.
- command-level timeouts.
- event skipping until matching response id.
- best-effort page/session cleanup.
- blocked-page detection for Cloudflare/security verification, reported as a renderer error/evidence instead of false availability.

## Deployment design

Replace the current Lightpanda compose profile with a CloakBrowser profile.

```yaml
services:
  cloakbrowser:
    image: cloakhq/cloakbrowser:<pinned-tag-or-digest>
    profiles: ["browser"]
    command: ["cloakserve", "--idle-timeout=300"]
    ports:
      - "127.0.0.1:9222:9222"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9222/json/version"]
```

Kubernetes/Helm shape:

- optional Deployment/Service for `cloakbrowser`, disabled by default.
- ClusterIP only; no public ingress.
- resource limits; start around CloakBrowser documented idle/tab usage and tune from live tests.
- optional values for `headless=false`, proxy, geoip, timezone/locale, idle timeout, fingerprint seed policy.

## Live testcase strategy

Live tests should be ignored by default and opt-in:

```bash
WEBWATCH_LIVE=1 \
WEBWATCH_RENDERER_ENDPOINT=http://127.0.0.1:9222 \
cargo test --test live_renderer -- --ignored
```

Test 1: Best Buy

- target policy: `render_first`.
- wait for product/offer content.
- assert the rendered snapshot is not a JS shell/security page.
- assert configured condition can be evaluated from rendered content.

Test 2: Jellycat

- target policy: `render_first`.
- scenario per size.
- select size/variant.
- assert availability condition is evaluated separately per scenario.
- if Cloudflare/security verification appears, classify as `BlockedBySecurityVerification` with evidence; the strict live gate remains red until CloakBrowser configuration makes it pass.

## Slices

### Slice 1 — config, policy, and persistence model

- Add `[renderer]` config and target `render` policy fields.
- Keep `[browser]` compatibility alias.
- Add `render_json` persistence and API round-trip for target render policy.
- Add validation for renderer config and policies.
- Explicitly reject non-empty steps/scenarios until Slice 4 lands.
- Tests for parsing/serialization/validation and persistence round-trip.

Success criteria:

- Existing configs still parse.
- New CloakBrowser renderer config parses.
- Best Buy target snippet with `render_first` policy parses.
- Targets with steps/scenarios fail with a clear "not implemented until operations slice" error.
- Stored targets preserve render policy through persistence/API round-trip.

### Slice 2 — CDP discovery and renderer service

- Build `RendererService` over current CDP client.
- Add `http://.../json/version` discovery and direct `ws://...` support.
- Add command/navigation/render timeouts and semaphore.
- Keep output as rendered HTML snapshots.

Success criteria:

- Fake CDP tests prove discovery, event skipping, timeout, and HTML extraction.
- No dependency on Python/Node/Playwright/Puppeteer/agent-browser.

### Slice 3 — check orchestration and policy

- Move HTTP-vs-render orchestration out of pure document evaluation.
- Implement `http_only`, `auto`, and `render_first`.
- Record renderer errors distinctly.

Success criteria:

- HTTP-only behavior remains unchanged.
- Auto fallback still works for JS-shell pages.
- Render-first skips HTTP and uses renderer.

### Slice 4 — steps and scenarios

- Add and validate operation/scenario config types.
- Implement `wait_for`, `wait_for_text`, `click`, and `select`.
- Implement scenario aggregation.
- Extend condition evidence with scenario labels.
- Enforce a bounded scenario count, with default max 10 unless configured lower.

Success criteria:

- Local fixture page can select variants and evaluate each separately.
- Jellycat config parses and can model size-specific availability without arbitrary JS.
- Over-large scenario lists are rejected with a clear validation error.

### Slice 5 — deployment and docs

- Replace Lightpanda-first docs/compose with CloakBrowser-first optional browser profile.
- Document external CDP endpoint escape hatch.
- Add examples for the two live testcases.

Success criteria:

- `docker compose --profile browser up --build` starts webwatch plus CloakBrowser.
- README clearly explains licensing/distribution boundaries and CDP security.

### Slice 6 — live verification

- Add ignored live tests for Best Buy and Jellycat.
- Build live-test config from test-only env vars such as `WEBWATCH_RENDERER_ENDPOINT`; do not imply runtime env support unless separately implemented.
- Run against local CloakBrowser `cloakserve`.
- Capture blocked/security states with explicit evidence.

Success criteria:

- Both live URLs can be checked without false positives.
- Passing state is either evaluated availability or an explicit blocked/security failure, depending on configured strictness.
- For final goal completion, the strict gate should evaluate real availability, not merely classify blocked.

## Risks and falsifiers

- **CloakBrowser binary license:** if distribution terms prevent even optional packaged compose/chart use, keep only external endpoint documentation.
- **Anti-bot reality:** CloakBrowser docs recommend residential proxy, `geoip`, `headless=false`, and `humanize` for aggressive sites. Pure CDP usage gets fingerprint patches, but wrapper-level humanize may not apply to our direct CDP operations. If the Jellycat/Best Buy live cases still block, the in-scope response is to tune the CloakBrowser CDP deployment first: headed/Xvfb, proxy, geoip, stable fingerprint seed, locale/timezone, and low frequency. A Python/JS wrapper companion would be a new product mode requiring explicit reapproval, not part of this design.
- **CDP over cloakserve params:** webwatch must preserve query parameters through discovery; losing `fingerprint`/`proxy`/`geoip` would silently degrade the backend.
- **Site terms/rate limits:** monitoring must stay low-frequency and user-configured.
- **Scenario explosion:** scenarios multiply browser work; enforce max scenarios or document resource implications.

## Rejected options

- **Use `agent-browser` in production:** rejected because it adds a CLI/session layer optimized for AI interaction, not low-frequency monitor rendering.
- **Keep Lightpanda as primary:** rejected for the current testcases because compatibility and anti-bot fidelity matter more than speed.
- **Embed Playwright/Puppeteer in webwatch:** rejected because it makes the Rust app depend on Node/Python runtime complexity.
- **Arbitrary JS user scripts:** rejected for safety, supportability, and product clarity.

## Evidence notes

- CloakBrowser claims source-level Chromium fingerprint patches, Cloudflare/FingerprintJS test results, and CDP server mode via `cloakserve`.
- CloakBrowser binary licensing distinguishes MIT wrapper code from a proprietary binary license and restricts redistribution/bundling.
- Lightpanda documents strong speed/memory advantages, but also beta status and incomplete Web API coverage.
- webwatch already has a small CDP path suitable to evolve rather than replacing it with a general automation tool.
