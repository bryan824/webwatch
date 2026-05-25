# webwatch frontend — design

- **Date:** 2026-05-24
- **Status:** Approved (brainstorm); ready for implementation plan
- **Author:** Bryan Tu (with Claude)

## Summary

A web dashboard for the existing webwatch service: view every monitored target with
its latest status, evidence, conditions, and errors, and trigger the service's existing
actions (re-check a target, reload `targets.toml`, send a Discord status report).

The frontend is a single-page app built with **SvelteKit (SPA) + TanStack Query
(`@tanstack/svelte-query`) + shadcn-svelte**, bundled into the existing Rust binary and
served from the same origin as the API. No new product features are added to the backend;
the only backend change is serving the built static assets.

## Goals

- See all targets and their latest status at a glance, and inspect one target in depth.
- Trigger the three existing API actions from the UI with clear feedback.
- Ship as the same single binary / single container we deploy today.

## Non-goals (deferred / separate specs)

- Creating, editing, deleting, or enabling/disabling targets from the UI (no write API exists).
- History / time-series / price charts (no historical endpoint exists).
- Multi-user accounts, roles, or cookie/session auth.
- WebSockets / SSE (polling only).

## Locked decisions (from brainstorm)

| Decision | Choice |
| --- | --- |
| Scope | Read dashboard **+ actions** (Check-now, Reload, Send report) |
| Serving | **Bundle** built SPA into the Rust binary, served same-origin (no CORS) |
| Auth in browser | Token field, stored in **localStorage**, sent as `Authorization: Bearer` |
| Layout | **Master–detail** (left target list, right detail pane) |
| Theme | **Follow system + manual toggle**, persisted |
| Routing | **Routed URLs** (`/`, `/targets/[id]`) — deep-linkable, refresh-safe |

## Backend contract consumed by the UI

Source of truth: `src/http.rs` (router at `src/http.rs:66`) and the types in `src/config.rs`.
The UI is built strictly against these; it does not assume any endpoint that does not exist today.

### Endpoints

| Method | Path | Auth | Notes |
| --- | --- | --- | --- |
| GET | `/health` | none | `{ status, persistence_backend }` |
| GET | `/targets` | optional¹ | Returns `TargetStatus[]` from stored state. **Cheap**, used for polling. |
| GET | `/targets/:id/status` | optional¹ | **Performs a live re-check** then returns that target's `TargetStatus`. Can be slow (HTTP, or CDP browser). `404` for unknown id. |
| POST | `/notify/status` | **required** | Re-checks **all** enabled targets, sends one Discord report, returns `{ sent, summary, statuses }`. Slow + external side effect. |
| POST | `/targets/reload` | **required** | Reloads `targets.toml` from disk; returns `{ added, removed, changed, unchanged }` (string arrays). `400` if the file is invalid. |

¹ "optional" = required only when `api_token` is configured server-side; the action
endpoints (`notify`, `reload`) are **always** required and return `401` with
`{ "error": "WEBWATCH_API_TOKEN is required for this endpoint" }` if no token is configured.

Error envelope for non-2xx: `{ "error": string }` (`src/http.rs` `ErrorResponse`).

### Data shapes (TypeScript mirror)

```ts
type EngineUsed = "http" | "browser_cdp";
type ConditionKind = "text" | "selector" | "selector_text" | "price" | "price_observed";

interface ConditionResult {
  condition_id: string;
  kind: ConditionKind;        // base kind only — see "Data limitation" below
  matched: boolean;
  evidence: string[];
  observed_price_cents: number | null;
  error: string | null;
}

interface TargetStatus {
  target_id: string;
  name: string;
  url: string;
  matched: boolean | null;    // null = unknown / never evaluated
  engine_used: EngineUsed | null;
  price_cents: number | null;
  evidence: string[];
  condition_results: ConditionResult[];
  last_success_at: string | null;  // ISO-8601 strings
  last_error_at: string | null;
  last_error: string | null;
  last_alert_at: string | null;
}
```

**Data limitation (intentional, no backend change):** `TargetStatus` carries condition
*results*, not the target's *configuration*. So `condition_results[].kind` is the base
kind (`text`), **not** the negate-aware label (`text_appears` / `text_disappears`), and the
configured value/selector/threshold are **not** present. The detail pane therefore shows
`condition_id`, base kind, matched, evidence, observed price, and error. Richer condition
labels would require the backend to include target config in the payload — out of scope here,
noted as a future enhancement.

## Architecture

### Repo layout

```
web/                     # standalone SvelteKit project (Node)
  src/
    routes/              # see Routing
    lib/
      api/               # client, types, queries, mutations
      stores/            # token store, theme
      components/        # shadcn-svelte components + app components
  svelte.config.js       # adapter-static, fallback: index.html
  vite.config.ts         # dev proxy to 127.0.0.1:3000
  package.json
src/                     # existing Rust app (one change: static serving)
docs/specs/              # this document
```

### SvelteKit configuration (SPA)

- `@sveltejs/adapter-static` with `fallback: "index.html"`.
- Root `+layout.ts`: `export const ssr = false; export const prerender = false;` (pure client SPA).
- Stack versions: SvelteKit 2 + Svelte 5, `@tanstack/svelte-query` v5, shadcn-svelte (Svelte 5),
  Tailwind. Pin compatible majors in `package.json`.

### Serving model (the only backend change)

- **Production:** embed `web/build` into the binary with **`rust-embed`** and serve it as the
  axum **fallback** registered after the existing routes in `src/http.rs`. Behavior:
  - Existing API routes keep priority (explicit routes beat the fallback), so `GET /targets`,
    `GET /targets/:id/status`, etc. are unchanged.
  - Any unmatched `GET` serves the embedded asset if it exists, else `index.html` (so a refresh
    on `/targets/abc` boots the SPA and the client router resolves it).
  - Correct `Content-Type` per asset extension; `index.html` for the SPA fallback.
- **`rust-embed` dev/release behavior:** in **debug** builds it reads `web/build` from disk at
  runtime (fine to be absent — dev uses the Vite server); in **release** it embeds at compile
  time, so `web/build` must exist before `cargo build --release` (Docker handles this).
- **Dev:** no backend change. Vite dev server proxies the API paths to the Rust server:
  `/health`, `/targets`, `/notify`, `/targets/reload` → `http://127.0.0.1:3000`.
- **Docker:** multi-stage — a Node stage builds `web/` and the result is copied to `web/build`
  in the Rust build context before `cargo build`. Output stays one image, one process.
- **Lighter alternative (if embedding is unwanted):** serve `web/build` from disk via
  `tower_http::services::ServeDir` + a `static_dir` config key instead of `rust-embed`.
- **Cosmetic note:** because the API owns `/targets`, typing the bare URL `/targets` returns
  JSON, not the app. The app only links to `/` and `/targets/[id]`, so this is harmless. Owning
  every path would require moving the API under `/api/*`, which breaks existing curl/Discord/cron
  paths — rejected to preserve the current API surface.

### Routing & app shell

- `/` → dashboard with an empty detail pane ("Select a target").
- `/targets/[id]` → dashboard with that target shown in the detail pane.
- Nested layouts keep the **left list mounted** while the right pane swaps by route:
  - Global shell layout: the toolbar.
  - Dashboard layout: left `TargetList` + `<slot/>` for the detail.
  - Detail page: renders `TargetDetail` for the route `id`.

### Data layer (TanStack Query)

- **Query** `["targets"]` → `GET /targets`, `refetchInterval: 30_000`, `refetchOnWindowFocus: true`,
  keep previous data across refetches. The **list and the detail pane both read from this one
  cached array** (detail = find by `target_id`); there is no per-target fetch just to display.
- **Mutations** (each calls `invalidateQueries(["targets"])` on success + shows a toast):
  - **Check now** → `GET /targets/:id/status` (live re-check; show a per-target spinner; generous
    client timeout because CDP checks are slow).
  - **Reload** → `POST /targets/reload`; toast summarizes `added/removed/changed/unchanged`.
  - **Send report** → confirmation dialog first (it posts to Discord and re-checks all), then
    `POST /notify/status`; toast shows the returned `summary`.
- **`apiFetch(path, opts)` wrapper** (`lib/api/client.ts`): same-origin relative paths; injects
  `Authorization: Bearer <token>` from the token store when present; parses the `{ error }` envelope;
  throws a typed error. A `401` triggers the TokenDialog.
- **Token store** (`lib/stores/token.ts`): localStorage-backed, reactive; `set`, `clear`, and a
  derived `hasToken`.

## Components

shadcn-svelte primitives noted in parentheses; install only what is used.

- **Toolbar** (global): brand; "updated Ns ago" indicator + manual refresh; theme toggle
  (`mode-watcher`); settings gear (opens TokenDialog); **Reload** button; **Send report** button.
  (`Button`, `DropdownMenu`, `Tooltip`)
- **TargetList** (left/master): search `Input` filtering on name + url; a summary line
  ("4 targets · 1 matched · 1 error"); scrollable list (`ScrollArea`) of **TargetListItem**
  (`StatusDot` + name + sub-line); selected item tracks the route `id`.
- **TargetDetail** (right): header (name, external `url` link, **Check now** button with spinner);
  `StatusBadge`; meta grid (engine, formatted price, `last_success_at`, `last_error_at`,
  `last_alert_at`); **Evidence** section (list of `evidence`); **Conditions** section of
  **ConditionResultRow** (base kind label, ✓/○ matched, evidence, observed price, error in red);
  an error banner when `last_error` is set. (`Card`, `Badge`, `Button`, `Separator`)
- **StatusDot / StatusBadge**: a single `deriveStatus(target)` used in both panes (see rules below).
- **TokenDialog** (`Dialog` + password `Input`): paste `WEBWATCH_API_TOKEN`, Save → localStorage,
  Forget button; opens from the gear and auto-opens on `401`.
- **Confirm dialog** (`AlertDialog`) for Send report.
- **Toaster** (`sonner`) for all action results and errors.
- **Theme toggle** (`mode-watcher`): system default + manual override, persisted.

### Status derivation rules

`deriveStatus(target) -> { kind, label, color }` precedence:

1. `last_error` set **and** (`last_error_at` >= `last_success_at`, or no `last_success_at`) →
   **Error** (red).
2. else `matched === true` → **Matched** (green).
3. else `matched === false` → **No match** (grey).
4. else (`matched === null`) → **Unknown** (amber).

Colors map to shadcn-svelte / Tailwind semantic tokens so they work in light and dark.

## States

- **First load:** `Skeleton`s in list + detail; toolbar actions disabled.
- **Polling refresh:** keep previous data; subtle "updating" indicator; no flicker/jump.
- **No token / 401:** auto-open TokenDialog; panes show a locked state until a token is set.
- **Empty (zero targets):** empty state hinting to edit `targets.toml` then Reload.
- **Error (network / 5xx):** error card in the affected pane + toast; retain last good data;
  retry button.
- **Check-now pending:** per-target spinner; button disabled; result lands after invalidation.
- **Action errors:** Reload invalid file → `400` → toast the `error` text; Notify Discord failure
  → toast the `error` text.

## Error handling summary

- All API errors surface via the `{ error }` envelope → typed throw → toast or inline card.
- `401` is special-cased to open the TokenDialog (covers "token not set" and "wrong token").
- Slow endpoints (`/targets/:id/status`, `/notify/status`) use a generous timeout and an explicit
  pending UI rather than blocking the whole app.

## Testing strategy

- **Tooling:** Vitest + `@testing-library/svelte` + jsdom; **MSW** to mock the API; Playwright
  optional for one E2E smoke test.
- **Unit:** `deriveStatus` table; cents→USD formatting; relative-time formatting; token store
  (`set`/`clear`/`hasToken`); `apiFetch` (adds header, parses `{ error }`, maps `401`).
- **Component:** TargetList search + summary counts; TargetDetail evidence/conditions/meta plus
  unknown and error variants; StatusBadge variants; TokenDialog persistence.
- **Integration (MSW):** initial load populates list + detail; Check-now hits `GET /:id/status`
  then refetches; Reload and Notify invalidate and toast; `401` opens TokenDialog; polling updates
  the view.
- **Backend (Rust):** extend `tests/http_fixture.rs` to assert (a) an unmatched `GET` serves
  `index.html` (SPA fallback), (b) API routes still return JSON and are not shadowed, and
  (c) `GET /targets/:id/status` still works after the fallback is added.
- **Manual golden path** before declaring done: run the real backend + built SPA, then set token →
  see targets → Check-now → Reload → Send report (to a test webhook) → toggle theme → deep-link
  refresh on `/targets/[id]`.

## Risks & open items

- **Embed build ordering:** release builds require `web/build` to exist before `cargo build`.
  Mitigated by the documented `rust-embed` debug-reads-from-disk behavior and the multi-stage
  Dockerfile; CI must build the frontend first.
- **CDP slowness:** `Check now` and `Send report` can take many seconds when a target falls back to
  the browser engine. Mitigated by pending UI + generous timeouts; no app-wide blocking.
- **Stack version drift:** SvelteKit 2 / Svelte 5 / shadcn-svelte / TanStack Query v5 must be
  compatible majors; pin them and verify on first scaffold.
- **Condition label richness** is limited by the API (see Data limitation). Acceptable for v1.

## Rough sequencing (detailed plan to follow via writing-plans)

1. Scaffold `web/` (SvelteKit SPA, Tailwind, shadcn-svelte, TanStack Query) + Vite dev proxy.
2. API layer: types, `apiFetch`, token store, `["targets"]` query, mutations.
3. Shell + master–detail layout + routing; TargetList; StatusBadge/`deriveStatus`.
4. TargetDetail (evidence, conditions, meta) + states (loading/empty/error/401).
5. Actions (Check-now, Reload, Send report) + toasts + confirm dialog.
6. Backend: `rust-embed` fallback serving + tests.
7. Docker multi-stage build + docs.
8. Test suites (unit/component/integration) + manual golden-path pass.
