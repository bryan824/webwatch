# webwatch Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a master–detail SvelteKit SPA that views every webwatch target's latest status and triggers the existing Check-now / Reload / Send-report actions, bundled into the Rust binary and served same-origin.

**Architecture:** A SvelteKit SPA (`adapter-static`, `ssr=false`) in `web/`. One TanStack Query (`["targets"]`) polls `GET /targets` and feeds both the left list and the right detail pane; mutations call the three action endpoints and invalidate that query. Presentational components receive data + callback props (easy to test); the root layout owns the query/mutations. In production the built assets are embedded into the Rust binary via `rust-embed` and served as the axum fallback behind the existing API routes. In dev, Vite proxies the API paths to `127.0.0.1:3000`.

**Tech Stack:** SvelteKit 2 + Svelte 5 (runes), TypeScript, Tailwind, shadcn-svelte, `@tanstack/svelte-query` v5, `mode-watcher`, `svelte-sonner`; Vitest + `@testing-library/svelte` + jsdom + MSW; `rust-embed` + axum on the backend.

**Spec:** `docs/specs/2026-05-24-webwatch-frontend-design.md`

---

## Conventions for every task

- Run all `npm`/`npx` commands from `web/` unless stated otherwise. Run `cargo`/`git` from repo root.
- Frequent commits: each task ends with a commit. Use the message shown.
- TDD where logic exists (pure functions, stores, client): write the failing test first, watch it fail, implement, watch it pass.
- Do not introduce features beyond the spec (YAGNI). No target CRUD, no charts.

## File map (what gets created/modified)

```
web/
  package.json                         # scripts + pinned deps
  svelte.config.js                     # adapter-static, fallback index.html
  vite.config.ts                       # dev proxy + vitest config
  tailwind.config.* / app.css          # shadcn-svelte init output
  components.json                      # shadcn-svelte config
  src/
    app.html
    app.css
    routes/
      +layout.ts                       # ssr=false, prerender=false
      +layout.svelte                   # QueryClientProvider, ModeWatcher, Toaster, Toolbar, master frame
      +page.svelte                     # empty detail state
      targets/[id]/+page.svelte        # TargetDetail for route id
    lib/
      api/
        types.ts                       # TargetStatus, ConditionResult, action responses
        client.ts                      # apiFetch + ApiError + endpoint fns
        queries.ts                     # targetsQueryKey, createTargetsQuery
        mutations.ts                   # check-now / reload / notify mutations
      stores/
        token.ts                       # localStorage-backed token + hasToken
      status.ts                        # deriveStatus
      format.ts                        # formatPrice, formatRelative
      components/
        AppFrame.svelte
        Toolbar.svelte
        ThemeToggle.svelte
        TokenDialog.svelte
        StatusDot.svelte
        StatusBadge.svelte
        TargetList.svelte
        TargetListItem.svelte
        TargetDetail.svelte
        ConditionResultRow.svelte
      components/ui/...                 # shadcn-svelte generated (do not hand-edit)
    test/
      setup.ts                         # jsdom + testing-library + MSW server
      msw-handlers.ts                  # mock API
  tests (vitest *.test.ts colocated in lib/)
src/http.rs                            # MODIFY: rust-embed fallback serving
Cargo.toml                             # MODIFY: add rust-embed
tests/http_fixture.rs                  # MODIFY: assert fallback + API coexistence
Dockerfile                            # MODIFY: multi-stage node build → embed
README.md                              # MODIFY: dev + build docs
```

---

## Task 1: Scaffold the SvelteKit SPA

**Files:**
- Create: `web/` (via scaffolder), then overwrite `web/svelte.config.js`, `web/src/routes/+layout.ts`, `web/vite.config.ts`, `web/package.json` (scripts).

- [ ] **Step 1: Scaffold the project**

Run from repo root:

```bash
npx sv create web --template minimal --types ts --no-add-ons
cd web
npm install
npm install -D @sveltejs/adapter-static
```

If the scaffolder is interactive, choose: **SvelteKit minimal**, **TypeScript**, no extra add-ons.

- [ ] **Step 2: Configure adapter-static (SPA)**

Overwrite `web/svelte.config.js`:

```js
import adapter from '@sveltejs/adapter-static';
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: 'index.html', pages: 'build', assets: 'build' }),
    alias: { $lib: 'src/lib', $test: 'src/test' }
  }
};
export default config;
```

- [ ] **Step 3: Force SPA mode**

Create `web/src/routes/+layout.ts`:

```ts
export const ssr = false;
export const prerender = false;
export const trailingSlash = 'never';
```

- [ ] **Step 4: Add npm scripts**

In `web/package.json`, ensure the `scripts` block contains:

```json
{
  "scripts": {
    "dev": "vite dev",
    "build": "vite build",
    "preview": "vite preview",
    "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
    "test": "vitest run",
    "test:watch": "vitest"
  }
}
```

- [ ] **Step 5: Verify it builds**

Run: `npm run build`
Expected: build succeeds and produces `web/build/index.html`.

- [ ] **Step 6: Commit**

```bash
cd /Users/bryan/Projects/webwatch
echo "node_modules/" > web/.gitignore
printf "build/\n.svelte-kit/\n" >> web/.gitignore
git add web/.gitignore web/package.json web/package-lock.json web/svelte.config.js web/vite.config.ts web/src web/tsconfig.json web/app.html 2>/dev/null || git add web
git commit -m "feat(web): scaffold SvelteKit SPA with adapter-static"
```

---

## Task 2: Dev proxy + Tailwind + shadcn-svelte + libraries

**Files:**
- Modify: `web/vite.config.ts`
- Create (via CLI): Tailwind + `web/components.json` + `web/src/app.css`

- [ ] **Step 1: Install runtime libraries**

Run from `web/`:

```bash
npm install @tanstack/svelte-query mode-watcher svelte-sonner
```

- [ ] **Step 2: Init Tailwind + shadcn-svelte**

```bash
npx @tailwindcss/cli@latest init 2>/dev/null || true
npx shadcn-svelte@latest init
```

When prompted by shadcn-svelte init, accept defaults: base color **neutral**, global CSS `src/app.css`, import alias `$lib`. This writes `components.json`, Tailwind config, and `src/app.css` with theme CSS variables (light + `.dark`).

- [ ] **Step 3: Add the UI primitives we use**

```bash
npx shadcn-svelte@latest add button input dialog alert-dialog badge card scroll-area separator skeleton tooltip dropdown-menu sonner
```

These land under `src/lib/components/ui/*` and are not hand-edited.

- [ ] **Step 4: Configure the dev proxy + Vitest**

Overwrite `web/vite.config.ts`:

```ts
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

const API_PATHS = ['/health', '/targets', '/notify'];

export default defineConfig({
  plugins: [sveltekit()],
  server: {
    proxy: Object.fromEntries(
      API_PATHS.map((p) => [p, { target: 'http://127.0.0.1:3000', changeOrigin: true }])
    )
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['src/test/setup.ts'],
    globals: true
  }
});
```

Note: `/targets` proxy also covers `/targets/reload` and `/targets/:id/status` (prefix match).

- [ ] **Step 5: Install test tooling**

```bash
npm install -D vitest @testing-library/svelte @testing-library/jest-dom jsdom msw @testing-library/user-event
```

- [ ] **Step 6: Verify Tailwind classes compile**

Run: `npm run build`
Expected: build succeeds (shadcn components + Tailwind compile with no errors).

- [ ] **Step 7: Commit**

```bash
cd /Users/bryan/Projects/webwatch
git add web
git commit -m "feat(web): add Tailwind, shadcn-svelte, query/toast libs, dev proxy"
```

---

## Task 3: API types

**Files:**
- Create: `web/src/lib/api/types.ts`

- [ ] **Step 1: Write the types** (mirrors `src/config.rs` `TargetStatus`, `ConditionResult`, and `src/http.rs` responses)

```ts
// web/src/lib/api/types.ts
export type EngineUsed = 'http' | 'browser_cdp';
export type ConditionKind = 'text' | 'selector' | 'selector_text' | 'price' | 'price_observed';

export interface ConditionResult {
  condition_id: string;
  kind: ConditionKind;
  matched: boolean;
  evidence: string[];
  observed_price_cents: number | null;
  error: string | null;
}

export interface TargetStatus {
  target_id: string;
  name: string;
  url: string;
  matched: boolean | null;
  engine_used: EngineUsed | null;
  price_cents: number | null;
  evidence: string[];
  condition_results: ConditionResult[];
  last_success_at: string | null;
  last_error_at: string | null;
  last_error: string | null;
  last_alert_at: string | null;
}

export interface HealthResponse {
  status: string;
  persistence_backend: string;
}

export interface ReloadReport {
  added: string[];
  removed: string[];
  changed: string[];
  unchanged: string[];
}

export interface NotifyStatusResponse {
  sent: boolean;
  summary: string;
  statuses: TargetStatus[];
}
```

- [ ] **Step 2: Verify it type-checks**

Run: `npm run check`
Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/api/types.ts
git commit -m "feat(web): add API types mirroring TargetStatus"
```

---

## Task 4: Token store (localStorage-backed)

**Files:**
- Create: `web/src/lib/stores/token.ts`
- Test: `web/src/lib/stores/token.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/stores/token.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { token, hasToken, setToken, clearToken } from './token';

describe('token store', () => {
  beforeEach(() => {
    localStorage.clear();
    clearToken();
  });

  it('starts empty', () => {
    expect(get(token)).toBeNull();
    expect(get(hasToken)).toBe(false);
  });

  it('persists to localStorage on set', () => {
    setToken('secret');
    expect(get(token)).toBe('secret');
    expect(get(hasToken)).toBe(true);
    expect(localStorage.getItem('webwatch_token')).toBe('secret');
  });

  it('clears the token and storage', () => {
    setToken('secret');
    clearToken();
    expect(get(token)).toBeNull();
    expect(localStorage.getItem('webwatch_token')).toBeNull();
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- token`
Expected: FAIL — cannot resolve `./token`.

- [ ] **Step 3: Implement the store**

```ts
// web/src/lib/stores/token.ts
import { writable, derived } from 'svelte/store';
import { browser } from '$app/environment';

const KEY = 'webwatch_token';
const initial = browser ? localStorage.getItem(KEY) : null;

export const token = writable<string | null>(initial);
export const hasToken = derived(token, ($t) => !!$t && $t.length > 0);

export function setToken(value: string): void {
  const v = value.trim();
  if (browser) localStorage.setItem(KEY, v);
  token.set(v);
}

export function clearToken(): void {
  if (browser) localStorage.removeItem(KEY);
  token.set(null);
}
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- token`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/stores
git commit -m "feat(web): add localStorage-backed token store"
```

---

## Task 5: `apiFetch` client + `ApiError`

**Files:**
- Create: `web/src/lib/api/client.ts`
- Test: `web/src/lib/api/client.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/api/client.test.ts
import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { apiFetch, ApiError } from './client';
import { setToken, clearToken } from '../stores/token';

describe('apiFetch', () => {
  beforeEach(() => { clearToken(); vi.restoreAllMocks(); });
  afterEach(() => vi.restoreAllMocks());

  it('attaches the bearer token when present', async () => {
    setToken('abc');
    const spy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    );
    await apiFetch('/targets');
    const init = spy.mock.calls[0][1] as RequestInit;
    expect(new Headers(init.headers).get('authorization')).toBe('Bearer abc');
  });

  it('throws ApiError with the server error message on non-2xx', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ error: 'boom' }), { status: 500, headers: { 'content-type': 'application/json' } })
    );
    await expect(apiFetch('/targets')).rejects.toMatchObject({ status: 500, message: 'boom' });
  });

  it('marks 401 errors as unauthorized', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ error: 'nope' }), { status: 401, headers: { 'content-type': 'application/json' } })
    );
    const err = await apiFetch('/targets').catch((e) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.status).toBe(401);
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- client`
Expected: FAIL — cannot resolve `./client`.

- [ ] **Step 3: Implement the client**

```ts
// web/src/lib/api/client.ts
import { get } from 'svelte/store';
import { token } from '../stores/token';
import type { HealthResponse, NotifyStatusResponse, ReloadReport, TargetStatus } from './types';

export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
  get unauthorized() { return this.status === 401; }
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers);
  const t = get(token);
  if (t) headers.set('Authorization', `Bearer ${t}`);

  const res = await fetch(path, { ...init, headers });
  const isJson = res.headers.get('content-type')?.includes('application/json');
  const body = isJson ? await res.json().catch(() => null) : null;

  if (!res.ok) {
    const message = (body && typeof body.error === 'string') ? body.error : `HTTP ${res.status}`;
    throw new ApiError(res.status, message);
  }
  return body as T;
}

export const getTargets = () => apiFetch<TargetStatus[]>('/targets');
export const getHealth = () => apiFetch<HealthResponse>('/health');
export const checkTarget = (id: string) =>
  apiFetch<TargetStatus>(`/targets/${encodeURIComponent(id)}/status`);
export const reloadTargets = () => apiFetch<ReloadReport>('/targets/reload', { method: 'POST' });
export const notifyStatus = () => apiFetch<NotifyStatusResponse>('/notify/status', { method: 'POST' });
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- client`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/api/client.ts web/src/lib/api/client.test.ts
git commit -m "feat(web): add apiFetch client with bearer + ApiError"
```

---

## Task 6: `deriveStatus` + formatters

**Files:**
- Create: `web/src/lib/status.ts`, `web/src/lib/format.ts`
- Test: `web/src/lib/status.test.ts`, `web/src/lib/format.test.ts`

- [ ] **Step 1: Write the failing tests**

```ts
// web/src/lib/status.test.ts
import { describe, it, expect } from 'vitest';
import { deriveStatus } from './status';
import type { TargetStatus } from './api/types';

const base: TargetStatus = {
  target_id: 'x', name: 'X', url: 'https://e.com', matched: null,
  engine_used: null, price_cents: null, evidence: [], condition_results: [],
  last_success_at: null, last_error_at: null, last_error: null, last_alert_at: null
};

describe('deriveStatus', () => {
  it('reports error when last_error is newer than last success', () => {
    const s = deriveStatus({ ...base, last_success_at: '2026-01-01T00:00:00Z', last_error: 'boom', last_error_at: '2026-01-02T00:00:00Z' });
    expect(s.kind).toBe('error');
  });
  it('reports matched', () => {
    expect(deriveStatus({ ...base, matched: true, last_success_at: '2026-01-02T00:00:00Z' }).kind).toBe('matched');
  });
  it('reports no_match', () => {
    expect(deriveStatus({ ...base, matched: false, last_success_at: '2026-01-02T00:00:00Z' }).kind).toBe('no_match');
  });
  it('reports unknown when never evaluated', () => {
    expect(deriveStatus(base).kind).toBe('unknown');
  });
  it('prefers success over a stale older error', () => {
    const s = deriveStatus({ ...base, matched: true, last_success_at: '2026-01-03T00:00:00Z', last_error: 'old', last_error_at: '2026-01-01T00:00:00Z' });
    expect(s.kind).toBe('matched');
  });
});
```

```ts
// web/src/lib/format.test.ts
import { describe, it, expect } from 'vitest';
import { formatPrice, formatRelative } from './format';

describe('formatPrice', () => {
  it('formats cents to USD', () => expect(formatPrice(3800)).toBe('$38.00'));
  it('handles null', () => expect(formatPrice(null)).toBe('—'));
});

describe('formatRelative', () => {
  it('handles null', () => expect(formatRelative(null)).toBe('never'));
  it('returns a string for a valid iso', () => {
    expect(typeof formatRelative(new Date(Date.now() - 60_000).toISOString())).toBe('string');
  });
});
```

- [ ] **Step 2: Run them to confirm they fail**

Run: `npm run test -- status format`
Expected: FAIL — modules not found.

- [ ] **Step 3: Implement**

```ts
// web/src/lib/status.ts
import type { TargetStatus } from './api/types';

export type StatusKind = 'matched' | 'no_match' | 'unknown' | 'error';

export interface DerivedStatus {
  kind: StatusKind;
  label: string;
  /** Tailwind text/bg color token suffix used by StatusDot/StatusBadge */
  tone: 'success' | 'muted' | 'warning' | 'destructive';
}

function errorIsCurrent(t: TargetStatus): boolean {
  if (!t.last_error) return false;
  if (!t.last_success_at) return true;
  if (!t.last_error_at) return false;
  return new Date(t.last_error_at) >= new Date(t.last_success_at);
}

export function deriveStatus(t: TargetStatus): DerivedStatus {
  if (errorIsCurrent(t)) return { kind: 'error', label: 'Error', tone: 'destructive' };
  if (t.matched === true) return { kind: 'matched', label: 'Matched', tone: 'success' };
  if (t.matched === false) return { kind: 'no_match', label: 'No match', tone: 'muted' };
  return { kind: 'unknown', label: 'Unknown', tone: 'warning' };
}
```

```ts
// web/src/lib/format.ts
export function formatPrice(cents: number | null): string {
  if (cents === null || cents === undefined) return '—';
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD' }).format(cents / 100);
}

export function formatRelative(iso: string | null): string {
  if (!iso) return 'never';
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return iso;
  const diffSec = Math.round((Date.now() - then) / 1000);
  const rtf = new Intl.RelativeTimeFormat('en', { numeric: 'auto' });
  const abs = Math.abs(diffSec);
  if (abs < 60) return rtf.format(-diffSec, 'second');
  if (abs < 3600) return rtf.format(-Math.round(diffSec / 60), 'minute');
  if (abs < 86400) return rtf.format(-Math.round(diffSec / 3600), 'hour');
  return rtf.format(-Math.round(diffSec / 86400), 'day');
}
```

- [ ] **Step 4: Run them to confirm they pass**

Run: `npm run test -- status format`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/status.ts web/src/lib/status.test.ts web/src/lib/format.ts web/src/lib/format.test.ts
git commit -m "feat(web): add deriveStatus and formatters"
```

---

## Task 7: Test setup (jsdom + MSW) and the targets query

**Files:**
- Create: `web/src/test/setup.ts`, `web/src/test/msw-handlers.ts`
- Create: `web/src/lib/api/queries.ts`, `web/src/lib/api/mutations.ts`

- [ ] **Step 1: Create the MSW handlers + test setup**

```ts
// web/src/test/msw-handlers.ts
import { http, HttpResponse } from 'msw';
import type { TargetStatus } from '$lib/api/types';

export const sampleTargets: TargetStatus[] = [
  {
    target_id: 'campfire-mug', name: 'Campfire Mug', url: 'https://example.com/products/campfire-mug',
    matched: true, engine_used: 'http', price_cents: 3800,
    evidence: ['"Add to cart" found'], condition_results: [
      { condition_id: 'condition-1', kind: 'text', matched: true, evidence: ['"Add to cart" found'], observed_price_cents: null, error: null }
    ],
    last_success_at: new Date().toISOString(), last_error_at: null, last_error: null, last_alert_at: null
  },
  {
    target_id: 'sale-price', name: 'Sale Price Watch', url: 'https://example.com/sale',
    matched: null, engine_used: null, price_cents: null, evidence: [], condition_results: [],
    last_success_at: null, last_error_at: null, last_error: null, last_alert_at: null
  }
];

export const handlers = [
  http.get('/targets', () => HttpResponse.json(sampleTargets)),
  http.get('/targets/:id/status', ({ params }) => {
    const t = sampleTargets.find((s) => s.target_id === params.id);
    return t ? HttpResponse.json(t) : HttpResponse.json({ error: 'target not found' }, { status: 404 });
  }),
  http.post('/targets/reload', () =>
    HttpResponse.json({ added: [], removed: [], changed: ['campfire-mug'], unchanged: ['sale-price'] })),
  http.post('/notify/status', () =>
    HttpResponse.json({ sent: true, summary: '2 targets checked', statuses: sampleTargets }))
];
```

```ts
// web/src/test/setup.ts
import '@testing-library/jest-dom/vitest';
import { afterAll, afterEach, beforeAll } from 'vitest';
import { setupServer } from 'msw/node';
import { handlers } from './msw-handlers';

export const server = setupServer(...handlers);
beforeAll(() => server.listen({ onUnhandledRequest: 'error' }));
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
```

- [ ] **Step 2: Implement the query**

```ts
// web/src/lib/api/queries.ts
import { createQuery } from '@tanstack/svelte-query';
import { getTargets } from './client';
import type { TargetStatus } from './types';

export const targetsQueryKey = ['targets'] as const;

export function createTargetsQuery() {
  return createQuery<TargetStatus[]>({
    queryKey: targetsQueryKey,
    queryFn: getTargets,
    refetchInterval: 30_000,
    refetchOnWindowFocus: true,
    staleTime: 10_000
  });
}
```

- [ ] **Step 3: Implement the mutations**

```ts
// web/src/lib/api/mutations.ts
import { createMutation, useQueryClient } from '@tanstack/svelte-query';
import { toast } from 'svelte-sonner';
import { checkTarget, notifyStatus, reloadTargets } from './client';
import { targetsQueryKey } from './queries';
import type { ApiError } from './client';

function describeApiError(e: unknown): string {
  return e instanceof Error ? e.message : 'Request failed';
}

export function createCheckNowMutation() {
  const qc = useQueryClient();
  return createMutation({
    mutationFn: (id: string) => checkTarget(id),
    onSuccess: () => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success('Re-checked'); },
    onError: (e) => toast.error(`Check failed: ${describeApiError(e)}`)
  });
}

export function createReloadMutation() {
  const qc = useQueryClient();
  return createMutation({
    mutationFn: () => reloadTargets(),
    onSuccess: (r) => {
      qc.invalidateQueries({ queryKey: targetsQueryKey });
      toast.success(`Reloaded: +${r.added.length} / -${r.removed.length} / ~${r.changed.length}`);
    },
    onError: (e) => toast.error(`Reload failed: ${describeApiError(e)}`)
  });
}

export function createNotifyMutation() {
  const qc = useQueryClient();
  return createMutation({
    mutationFn: () => notifyStatus(),
    onSuccess: (r) => { qc.invalidateQueries({ queryKey: targetsQueryKey }); toast.success(r.summary || 'Report sent'); },
    onError: (e) => toast.error(`Report failed: ${describeApiError(e)}`)
  });
}
```

- [ ] **Step 4: Verify type-check + existing tests still pass**

Run: `npm run check && npm run test`
Expected: 0 type errors; all prior tests PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/test web/src/lib/api/queries.ts web/src/lib/api/mutations.ts
git commit -m "feat(web): add MSW test setup, targets query, action mutations"
```

---

## Task 8: StatusDot + StatusBadge (presentational)

**Files:**
- Create: `web/src/lib/components/StatusDot.svelte`, `web/src/lib/components/StatusBadge.svelte`
- Test: `web/src/lib/components/StatusBadge.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/components/StatusBadge.test.ts
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import StatusBadge from './StatusBadge.svelte';
import type { TargetStatus } from '$lib/api/types';

const t = (over: Partial<TargetStatus>): TargetStatus => ({
  target_id: 'x', name: 'X', url: 'https://e.com', matched: null, engine_used: null,
  price_cents: null, evidence: [], condition_results: [], last_success_at: null,
  last_error_at: null, last_error: null, last_alert_at: null, ...over
});

describe('StatusBadge', () => {
  it('shows Matched', () => {
    render(StatusBadge, { target: t({ matched: true, last_success_at: '2026-01-02T00:00:00Z' }) });
    expect(screen.getByText('Matched')).toBeInTheDocument();
  });
  it('shows Error', () => {
    render(StatusBadge, { target: t({ last_error: 'boom', last_error_at: '2026-01-02T00:00:00Z' }) });
    expect(screen.getByText('Error')).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- StatusBadge`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement both components**

```svelte
<!-- web/src/lib/components/StatusDot.svelte -->
<script lang="ts">
  import type { StatusKind } from '$lib/status';
  let { kind }: { kind: StatusKind } = $props();
  const color: Record<StatusKind, string> = {
    matched: 'bg-green-500',
    no_match: 'bg-muted-foreground',
    unknown: 'bg-amber-500',
    error: 'bg-red-500'
  };
</script>

<span class={`inline-block h-2.5 w-2.5 rounded-full ${color[kind]}`} aria-hidden="true"></span>
```

```svelte
<!-- web/src/lib/components/StatusBadge.svelte -->
<script lang="ts">
  import { Badge } from '$lib/components/ui/badge';
  import StatusDot from './StatusDot.svelte';
  import { deriveStatus } from '$lib/status';
  import type { TargetStatus } from '$lib/api/types';

  let { target }: { target: TargetStatus } = $props();
  const s = $derived(deriveStatus(target));
</script>

<Badge variant="outline" class="gap-1.5">
  <StatusDot kind={s.kind} />
  {s.label}
</Badge>
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- StatusBadge`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/components/StatusDot.svelte web/src/lib/components/StatusBadge.svelte web/src/lib/components/StatusBadge.test.ts
git commit -m "feat(web): add StatusDot and StatusBadge"
```

---

## Task 9: TargetList + TargetListItem (search + selection)

**Files:**
- Create: `web/src/lib/components/TargetListItem.svelte`, `web/src/lib/components/TargetList.svelte`
- Test: `web/src/lib/components/TargetList.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/components/TargetList.test.ts
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TargetList from './TargetList.svelte';
import { sampleTargets } from '$test/msw-handlers';

describe('TargetList', () => {
  it('renders all targets and a summary count', () => {
    render(TargetList, { targets: sampleTargets, selectedId: undefined });
    expect(screen.getByText('Campfire Mug')).toBeInTheDocument();
    expect(screen.getByText('Sale Price Watch')).toBeInTheDocument();
    expect(screen.getByText(/2 targets/i)).toBeInTheDocument();
  });

  it('filters by search text', async () => {
    render(TargetList, { targets: sampleTargets, selectedId: undefined });
    await userEvent.type(screen.getByPlaceholderText(/search/i), 'campfire');
    expect(screen.getByText('Campfire Mug')).toBeInTheDocument();
    expect(screen.queryByText('Sale Price Watch')).not.toBeInTheDocument();
  });
});
```

(The `$test` → `src/test` alias was already added to `svelte.config.js` in Task 1, so `$test/msw-handlers` resolves in tests.)

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- TargetList`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement the components**

```svelte
<!-- web/src/lib/components/TargetListItem.svelte -->
<script lang="ts">
  import StatusDot from './StatusDot.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, selected }: { target: TargetStatus; selected: boolean } = $props();
  const s = $derived(deriveStatus(target));
</script>

<a
  href={`/targets/${encodeURIComponent(target.target_id)}`}
  data-sveltekit-noscroll
  class={`flex items-center gap-2 rounded-md px-2.5 py-2 text-sm transition-colors hover:bg-muted ${selected ? 'bg-muted ring-1 ring-primary' : ''}`}
  aria-current={selected ? 'page' : undefined}
>
  <StatusDot kind={s.kind} />
  <span class="flex-1 min-w-0">
    <span class="block truncate font-medium">{target.name}</span>
    <span class="block truncate text-xs text-muted-foreground">
      {s.kind === 'error' ? 'error ' : ''}{formatRelative(target.last_success_at ?? target.last_error_at)}
    </span>
  </span>
</a>
```

```svelte
<!-- web/src/lib/components/TargetList.svelte -->
<script lang="ts">
  import { Input } from '$lib/components/ui/input';
  import { ScrollArea } from '$lib/components/ui/scroll-area';
  import TargetListItem from './TargetListItem.svelte';
  import { deriveStatus } from '$lib/status';
  import type { TargetStatus } from '$lib/api/types';

  let { targets, selectedId }: { targets: TargetStatus[]; selectedId?: string } = $props();
  let q = $state('');

  const filtered = $derived(
    targets.filter((t) => `${t.name} ${t.url}`.toLowerCase().includes(q.toLowerCase()))
  );
  const matched = $derived(targets.filter((t) => deriveStatus(t).kind === 'matched').length);
  const errored = $derived(targets.filter((t) => deriveStatus(t).kind === 'error').length);
</script>

<div class="flex h-full flex-col gap-2 p-2">
  <Input placeholder="Search targets…" bind:value={q} />
  <p class="px-1 text-xs text-muted-foreground">
    {targets.length} targets · {matched} matched · {errored} error
  </p>
  <ScrollArea class="flex-1">
    <div class="flex flex-col gap-0.5">
      {#each filtered as t (t.target_id)}
        <TargetListItem target={t} selected={t.target_id === selectedId} />
      {/each}
      {#if filtered.length === 0}
        <p class="px-2 py-6 text-center text-sm text-muted-foreground">No matching targets.</p>
      {/if}
    </div>
  </ScrollArea>
</div>
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- TargetList`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/components/TargetList.svelte web/src/lib/components/TargetListItem.svelte web/src/lib/components/TargetList.test.ts
git commit -m "feat(web): add TargetList with search, counts, selection"
```

---

## Task 10: ConditionResultRow + TargetDetail

**Files:**
- Create: `web/src/lib/components/ConditionResultRow.svelte`, `web/src/lib/components/TargetDetail.svelte`
- Test: `web/src/lib/components/TargetDetail.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/components/TargetDetail.test.ts
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import TargetDetail from './TargetDetail.svelte';
import { sampleTargets } from '$test/msw-handlers';

describe('TargetDetail', () => {
  it('shows evidence and condition results', () => {
    render(TargetDetail, { target: sampleTargets[0], checking: false, onCheckNow: () => {} });
    expect(screen.getByText('"Add to cart" found')).toBeInTheDocument();
    expect(screen.getByText(/text/)).toBeInTheDocument();
  });

  it('fires onCheckNow when the button is clicked', async () => {
    const onCheckNow = vi.fn();
    render(TargetDetail, { target: sampleTargets[0], checking: false, onCheckNow });
    await userEvent.click(screen.getByRole('button', { name: /check now/i }));
    expect(onCheckNow).toHaveBeenCalledOnce();
  });

  it('shows an unknown empty-state for never-checked targets', () => {
    render(TargetDetail, { target: sampleTargets[1], checking: false, onCheckNow: () => {} });
    expect(screen.getByText(/not checked yet/i)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- TargetDetail`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement both components**

```svelte
<!-- web/src/lib/components/ConditionResultRow.svelte -->
<script lang="ts">
  import { formatPrice } from '$lib/format';
  import type { ConditionResult } from '$lib/api/types';
  let { c }: { c: ConditionResult } = $props();
</script>

<div class="flex items-start gap-2 py-1 text-sm">
  <span class={c.matched ? 'text-green-600' : 'text-muted-foreground'}>{c.matched ? '✓' : '○'}</span>
  <div class="flex-1">
    <span class="font-mono text-xs">{c.kind}</span>
    {#if c.observed_price_cents !== null}<span class="text-muted-foreground"> · {formatPrice(c.observed_price_cents)}</span>{/if}
    {#each c.evidence as e}<div class="text-xs text-muted-foreground">{e}</div>{/each}
    {#if c.error}<div class="text-xs text-red-600">{c.error}</div>{/if}
  </div>
</div>
```

```svelte
<!-- web/src/lib/components/TargetDetail.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { Separator } from '$lib/components/ui/separator';
  import StatusBadge from './StatusBadge.svelte';
  import ConditionResultRow from './ConditionResultRow.svelte';
  import { deriveStatus } from '$lib/status';
  import { formatPrice, formatRelative } from '$lib/format';
  import type { TargetStatus } from '$lib/api/types';

  let { target, checking, onCheckNow }:
    { target: TargetStatus; checking: boolean; onCheckNow: () => void } = $props();
  const s = $derived(deriveStatus(target));
</script>

<div class="flex h-full flex-col gap-4 p-4">
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <h1 class="truncate text-xl font-semibold">{target.name}</h1>
      <a href={target.url} target="_blank" rel="noreferrer" class="truncate text-sm text-muted-foreground underline">{target.url}</a>
    </div>
    <Button onclick={onCheckNow} disabled={checking}>{checking ? 'Checking…' : 'Check now'}</Button>
  </div>

  <div class="flex flex-wrap items-center gap-4 text-sm">
    <StatusBadge {target} />
    <span class="text-muted-foreground">engine: <span class="text-foreground">{target.engine_used ?? '—'}</span></span>
    <span class="text-muted-foreground">price: <span class="text-foreground">{formatPrice(target.price_cents)}</span></span>
    <span class="text-muted-foreground">last success: <span class="text-foreground">{formatRelative(target.last_success_at)}</span></span>
    <span class="text-muted-foreground">last alert: <span class="text-foreground">{formatRelative(target.last_alert_at)}</span></span>
  </div>

  {#if s.kind === 'error' && target.last_error}
    <div class="rounded-md border border-red-300 bg-red-50 p-3 text-sm text-red-700 dark:bg-red-950/40">
      {target.last_error} <span class="text-xs opacity-70">({formatRelative(target.last_error_at)})</span>
    </div>
  {/if}

  {#if s.kind === 'unknown' && target.condition_results.length === 0}
    <p class="text-sm text-muted-foreground">Not checked yet. Click <strong>Check now</strong> to evaluate this target.</p>
  {:else}
    <Separator />
    <section>
      <h2 class="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Evidence</h2>
      {#if target.evidence.length}
        {#each target.evidence as e}<p class="text-sm">{e}</p>{/each}
      {:else}<p class="text-sm text-muted-foreground">No evidence.</p>{/if}
    </section>
    <section>
      <h2 class="mb-1 text-xs font-medium uppercase tracking-wide text-muted-foreground">Conditions</h2>
      {#each target.condition_results as c (c.condition_id)}<ConditionResultRow {c} />{/each}
      {#if target.condition_results.length === 0}<p class="text-sm text-muted-foreground">No condition results.</p>{/if}
    </section>
  {/if}
</div>
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- TargetDetail`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/components/ConditionResultRow.svelte web/src/lib/components/TargetDetail.svelte web/src/lib/components/TargetDetail.test.ts
git commit -m "feat(web): add TargetDetail and ConditionResultRow"
```

---

## Task 11: TokenDialog + ThemeToggle

**Files:**
- Create: `web/src/lib/components/TokenDialog.svelte`, `web/src/lib/components/ThemeToggle.svelte`
- Test: `web/src/lib/components/TokenDialog.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// web/src/lib/components/TokenDialog.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';
import TokenDialog from './TokenDialog.svelte';
import { token, clearToken } from '$lib/stores/token';

describe('TokenDialog', () => {
  beforeEach(() => { localStorage.clear(); clearToken(); });

  it('saves the entered token to the store', async () => {
    render(TokenDialog, { open: true });
    await userEvent.type(screen.getByLabelText(/api token/i), 'my-secret');
    await userEvent.click(screen.getByRole('button', { name: /save/i }));
    expect(get(token)).toBe('my-secret');
  });
});
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `npm run test -- TokenDialog`
Expected: FAIL — component not found.

- [ ] **Step 3: Implement both components**

```svelte
<!-- web/src/lib/components/TokenDialog.svelte -->
<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { token, setToken, clearToken } from '$lib/stores/token';

  let { open = $bindable(false) }: { open?: boolean } = $props();
  let value = $state($token ?? '');

  function save() { setToken(value); open = false; }
  function forget() { clearToken(); value = ''; }
</script>

<Dialog.Root bind:open>
  <Dialog.Content>
    <Dialog.Header>
      <Dialog.Title>API token</Dialog.Title>
      <Dialog.Description>Paste your WEBWATCH_API_TOKEN. Stored in this browser only.</Dialog.Description>
    </Dialog.Header>
    <label class="text-sm font-medium" for="token-input">API token</label>
    <Input id="token-input" type="password" bind:value placeholder="Bearer token" />
    <Dialog.Footer>
      <Button variant="ghost" onclick={forget}>Forget</Button>
      <Button onclick={save}>Save</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
```

```svelte
<!-- web/src/lib/components/ThemeToggle.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { toggleMode } from 'mode-watcher';
</script>

<Button variant="ghost" size="icon" onclick={toggleMode} aria-label="Toggle theme">
  <span class="dark:hidden">☾</span>
  <span class="hidden dark:inline">☀</span>
</Button>
```

- [ ] **Step 4: Run it to confirm it passes**

Run: `npm run test -- TokenDialog`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/components/TokenDialog.svelte web/src/lib/components/ThemeToggle.svelte web/src/lib/components/TokenDialog.test.ts
git commit -m "feat(web): add TokenDialog and ThemeToggle"
```

---

## Task 12: Toolbar (Reload + Send report + confirm + token gear)

**Files:**
- Create: `web/src/lib/components/Toolbar.svelte`

- [ ] **Step 1: Implement the Toolbar**

```svelte
<!-- web/src/lib/components/Toolbar.svelte -->
<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import * as AlertDialog from '$lib/components/ui/alert-dialog';
  import ThemeToggle from './ThemeToggle.svelte';
  import { createReloadMutation, createNotifyMutation } from '$lib/api/mutations';

  let { onOpenToken, updatedLabel }:
    { onOpenToken: () => void; updatedLabel: string } = $props();

  const reload = createReloadMutation();
  const notify = createNotifyMutation();
  let confirmOpen = $state(false);
</script>

<header class="flex items-center justify-between border-b px-4 py-2">
  <div class="flex items-center gap-3">
    <strong>webwatch</strong>
    <span class="text-xs text-muted-foreground">updated {updatedLabel}</span>
  </div>
  <div class="flex items-center gap-2">
    <Button variant="outline" size="sm" disabled={$reload.isPending} onclick={() => $reload.mutate()}>
      {$reload.isPending ? 'Reloading…' : 'Reload'}
    </Button>
    <Button size="sm" onclick={() => (confirmOpen = true)}>Send report</Button>
    <Button variant="ghost" size="icon" aria-label="Settings" onclick={onOpenToken}>⚙</Button>
    <ThemeToggle />
  </div>
</header>

<AlertDialog.Root bind:open={confirmOpen}>
  <AlertDialog.Content>
    <AlertDialog.Header>
      <AlertDialog.Title>Send Discord status report?</AlertDialog.Title>
      <AlertDialog.Description>
        This re-checks every enabled target and posts one report to Discord. It can take a while.
      </AlertDialog.Description>
    </AlertDialog.Header>
    <AlertDialog.Footer>
      <AlertDialog.Cancel>Cancel</AlertDialog.Cancel>
      <AlertDialog.Action disabled={$notify.isPending} onclick={() => $notify.mutate()}>
        {$notify.isPending ? 'Sending…' : 'Send report'}
      </AlertDialog.Action>
    </AlertDialog.Footer>
  </AlertDialog.Content>
</AlertDialog.Root>
```

- [ ] **Step 2: Verify type-check**

Run: `npm run check`
Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/lib/components/Toolbar.svelte
git commit -m "feat(web): add Toolbar with Reload and Send-report confirm"
```

---

## Task 13: Root layout (providers + master frame) and routes

**Files:**
- Modify: `web/src/routes/+layout.svelte`
- Create: `web/src/lib/components/AppFrame.svelte`
- Create: `web/src/routes/+page.svelte`, `web/src/routes/targets/[id]/+page.svelte`
- Create: `web/src/lib/api/query-client.ts`

- [ ] **Step 1: Shared QueryClient**

```ts
// web/src/lib/api/query-client.ts
import { QueryClient } from '@tanstack/svelte-query';

export const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: 1, refetchOnWindowFocus: true } }
});
```

- [ ] **Step 2: Root layout — providers + toolbar + master frame**

```svelte
<!-- web/src/routes/+layout.svelte -->
<script lang="ts">
  import '../app.css';
  import { QueryClientProvider } from '@tanstack/svelte-query';
  import { ModeWatcher } from 'mode-watcher';
  import { Toaster } from 'svelte-sonner';
  import { queryClient } from '$lib/api/query-client';
  import AppFrame from '$lib/components/AppFrame.svelte';

  let { children } = $props();
</script>

<QueryClientProvider client={queryClient}>
  <AppFrame {children} />
  <Toaster richColors position="top-right" />
  <ModeWatcher />
</QueryClientProvider>
```

- [ ] **Step 2b: AppFrame — master frame inside the provider (owns the targets query)**

`createQuery`/`createMutation` must run **inside** the `QueryClientProvider`, so the query and the persistent master frame live in a child component, not in `+layout.svelte`.

```svelte
<!-- web/src/lib/components/AppFrame.svelte -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/stores';
  import { createTargetsQuery } from '$lib/api/queries';
  import { hasToken } from '$lib/stores/token';
  import { ApiError } from '$lib/api/client';
  import { formatRelative } from '$lib/format';
  import Toolbar from './Toolbar.svelte';
  import TargetList from './TargetList.svelte';
  import TokenDialog from './TokenDialog.svelte';
  import { Skeleton } from '$lib/components/ui/skeleton';

  let { children }: { children: Snippet } = $props();
  let tokenOpen = $state(false);

  const targets = createTargetsQuery();
  const selectedId = $derived($page.params.id);
  const updatedLabel = $derived(
    formatRelative($targets.dataUpdatedAt ? new Date($targets.dataUpdatedAt).toISOString() : null)
  );

  // Auto-open the token dialog when the API rejects us as unauthorized.
  $effect(() => {
    const err = $targets.error;
    if (err instanceof ApiError && err.status === 401) tokenOpen = true;
  });
</script>

<div class="flex h-screen flex-col">
  <Toolbar onOpenToken={() => (tokenOpen = true)} {updatedLabel} />
  <div class="grid flex-1 grid-cols-[320px_1fr] overflow-hidden">
    <aside class="overflow-hidden border-r">
      {#if $targets.isPending}
        <div class="space-y-2 p-3">{#each Array(4) as _}<Skeleton class="h-10 w-full" />{/each}</div>
      {:else if $targets.error}
        <div class="p-4 text-sm">
          <p class="text-red-600">{($targets.error as Error).message}</p>
          <button class="mt-2 underline" onclick={() => $targets.refetch()}>Retry</button>
          {#if !$hasToken}<button class="mt-2 block underline" onclick={() => (tokenOpen = true)}>Enter API token</button>{/if}
        </div>
      {:else if ($targets.data ?? []).length === 0}
        <p class="p-4 text-sm text-muted-foreground">No targets. Edit <code>targets.toml</code> then Reload.</p>
      {:else}
        <TargetList targets={$targets.data ?? []} {selectedId} />
      {/if}
    </aside>
    <main class="overflow-auto">{@render children()}</main>
  </div>
</div>
<TokenDialog bind:open={tokenOpen} />

- [ ] **Step 3: Empty detail route**

```svelte
<!-- web/src/routes/+page.svelte -->
<div class="flex h-full items-center justify-center text-muted-foreground">
  Select a target from the list.
</div>
```

- [ ] **Step 4: Detail route**

```svelte
<!-- web/src/routes/targets/[id]/+page.svelte -->
<script lang="ts">
  import { page } from '$app/stores';
  import { createTargetsQuery } from '$lib/api/queries';
  import { createCheckNowMutation } from '$lib/api/mutations';
  import TargetDetail from '$lib/components/TargetDetail.svelte';

  const targets = createTargetsQuery();
  const check = createCheckNowMutation();
  const id = $derived($page.params.id);
  const target = $derived(($targets.data ?? []).find((t) => t.target_id === id));
</script>

{#if target}
  <TargetDetail {target} checking={$check.isPending} onCheckNow={() => $check.mutate(target.target_id)} />
{:else if $targets.isPending}
  <div class="p-4 text-sm text-muted-foreground">Loading…</div>
{:else}
  <div class="p-4 text-sm text-muted-foreground">Target <code>{id}</code> not found.</div>
{/if}
```

- [ ] **Step 5: Verify build + type-check**

Run: `npm run check && npm run build`
Expected: 0 type errors; build emits `web/build/index.html`.

- [ ] **Step 6: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/routes web/src/lib/api/query-client.ts web/src/lib/components/AppFrame.svelte
git commit -m "feat(web): wire root layout, master-detail routing, states"
```

---

## Task 14: Integration test — action flow with MSW

**Files:**
- Create: `web/src/routes/integration.test.ts`

- [ ] **Step 1: Write the integration test**

```ts
// web/src/routes/integration.test.ts
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import TargetList from '$lib/components/TargetList.svelte';
import { getTargets } from '$lib/api/client';
import { setToken } from '$lib/stores/token';

describe('targets API (MSW)', () => {
  beforeEach(() => setToken('test'));

  it('GET /targets returns the mocked targets', async () => {
    const data = await getTargets();
    expect(data).toHaveLength(2);
    expect(data[0].name).toBe('Campfire Mug');
  });

  it('renders the fetched targets in the list', async () => {
    const data = await getTargets();
    render(TargetList, { targets: data, selectedId: undefined });
    await waitFor(() => expect(screen.getByText('Campfire Mug')).toBeInTheDocument());
  });
});
```

- [ ] **Step 2: Run the whole suite**

Run: `npm run test`
Expected: all tests PASS.

- [ ] **Step 3: Manual dev smoke (requires backend running)**

```bash
# terminal 1 (repo root): WEBWATCH_API_TOKEN=dev cargo run
# terminal 2: cd web && npm run dev
```

Open `http://localhost:5173`, click ⚙, enter `dev`, confirm targets load, click a target, Check now, Reload. Confirm toasts appear.

- [ ] **Step 4: Commit**

```bash
cd /Users/bryan/Projects/webwatch && git add web/src/routes/integration.test.ts
git commit -m "test(web): add API integration tests with MSW"
```

---

## Task 15: Backend — serve the built SPA via rust-embed fallback

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/http.rs`
- Test: `tests/http_fixture.rs`

- [ ] **Step 1: Add the dependency**

In `Cargo.toml` `[dependencies]` add:

```toml
rust-embed = { version = "8", features = ["mime-guess"] }
```

- [ ] **Step 2: Write the failing backend tests**

Append to `tests/http_fixture.rs` (use the existing test harness/router builder in that file as the pattern; the router is built by `webwatch::http::router(state)` per `src/http.rs:66`):

```rust
#[tokio::test]
async fn serves_spa_index_for_unknown_get() {
    let app = test_router().await; // existing helper that builds the axum Router
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/targets/some-id")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), axum::http::StatusCode::OK);
    let body = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("<!DOCTYPE html>") || body.starts_with(b"<!"));
}

#[tokio::test]
async fn api_routes_are_not_shadowed_by_spa() {
    let app = test_router().await;
    let res = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/targets")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // /targets is JSON from the API, not the SPA HTML
    let ct = res.headers().get(axum::http::header::CONTENT_TYPE).cloned();
    assert!(ct.map(|v| v.to_str().unwrap().contains("application/json")).unwrap_or(false));
}
```

> If `tests/http_fixture.rs` does not already expose a `test_router()` helper, add one that constructs `HttpState` with an in-memory/temp persistence backend (mirror the setup the existing tests use) and returns `webwatch::http::router(state)`. Reuse the existing fixtures rather than duplicating them.

- [ ] **Step 3: Run the tests to confirm they fail**

Run: `cargo test --test http_fixture serves_spa_index_for_unknown_get`
Expected: FAIL (currently unknown GET returns 404, no embedded assets).

- [ ] **Step 4: Implement embedded static serving**

In `src/http.rs`, add near the top:

```rust
use axum::body::Body;
use axum::http::{header, Uri};
use axum::response::Response;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "web/build"]
struct WebAssets;

async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let candidate = if path.is_empty() { "index.html" } else { path };

    if let Some(content) = WebAssets::get(candidate) {
        let mime = mime_guess::from_path(candidate).first_or_octet_stream();
        return Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }
    // SPA fallback: serve index.html for client-side routes
    match WebAssets::get("index.html") {
        Some(content) => Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(content.data.into_owned()))
            .unwrap(),
        None => Response::builder()
            .status(axum::http::StatusCode::NOT_FOUND)
            .body(Body::from("frontend not built"))
            .unwrap(),
    }
}
```

Then register it as the router fallback in `router()` (after the existing `.route(...)` calls, before `.with_state`/`.layer`):

```rust
        .fallback(static_handler)
```

Add `mime_guess` to `Cargo.toml` if not pulled transitively:

```toml
mime_guess = "2"
```

- [ ] **Step 5: Build the frontend so the embed folder exists, then run tests**

```bash
cd web && npm run build && cd ..
cargo test --test http_fixture
```

Expected: both new tests PASS, all existing tests still PASS.

> `rust-embed` reads `web/build` from disk in debug builds, so the tests work as long as `web/build` exists. Release builds embed it at compile time.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/http.rs tests/http_fixture.rs
git commit -m "feat: serve embedded SvelteKit SPA via axum fallback"
```

---

## Task 16: Docker multi-stage + docs

**Files:**
- Modify: `Dockerfile`
- Modify: `README.md`

- [ ] **Step 1: Add a Node build stage to the Dockerfile**

Edit `Dockerfile` so a Node stage builds the frontend and its output is present at `web/build` before `cargo build`. Add as the first stage and copy its output into the Rust build context:

```dockerfile
# ---- frontend build ----
FROM node:22-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# ---- existing rust build stage ----
# (in the Rust builder stage, before `cargo build --release`, add:)
# COPY --from=web /web/build ./web/build
```

Integrate the `COPY --from=web /web/build ./web/build` line into the existing Rust builder stage right before the `cargo build` step so `rust-embed` can embed it.

- [ ] **Step 2: Build the image**

```bash
docker build -t webwatch:frontend-test .
```

Expected: build succeeds; the binary contains embedded assets.

- [ ] **Step 3: Update README**

Add a "Web UI" section to `README.md` documenting:
- Dev: `cargo run` (terminal 1) + `cd web && npm run dev` (terminal 2), open `http://localhost:5173`, enter the API token via the gear.
- Prod: `cd web && npm run build` then `cargo build --release`; the UI is served at `http://127.0.0.1:3000/`.
- Note the bare `/targets` URL returns API JSON (the app lives at `/` and `/targets/[id]`).

- [ ] **Step 4: Commit**

```bash
git add Dockerfile README.md
git commit -m "build: multi-stage Docker build embedding the web UI; docs"
```

---

## Final verification

- [ ] `cd web && npm run check && npm run test` → 0 type errors, all tests pass.
- [ ] `cd web && npm run build` → `web/build/index.html` exists.
- [ ] `cargo test` → all backend tests pass (including the two new fallback tests).
- [ ] Manual golden path with `WEBWATCH_API_TOKEN=dev cargo run` + built UI served at `:3000`:
  set token → list loads → select target → Check now → Reload → Send report (test webhook) → theme toggle → refresh on `/targets/<id>` keeps selection.

---

## Self-review against the spec (filled in)

- **Scope (dashboard + actions):** Tasks 9–14 (view) + Task 12/13 (Reload, Send report, Check now). ✓
- **Serving bundled in binary:** Task 15 (rust-embed fallback), Task 16 (Docker). ✓
- **Token in localStorage + Bearer:** Task 4 (store), Task 5 (header), Task 11 (dialog), Task 13 (401 auto-open). ✓
- **Master–detail + routed URLs:** Task 13 routes `/` and `/targets/[id]`, list mounted in layout. ✓
- **Follow-system theme + toggle:** Task 11 ThemeToggle + Task 13 `ModeWatcher`. ✓
- **Data flow (single ['targets'] query feeds list + detail; mutations invalidate):** Tasks 7, 13. ✓
- **Condition display limited to results (base kind):** Task 10 renders `kind`/`matched`/evidence only. ✓
- **States (loading/empty/error/401/check-pending):** Task 13 layout branches + Task 10 unknown state. ✓
- **Testing (unit/component/integration/backend):** Tasks 4–11, 14, 15. ✓
- **Out of scope** (CRUD, charts, SSE, cookie auth): not present. ✓
