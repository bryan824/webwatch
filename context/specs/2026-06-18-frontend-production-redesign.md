# Frontend Production Redesign

Date: 2026-06-18
Status: Implemented baseline — visual selector/picker and other deferred items remain out of scope

## Goal

Make the Webwatch frontend production-grade by combining two directions:

1. **Evidence-first builder** — users should prove a watch works before trusting it.
2. **Ops-grade dashboard** — operators should understand service health, renderer state, failures, and check behavior without reading logs.

Visual selector/picker work is explicitly deferred. The near-term product should win through honest evidence, clear contracts, and operational trust, not another prototype UI pass.

## Current-state evidence

The current Solid frontend is useful but not mature enough to call production-grade:

- It is a recent prototype/migration convergence. `docs/design/2026-06-09-solid-frontend.md` replaced the SvelteKit app with SolidJS to adopt the Watchtower aesthetic and NL-first direction, but also deferred dry-run, real NL, cost economics, and catalog/backend alignment.
- Git history shows the migration commit `24f4f18` deleted the SvelteKit/shadcn test setup and reduced web scripts to `dev`, `build`, and `preview`. Current `web/package.json` has no `check` or `test` script.
- `web/src/components/BuilderPane.tsx` presents create/edit affordances, but edit is not a real full edit: it seeds from status data, loses existing config details, and save calls the create mutation. Backend `PATCH /targets/:id` only accepts `{ enabled }`.
- `web/src/lib/nl.ts` is a mock keyword interpreter with hardcoded guesses such as `.price` and `.availability`. It should not be presented as production AI.
- `web/src/components/ConditionCard.tsx` exposes XPath and strategy controls, but the backend condition path is CSS-selector based and those strategy values are not persisted.
- The frontend condition catalog can drift from backend-supported wire kinds. Unsupported options are visible then rejected, instead of the UI being generated from or constrained by backend capabilities.
- README/docs are stale and conflicting: README still describes SvelteKit, a token/gear flow, and old route assumptions, while the current app is Solid with `/watches/$id` routes and no frontend bearer-token handling.
- The app shows latest evidence and latest snapshot links, but it lacks run history, dry-run diagnostics, typed failure reasons, alert controls, and renderer/scheduler status.

## Market evidence

Competitors set the expectation that a monitoring UI proves what it is watching:

- changedetection.io documents a visual selector with click-to-select, live preview, generated selectors, browser fetching prerequisites, manual CSS/XPath refinement, and test/save flow: https://dgtlmoon-changedetection-io.mintlify.app/features/visual-selector
- Distill's visual selector flow includes multiple selections, expand/narrow, exclusions, selector preview, regex filters, and attribute/property monitoring: https://distill.io/docs/web-monitor/what-is-visual-selector/
- Visualping pushes natural-language alert importance and muting non-important alerts: https://visualping.io/ai
- Browse AI emphasizes training, custom schedules, history, color-coded change evidence, integrations, and operator-facing scale/workflow concepts: https://www.browse.ai/monitor

For Webwatch, the transferable pattern is not “copy a visual selector now.” It is: **never ask users to trust hidden extraction. Show the captured evidence, selector hit count, engine used, failure reason, and notification consequence before and after saving.**

## Decision

Build toward an **Evidence-first Watch Lifecycle**:

```text
Create/edit watch
  -> URL and render policy
  -> supported rule builder
  -> dry-run against current page
  -> review evidence and diagnostics
  -> save verified watch
  -> dashboard/detail show latest run, next run, failures, snapshots, and service health
```

The frontend should become a control room for known monitor semantics, not a broad scraping/automation suite.

## Non-goals

- No visual selector/picker in this spec.
- No real LLM/NL backend in the first slice. NL may remain a clearly labeled helper/stub only if it cannot be mistaken for production AI.
- No multi-user SaaS features, roles, teams, or billing.
- No arbitrary browser automation, checkout/cart operations, CAPTCHA solving, account login flows, or anti-bot bypass UX.
- No full workflow/integration platform like Browse AI/Hexomatic.
- No stack migration. Keep SolidJS unless verification proves it blocks production readiness.

## Product principles

1. **Evidence before trust** — a watch is not “configured” until the UI can show what was fetched, extracted, matched, and why.
2. **Honest affordances only** — do not expose edit, XPath, strategy, AI, or condition operations unless the backend contract can execute and persist them.
3. **Operator visibility beats polish** — users need next check, last duration, engine, renderer health, queue/failure state, and typed errors more than visual flourish.
4. **HTTP-first remains visible** — the UI should show when a cheap HTTP check was enough and when browser rendering was required.
5. **Safe monitoring, not botting** — browser steps stay declarative, bounded, and auditable.

## Target feature model

### 1. Production dashboard

The home screen should answer:

- What is matched, broken, unknown, disabled, or stale?
- Which watches are due next?
- Is the scheduler running?
- Is the renderer enabled/configured/reachable?
- Did Discord/notification delivery last succeed?
- Which watches failed recently and why?

Recommended dashboard sections:

- **Status rail**: counts for matched, no-match, error, unknown, disabled.
- **Watch list**: name, URL/domain, status, enabled, next check, last check age, last duration, engine used.
- **Ops strip**: API healthy, DB backend/path if available, scheduler state, renderer state, notification state.
- **Recent failures**: typed reasons with direct action links.

### 2. Evidence-first builder

Replace the current implicit builder behavior with a four-step flow:

1. **Target**
   - URL, name, enabled, interval.
   - Render policy: `http_only`, `auto`, `render_first`.
   - Show whether renderer is available before allowing render-first confidence.

2. **Rules**
   - Only show backend-supported condition subjects/ops.
   - Manual selectors remain allowed, but CSS-only until backend supports more.
   - Condition summaries should read like predicates: `Page text contains "Add to cart"`, `Element .price drops below $25.00`.
   - If NL helper remains, label it as `suggest rules` / `local heuristic`, not AI.

3. **Dry-run**
   - Run the draft watch without saving.
   - Show engine used, final URL, duration, matched/not matched, selector hit counts, extracted text/value, parsed price, condition-level evidence, and typed errors.
   - Show links to generated HTML/screenshot artifacts when available.
   - Let users save untested only through an explicit warning path; the primary path is verified save.

4. **Review + save**
   - Confirm what will alert and when.
   - Preview notification text for Discord/status report.
   - Save creates or updates the DB-authoritative watch.

### 3. Real edit/update contract

Current edit must not create duplicate watches or lose existing config. Production edit requires a backend contract that returns and updates watch configuration, not just status.

Recommended API shape:

```http
GET /targets/:id
PUT /targets/:id
POST /targets/dry-run
POST /targets/:id/dry-run
```

`GET /targets/:id` returns persisted config plus latest status. `PUT /targets/:id` replaces editable config. Existing `PATCH /targets/:id` can remain the small enabled toggle.

Draft types:

```ts
interface WatchConfig {
  target_id: string;
  name: string;
  url: string;
  enabled: boolean;
  interval_secs: number;
  render: RenderPlan;
  conditions: ConditionInput[];
}

interface WatchDetailResponse {
  config: WatchConfig;
  status: TargetStatus;
}

interface DryRunRequest {
  target_id?: string;
  name?: string;
  url: string;
  render: RenderPlan;
  conditions: ConditionInput[];
}

interface DryRunResponse {
  matched: boolean | null;
  engine_used: EngineUsed;
  duration_ms: number;
  final_url: string | null;
  evidence: string[];
  condition_results: ConditionResult[];
  diagnostics: DryRunDiagnostic[];
  artifacts: {
    html_url?: string;
    screenshot_url?: string;
  };
  error: string | null;
}
```

### 4. Watch detail as runbook

The detail pane should become the place to debug a monitor:

- Current state: status, enabled, engine, price, last/next check, last alert.
- Config summary: URL, interval, render policy, safe render steps/scenarios, rules.
- Last run: condition evidence, selector diagnostics, artifact links.
- Recent runs: short timeline with status, engine, duration, error, alert sent.
- Actions: check now, dry-run config, edit, enable/disable, delete, test notification.

### 5. Ops panel

Add an operator-facing page or drawer:

- Health response including persistence backend, renderer enabled/configured/reachable, renderer backend.
- Scheduler state, queue depth/in-flight checks if available.
- Renderer concurrency and last renderer error.
- Notification/Discord status and last send error.
- Snapshot directory availability.
- App version/build info.
- Recent global errors.

This is a differentiator for a self-hosted tool; SaaS competitors often hide these details.

### 6. Alert/noise controls, minimal v1

Do not build a full alerting product yet, but add the minimum production controls:

- Test notification/report preview.
- Mute/disable watch is already supported; make it clearer in UI.
- Display `last_alert_at` and alert transition rule.
- Future: snooze, acknowledge, repeat policy, per-watch channel/template.

## Kill list

Remove or hide these until supported end-to-end:

- Fake/full edit flow that saves via create.
- XPath UI if backend remains CSS-only.
- `strategy` controls unless persisted and executed.
- Unsupported `value equals` / `value changed` controls unless backend supports them.
- Production-looking AI/NL claims backed only by `web/src/lib/nl.ts` heuristics.
- Stale token/SvelteKit documentation.
- Any UI path that implies visual selector support in this scope.

## Required backend/API contracts

The frontend redesign depends on these contracts being explicit:

1. **Watch config read/write**
   - Return persisted config separately from latest status.
   - Full update should be idempotent and not create duplicates.

2. **Dry-run**
   - Evaluate unsaved or edited config and return evidence/diagnostics/artifacts.
   - Must not send alerts or mutate persisted watch state.

3. **Condition catalog**
   - Either return supported subjects/ops from backend or keep a tested shared mapping that cannot expose unsupported UI operations.

4. **Typed failure reasons**
   - Distinguish DNS/TLS/network, HTTP status/blocking, JS render required, renderer unavailable, step timeout, selector zero/too-many matches, price parse failure, notification failure.

5. **Ops status**
   - Expand `/health` or add `/ops` for scheduler/renderer/notification state.

## Slice plan

### Slice 0 — Stabilize truth and verification

- Update README/docs to current Solid/no-token reality or explicitly reintroduce auth as a separate security decision.
- Restore frontend `check` and `test` scripts with at least pure unit coverage for status, format, conditions, API wrapper, and builder conversion.
- Add a small integration/smoke test for build and router paths.
- Success: `cd web && npm run build`, `npm run check`, `npm test`; root `cargo test` remains green.

### Slice 1 — Make edit honest

- Add backend config read/full update contract.
- Change edit route to load persisted config, update via `PUT`, and preserve conditions/render/interval.
- Hide/remove unsupported fields.
- Success: editing a watch changes the existing target and does not create a duplicate.

### Slice 2 — Dry-run evidence loop

- Add dry-run endpoint with no alert side effects.
- Add builder dry-run panel and require/recommend dry-run before save.
- Show condition evidence, selector diagnostics, engine, duration, and artifacts.
- Success: a new watch can be tested, reviewed, and saved from evidence.

### Slice 3 — Ops-grade dashboard

- Expand health/ops data.
- Add ops strip/panel and recent failures.
- Add typed error presentation in list/detail.
- Success: renderer disabled/unreachable, notification failure, and check errors are visible and actionable.

### Slice 4 — Detail runbook/history foundation

- Persist or expose recent check runs enough for a short timeline.
- Detail pane shows last N runs with artifacts/errors.
- Success: users can debug “why did this alert/fail?” from UI without shell access.

## Risks and falsifiers

- If backend config update/dry-run is not approved, the frontend cannot honestly support production edit/build flows.
- If run history is out of scope, detail can still improve latest-run evidence, but competitor-level diff/history remains deferred.
- If auth was removed intentionally for local-only use, docs must say so clearly; if public binding is expected, security needs a separate design before production claims.
- If Solid/base-ui tooling remains hard to test, the stack decision should be revisited only after Slice 0 evidence, not by preference.

## Verification gate

Before implementation is considered production-ready:

```bash
cd web && npm run build
cd web && npm run check
cd web && npm test
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Manual golden paths:

1. Create watch from URL/rules, dry-run, save, see it in list/detail.
2. Edit existing watch and verify same `target_id` persists.
3. Disable/enable, check now, delete.
4. Renderer unavailable state is visible and actionable.
5. Notification/report failure is visible and actionable.
6. SPA fallback and API routes do not shadow each other.

## Deferred

- Visual selector/picker.
- Screenshot diff and HTML/text diff UI.
- Alert acknowledge/snooze/repeat policy.
- Bulk actions/tags/groups.
- Real LLM-backed rule generation.
- Multi-channel notification configuration.
- Multi-user auth/roles.
