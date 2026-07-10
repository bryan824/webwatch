# Replace SvelteKit frontend with SolidJS

Date: 2026-06-09
Status: proposed
Supersedes: current `web/` SvelteKit app

## Context

The `web/` directory contains a SvelteKit SPA (SSR disabled, adapter-static) that rust-embed
serves from `web/build`. It uses TanStack Svelte Query, shadcn-svelte/bits-ui, and Tailwind v4.
The UI is functional but tied to the legacy 9-kind condition wire format and lacks the
reimagined direction: NL-first config, structured condition model, extraction ladder,
Watchtower instrument aesthetic.

A working interactive prototype at `designs/webwatch-watch-builder/` demonstrates the target
UX using SolidJS + @msviderok/base-ui-solid (no build step, ES modules from esm.sh). This
spec replaces the SvelteKit app with a real Vite-built SolidJS app that:

1. Achieves feature parity with the current UI (list, detail, add, delete, enable/disable,
   check-now, reload, send-report, auth token)
2. Adopts the Watchtower instrument aesthetic from the prototype
3. Uses the new condition model internally (subject/op/value/negate + locator), translating
   to/from the legacy wire format at the API boundary
4. Lays the groundwork for NL compose, dry-run, and cost economics (stubs, not shipped)

## Scope

### In scope — Phase 1 (this spec)

- Delete `web/` SvelteKit app entirely
- New `web/` with Vite + vite-plugin-solid + TypeScript
- @msviderok/base-ui-solid for Dialog, Tooltip, Select, Switch, Tabs
- Custom CSS (Watchtower instrument system, ported from `designs/…/styles.css`)
- Feature parity: target list + search + status dots, target detail (metadata grid,
  conditions, evidence, error banner), add-target dialog (Watch Builder with Build Rules
  tab), delete confirm, enable/disable, check-now, reload, send-report, auth token dialog
- New condition editor using subject/op/value/negate model with wire-format translation
- SPA routing via @tanstack/solid-router (replaces SvelteKit file-based routing)
- Build to `web/build` (unchanged rust-embed contract)
- Vite dev proxy for `/health`, `/targets`, `/notify` → `http://127.0.0.1:3000`

### Deferred

- NL compose tab (placeholder tab, no LLM integration)
- Dry-run / test-against-live-page
- Cost economics display
- Backend wire format migration (tagged enum) — separate spec
- Shared condition catalog endpoint — separate spec

## Architecture

### Principles

- FCIS: pure functions (format, status derivation, wire translation) in `lib/`, imperative
  shell (API calls, routing, DOM) at the edges
- TanStack ecosystem for Solid: solid-query (data), solid-router (routing). Same patterns
  as the current Svelte app's TanStack Query usage — direct port of query keys, mutations,
  cache invalidation
- @msviderok/base-ui-solid for accessible headless primitives (Dialog, Tooltip, Tabs, etc.)

### Data flow

```
localStorage (token)
      ↓
  createSignal ──→ apiFetch (Bearer header)
      ↓
  @tanstack/solid-query
    createQuery (targets, 30s refetchInterval, refetchOnWindowFocus)
    createMutation (check/delete/enable/reload/notify/create → invalidateQueries)
      ↓
  Components read query data, call mutation.mutate()
```

### Condition model translation

The UI works with the new structured model internally:

```ts
interface Condition {
  subject: 'text' | 'element' | 'value' | 'price';
  op: string;          // per-subject: appears/disappears, exists/missing, contains/not_contains, below/above/changed
  value: string;       // text to match, or price threshold as string
  selector: string;    // CSS selector (empty for text subject)
  negate: boolean;     // UI toggle, collapsed into op for wire
}
```

Translation functions at the API boundary convert to/from `ConditionWireKind`:

```ts
// structured → wire (on create/update)
function toWire(c: Condition): ConditionInput { ... }

// wire → structured (on read, if detail view ever needs to show editable conditions)
function fromWire(c: ConditionInput): Condition { ... }
```

This keeps the frontend ready for when the backend adopts the new model — just delete the
translation layer.

## File structure

```
web/
  index.html              # SPA entry, <div id="app">
  vite.config.ts          # solid plugin, dev proxy
  tsconfig.json
  package.json
  src/
    index.tsx             # render(<App />, root)
    App.tsx               # RouterProvider + QueryClientProvider + Tooltip.Provider
    styles/
      instrument.css      # Watchtower design system (from prototype)
      reset.css           # minimal reset
    lib/
      api.ts              # apiFetch, all endpoint functions, ApiError
      types.ts            # TargetStatus, ConditionResult, wire types
      conditions.ts       # Condition model, toWire/fromWire, subject catalog
      queries.ts          # createTargetsQuery, query keys
      mutations.ts        # createCheckMutation, createDeleteMutation, etc.
      status.ts           # deriveStatus (port from current)
      format.ts           # formatPrice, formatRelative (port from current)
      token.ts            # createSignal + localStorage get/set/clear
    components/
      Shell.tsx            # Topbar + sidebar + <Outlet />
      Topbar.tsx           # brand, stats, + watch, reload, send-report, settings
      WatchList.tsx        # search + filtered list + status dots
      WatchListItem.tsx    # single row
      WatchDetail.tsx      # metadata grid, conditions, evidence, actions
      WatchBuilder.tsx     # add/edit dialog — Describe (stub) + Build Rules tabs
      ConditionCard.tsx    # subject/op/value/negate editor row
      TokenDialog.tsx      # auth token entry
      ConfirmDialog.tsx    # controlled AlertDialog wrapper
      StatusDot.tsx        # colored dot
      StatusBadge.tsx      # matched/no-match/error/unknown badge
    pages/
      Home.tsx             # "select a target" placeholder
      Detail.tsx           # loads target by route param, wires mutations
```

## Component mapping

| Current (Svelte)           | New (Solid)            | Notes                              |
|---------------------------|------------------------|------------------------------------|
| AppFrame.svelte           | Shell.tsx              | grid layout, sidebar + main        |
| Toolbar.svelte            | Topbar.tsx             | brand + action buttons             |
| TargetList.svelte         | WatchList.tsx          | search input + scroll list         |
| TargetListItem.svelte     | WatchListItem.tsx      | link to /watches/:id               |
| TargetDetail.svelte       | WatchDetail.tsx        | metadata, conditions, actions      |
| AddTargetDialog.svelte    | WatchBuilder.tsx       | tabs: Describe (stub) / Build      |
| addTargetForm.ts          | conditions.ts          | replaced by new condition model    |
| ConditionResultRow.svelte | (inline in WatchDetail)| simple enough to inline            |
| StatusBadge.svelte        | StatusBadge.tsx        | direct port                        |
| StatusDot.svelte          | StatusDot.tsx          | direct port                        |
| TokenDialog.svelte        | TokenDialog.tsx        | direct port                        |
| ThemeToggle.svelte        | (deferred)             | single dark theme for now          |

## State management

One module (`src/lib/token.ts`) owns the auth token as a signal + localStorage.

@tanstack/solid-query manages server state — direct port of the existing TanStack Svelte
Query patterns:

- `createQuery` for targets (30s `refetchInterval`, `refetchOnWindowFocus: true`,
  `staleTime: 10_000` — identical to current)
- `createMutation` for each write op (check, delete, enable/disable, reload, notify, create)
  with `onSuccess → queryClient.invalidateQueries({ queryKey: targetsQueryKey })`
- Toast notifications via a minimal toast signal array (no library — 5 lines of Solid)

Selected target derived from route params via @tanstack/solid-router's `useParams()`.

## Routing

@tanstack/solid-router with two routes:

```
/                → Home.tsx ("select a watch")
/watches/$id     → Detail.tsx (target detail)
```

Shell.tsx is the layout route wrapping both via `Outlet`. SPA fallback in Vite config
matches the current adapter-static `fallback: 'index.html'` behavior.

## Build & dev

### package.json scripts

```json
{
  "dev": "vite",
  "build": "vite build",
  "preview": "vite preview"
}
```

### Dependencies

```
solid-js
@tanstack/solid-query
@tanstack/solid-router
vite
vite-plugin-solid
typescript
@msviderok/base-ui-solid
```

No Tailwind, no shadcn, no bits-ui, no mode-watcher, no svelte-sonner.

### Vite config

```ts
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

const API_PATHS = ['/health', '/targets', '/notify'];

export default defineConfig({
  plugins: [solid()],
  server: {
    proxy: Object.fromEntries(
      API_PATHS.map((p) => [p, { target: 'http://127.0.0.1:3000', changeOrigin: true }])
    )
  },
  build: {
    outDir: 'build',
    emptyOutDir: true
  }
});
```

Build output goes to `web/build` — rust-embed `#[folder = "web/build"]` unchanged.

## Migration steps

1. Delete `web/` entirely (git tracks it — recoverable)
2. `mkdir web && cd web && npm init -y`
3. Install deps: `solid-js @tanstack/solid-query @tanstack/solid-router vite vite-plugin-solid typescript @msviderok/base-ui-solid`
4. Scaffold: index.html, vite.config.ts, tsconfig.json, src/ tree
5. Port lib/ modules (api, types, conditions, status, format, token) — mostly mechanical
6. Port components in dependency order: StatusDot → StatusBadge → WatchListItem → WatchList → WatchDetail → ConditionCard → WatchBuilder → ConfirmDialog → TokenDialog → Topbar → Shell
7. Wire routing: App.tsx with Router, Shell layout, Home + Detail pages
8. Port styles: instrument.css from prototype, adapted for component class names
9. Verify: `npm run dev`, proxy to running backend, exercise all features
10. Build: `npm run build`, verify `web/build/` output, `cargo build` to confirm rust-embed picks it up

## Risks

- **@msviderok/base-ui-solid maturity**: beta port of Base UI React. Gotchas documented in
  `memory/reference_webwatch_watch_builder_prototype.md`. Arrays in compound components must
  be wrapped in functions; Portal children must be lazy; controlled mode preferred.
- **No toast library**: rolling a minimal signal-based toast. If it gets complex, swap in a
  library later.
- **Single theme**: dropping ThemeToggle (dark only). Can add back later — the prototype's
  oklch palette is dark-first.

## Success criteria

- `npm run build` produces `web/build/` with index.html + assets
- `cargo build` embeds the new frontend (no rust-embed changes needed)
- All 8 API operations work: list, detail, create, delete, enable/disable, check-now, reload, send-report
- Auth token flow works (localStorage persist, 401 → prompt)
- Watchtower instrument aesthetic matches prototype
- Condition editor uses subject/op/value/negate model, translates correctly to wire format
