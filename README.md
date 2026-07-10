# webwatch

Rust service for monitoring web pages and alerting when configured conditions become true.

Users configure URLs and alert conditions. The service prefers cheap HTTP checks and can optionally fall back to a CloakBrowser/CDP renderer for JavaScript-rendered pages.

## Run

```bash
cp config.toml.example config.toml
cp targets.toml.example targets.toml
export DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/...'
cargo run
```

Status API binds to `127.0.0.1:3000` by default. Override config path with `WEBWATCH_CONFIG=/path/to/config.toml cargo run` if needed. `/health` also reports the compiled persistence backend:

```bash
curl http://127.0.0.1:3000/health
curl http://127.0.0.1:3000/targets
curl http://127.0.0.1:3000/targets/campfire-mug/status
curl -X POST http://127.0.0.1:3000/notify/status

# Manage the watch list at runtime:
curl -X POST -H 'content-type: application/json' \
  -d '{"name":"Campfire Mug","url":"https://example.com/mug","conditions":[{"kind":"text_appears","value":"Add to cart"}]}' \
  http://127.0.0.1:3000/targets
curl http://127.0.0.1:3000/targets/campfire-mug
curl -X PUT -H 'content-type: application/json' \
  -d '{"name":"Campfire Mug","url":"https://example.com/mug","enabled":true,"interval_secs":900,"conditions":[{"id":"stock","kind":"text_appears","value":"Add to cart"}]}' \
  http://127.0.0.1:3000/targets/campfire-mug
curl -X POST -H 'content-type: application/json' \
  -d '{"name":"Draft Mug","url":"https://example.com/mug","conditions":[{"kind":"text_appears","value":"Add to cart"}]}' \
  http://127.0.0.1:3000/targets/dry-run
curl -X PATCH -H 'content-type: application/json' \
  -d '{"enabled":false}' http://127.0.0.1:3000/targets/campfire-mug
curl -X DELETE http://127.0.0.1:3000/targets/campfire-mug
```

When running normally, Discord receives an alert when a target transitions from not matched/unknown to matched.

Use the status API on demand to verify service health, target status, engine used, evidence, last success, last error, last alert, next approximate check, price, and URL.

Use `POST /notify/status` to fresh-check all enabled targets and send one compact status report to Discord. This is the webhook verification path; webwatch sends no startup or heartbeat messages.

The database is the source of truth for the watch list. `targets.toml` seeds it on first run (when empty); after that, add/remove/enable/disable targets from the web UI or the `/targets` API above. Editing `targets.toml` and importing it (upsert — never deletes targets added elsewhere) is still supported:

```bash
curl -X POST http://127.0.0.1:3000/targets/reload
```

Changes to `config.toml` require a process restart.

The HTTP API has no built-in authentication in this local/self-hosted build. Keep the default loopback bind or put webwatch behind a trusted reverse proxy/VPN before exposing it publicly.

## Web UI

A SolidJS dashboard (in `web/`) lists every target with latest status, evidence, conditions, errors, recent runs, renderer artifacts, and ops health. It can add a target with a supported condition builder, dry-run draft watches before saving, edit existing target config, delete one, enable/disable it, re-check a target, import `targets.toml`, and send a Discord report.

Development (two processes — the Vite dev server proxies the API):

```bash
cargo run                              # terminal 1: API on :3000
cd web && npm install && npm run dev   # terminal 2: UI on :5173
```

Production (single binary): the built SPA is embedded into the server via `rust-embed`, so build the frontend before the release binary:

```bash
cd web && npm run build && cd ..
cargo build --release
```

The UI is then served same-origin at http://127.0.0.1:3000/. The API owns `/targets`, so opening that bare URL returns JSON — the app itself lives at `/`, `/watches/<id>`, `/watches/new`, and `/ops`. (The Docker image builds and embeds the UI automatically.)

## Config

`config.toml` contains service settings. `targets.toml` contains the watch list. If upgrading from an older single-file config, move every `[[targets]]` block from `config.toml` into `targets.toml`.

V1 supports user-facing conditions:

- `text_appears`
- `text_disappears`
- `selector_exists`
- `selector_missing`
- `selector_text_contains`
- `selector_text_not_contains`
- `price_below`
- `price_above`
- `price_changed`

Example:

```toml
[[targets]]
id = "campfire-mug"
name = "Campfire Mug"
url = "https://example.com/products/campfire-mug"

[[targets.conditions]]
kind = "text_appears"
value = "Add to cart"

[[targets.conditions]]
kind = "price_below"
threshold_cents = 5000
price_selector = ".price"
```

All conditions on a target must match before an alert is sent.

## JavaScript-rendered pages

HTTP checks are tried first. If a page looks JavaScript-rendered and HTTP cannot prove a positive condition, webwatch can use an optional CDP renderer.

CloakBrowser is the packaged browser backend. It runs outside the Rust app and exposes Chrome DevTools Protocol through `cloakserve`:

```bash
docker run -d --name cloak -p 127.0.0.1:9222:9222 cloakhq/cloakbrowser cloakserve
```

Enable the renderer in `config.toml`:

```toml
[renderer]
enabled = true
backend = "cloakbrowser"
endpoint = "http://127.0.0.1:9222"
max_concurrency = 1
navigation_timeout_ms = 30000
operation_timeout_ms = 10000
settle_ms = 750
```

`endpoint` may also be a direct `ws://.../devtools/browser/...` CDP websocket URL. `http://...` endpoints use `/json/version` discovery, preserving CloakBrowser query parameters such as `?fingerprint=...&geoip=true`.

Per-target render policy:

```toml
[targets.render]
policy = "auto" # http_only | auto | render_first
```

- `http_only` never renders.
- `auto` tries HTTP first, then renders only when HTTP cannot prove a positive condition on a JavaScript shell.
- `render_first` skips HTTP and renders immediately; use it for interaction-dependent pages.

CloakBrowser is optional. HTTP-only targets do not require it.

For local debugging only, `agent-browser` can connect to the same CloakBrowser CDP service:

```bash
agent-browser connect 9222
agent-browser open https://bot.sannysoft.com
```

webwatch itself does not shell out to `agent-browser`, Playwright, Puppeteer, Python, or Node at runtime.

### Example: Best Buy JavaScript-rendered open-box page

```toml
[[targets]]
id = "bestbuy-airtag-open-box"
name = "Best Buy AirTag Open-Box"
url = "https://www.bestbuy.com/product/apple-airtag-4-pack-1st-generation-2021-silver/JJGCQ8XFQH/sku/6461349/openbox?condition=good"
enabled = true

[targets.render]
policy = "render_first"
fingerprint_seed = "bestbuy-airtag-open-box"
wait_ms = 3000

[[targets.conditions]]
id = "product-title"
kind = "text_appears"
value = "AirTag"
```

### Example: Jellycat size/variant availability

Use scenarios when one page has several variants to check separately. Each scenario reloads the page, runs base steps, then scenario-specific steps before evaluating the target conditions.

```toml
[[targets]]
id = "jellycat-bartholomew-bear"
name = "Jellycat Bartholomew Bear"
url = "https://us.jellycat.com/bartholomew-bear/"
enabled = true

[targets.render]
policy = "render_first"
scenario_match = "any"
fingerprint_seed = "jellycat-bartholomew-bear"
wait_ms = 1000

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
selector = '#form-action-addToCart[value="Add to Bag"]'
```

V1 render operations are intentionally small: `wait_for`, `wait_for_text`, `click`, and native-`select` selection. If a site uses a custom dropdown rather than a `<select>`, model it with safe selectors/`click` steps and avoid checkout/cart actions.

Live renderer checks are ignored by default. With CloakBrowser running locally, run:

```bash
WEBWATCH_LIVE_CDP_ENDPOINT=http://127.0.0.1:9222 \
  cargo test --test live_renderer -- --ignored --nocapture --test-threads=1
```

## Docker

### Local compose

```bash
cp config.docker.toml.example config.docker.toml
cp targets.toml.example targets.toml
export DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/...'
docker compose up --build
```

Compose publishes only `127.0.0.1:3000` on host.

To include CloakBrowser for browser-capable targets, set `enabled = true` in `config.docker.toml`'s `[renderer]` block, then run:

```bash
docker compose --profile browser up --build
```

Compose publishes CloakBrowser CDP only on `127.0.0.1:9222` on the host. Inside compose, webwatch reaches it at `http://cloakbrowser:9222`.

### Kubernetes

A starter single-pod Deployment with a CloakBrowser sidecar lives at `deploy/kubernetes/webwatch-cloakbrowser.yaml`:

```bash
kubectl apply -f deploy/kubernetes/webwatch-cloakbrowser.yaml
```

Edit the image, secret values, storage class/size, and ingress/service exposure before applying. The manifest exposes only webwatch's HTTP service; CloakBrowser CDP stays pod-local at `http://127.0.0.1:9222`.

### GHCR publish

`webwatch` publishes to GHCR via `.github/workflows/docker-publish.yml` on:
- pushes to `main`
- manual `workflow_dispatch`

Image tags include:
- `latest`
- `sha-<commit>` (short SHA tag)

Published image path:

```text
ghcr.io/<github-owner>/webwatch
```

The workflow uses `GITHUB_TOKEN` with `packages:write` permission. For private images, deploy environments need GHCR pull auth (`read:packages`).

Pull a published image from GHCR:

```bash
docker pull ghcr.io/<github-owner>/webwatch:latest
docker pull ghcr.io/<github-owner>/webwatch:sha-<commit>
```

If the package is private, authenticate for pulls (`read:packages`) or make the package public in GitHub Packages.

## Persistence

webwatch stores targets and check history in a local SQLite database (via Diesel) at the `sqlite_path` from `config.toml`. The schema is created automatically on first run; no manual migration step is needed.

## Target files

The database is authoritative for the watch list. `targets.toml` seeds it on first run (when the database has no targets); afterward, manage targets through the web UI or the `/targets` API, and use `POST /targets/reload` to re-import the file (upsert — it never deletes targets added elsewhere). A missing or empty `targets.toml` is fine once the database is populated.

Targets use full URLs. Keyword/search-based targets are deferred. Set `targets_path` in `config.toml` or `WEBWATCH_TARGETS` to use a non-default seed file. Relative `targets_path` values resolve relative to `config.toml`.

Default scheduler: 5 minutes plus ±30 seconds jitter.

## Ethics / limits

- Low-frequency jittered polling.
- JavaScript rendering is optional and should be used only when HTTP cannot evaluate a page.
- Respect site terms and robots.txt where applicable.
- CDP gives full browser control; never expose CloakBrowser/CDP to the public internet without additional authentication and network controls.
- CloakBrowser's wrapper source and browser binary have different license terms. webwatch documents and connects to the official CloakBrowser image; it does not bundle the CloakBrowser binary into the webwatch image.
- No CAPTCHA solving, checkout automation, payment automation, account creation, credential stuffing, or login automation in v1.
