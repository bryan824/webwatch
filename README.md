# webwatch

Rust service for monitoring web pages and alerting when configured conditions become true.

Users configure URLs and alert conditions. The service prefers cheap HTTP checks and can optionally fall back to a Lightpanda/CDP browser endpoint for JavaScript-rendered pages.

## Run

```bash
cp config.toml.example config.toml
cp targets.toml.example targets.toml
export DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/...'
export WEBWATCH_API_TOKEN='choose-a-token'
cargo run
```

Status API binds to `127.0.0.1:3000` by default. Override config path with `WEBWATCH_CONFIG=/path/to/config.toml cargo run` if needed. `/health` also reports the compiled persistence backend:

```bash
curl http://127.0.0.1:3000/health
curl http://127.0.0.1:3000/targets
curl http://127.0.0.1:3000/targets/campfire-mug/status
curl -X POST -H "Authorization: Bearer $WEBWATCH_API_TOKEN" http://127.0.0.1:3000/notify/status
```

When running normally, Discord receives an alert when a target transitions from not matched/unknown to matched.

Use the status API on demand to verify service health, target status, engine used, evidence, last success, last error, last alert, next approximate check, price, and URL.

Use `POST /notify/status` to fresh-check all enabled targets and send one compact status report to Discord. This is the webhook verification path; webwatch sends no startup or heartbeat messages.

If binding publicly, set `WEBWATCH_API_TOKEN`; protected endpoints require it. `POST /notify/status` always requires it:

```bash
curl -H "Authorization: Bearer $WEBWATCH_API_TOKEN" http://127.0.0.1:3000/targets
```

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

HTTP checks are tried first. If a page looks JavaScript-rendered and HTTP cannot prove a positive condition, webwatch can use an optional CDP browser endpoint.

Lightpanda is the intended lightweight browser backend. Uncomment the `[browser]` block in `config.toml` and run a Lightpanda container:

```bash
docker run -p 127.0.0.1:9222:9222 lightpanda/browser:nightly
cargo run
```

Lightpanda is optional and runs outside the Rust app. HTTP-only targets do not require it.

## Docker

### Local compose

```bash
cp config.docker.toml.example config.docker.toml
cp targets.toml.example targets.toml
export DISCORD_WEBHOOK_URL='https://discord.com/api/webhooks/...'
docker compose up --build
```

Compose publishes only `127.0.0.1:3000` on host.

To include Lightpanda for browser-capable targets:

```bash
docker compose --profile browser up --build
```

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

## Persistence backend

Diesel is the default backend:

```bash
cargo build
```

Build another backend by disabling defaults and enabling exactly one feature:

```bash
cargo build --no-default-features --features persistence-sqlx
cargo build --no-default-features --features persistence-seaorm
```

Enabling multiple persistence features fails at compile time.

## Target files

Targets use full URLs. Keyword/search-based targets are deferred. Set `targets_path` in `config.toml` or `WEBWATCH_TARGETS` to use a non-default watch-list file. Relative `targets_path` values resolve relative to `config.toml`.

Default scheduler: 5 minutes plus ±30 seconds jitter.

## Ethics / limits

- Low-frequency jittered polling.
- JavaScript rendering is optional and should be used only when HTTP cannot evaluate a page.
- Respect site terms and robots.txt where applicable.
- No CAPTCHA bypass, proxy evasion, checkout automation, or login automation in v1.
