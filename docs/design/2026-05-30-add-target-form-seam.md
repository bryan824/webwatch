# Add Target Form Pure Seam

Date: 2026-05-30

## Context

`web/src/lib/components/AddTargetDialog.svelte` currently owns UI markup, local form state, condition kind metadata, field visibility rules, validation, request conversion, price conversion, interval conversion, mutation wiring, and reset behavior.

The backend condition validation seam is now in place. The next frontend step is to centralize equivalent client-side condition metadata and request-building rules without changing UI behavior.

## Goal

Move condition metadata and target-input building out of `AddTargetDialog.svelte` into a pure TypeScript module.

## Non-goals

- Do not migrate Svelte state into a form object yet.
- Do not change API request/response types.
- Do not change backend behavior.
- Do not redesign the dialog UI.
- Do not change mutation/query behavior.

## Contracts and invariants

The extracted form seam must preserve current behavior:

- `text_appears` and `text_disappears` require `value`.
- `selector_exists` and `selector_missing` require `selector`.
- `selector_text_contains` and `selector_text_not_contains` require both `selector` and `value`.
- `price_below` and `price_above` require a numeric USD threshold, converted with `Math.round(dollars * 100)`.
- `price_changed` requires no threshold and may include optional `price_selector`.
- `price_selector` is included only for price-related kinds and only when nonblank.
- `interval_secs` is included only when interval minutes is nonblank, finite, and positive.
- String fields are trimmed before request building.
- Existing validation messages should remain meaningfully the same.

## Recommended approach

Create a pure module, likely `web/src/lib/components/addTargetForm.ts`, that exports:

- `CONDITION_KINDS`
- `fieldsForCondition(kind)`
- `buildTargetInput(draft)`

Keep the dialog's existing local Svelte state and draft shape for this slice. The component should call the pure helpers instead of owning condition rules and request conversion inline.

A later optional slice may move form state/add/remove/reset into a Svelte-runes form object if the pure seam proves useful.

## Tests

Add unit tests for the pure seam covering:

- missing name.
- invalid URL.
- missing required text value.
- missing required selector.
- missing required price threshold.
- trimming `name`, `url`, `value`, `selector`, and `price_selector`.
- converting price dollars to cents.
- converting interval minutes to seconds.
- omitting blank/invalid/non-positive interval.
- `price_changed` without `price_selector` remains valid.

Keep component tests focused on visible behavior.

## Verification

Run:

```bash
cd web && npm test && npm run check
```

## Rollback boundary

Rollback is frontend-only: remove the pure helper module/tests and restore the inline helpers/request builder in `AddTargetDialog.svelte`.

## Acceptance criteria

- `AddTargetDialog.svelte` no longer owns condition metadata or request-building rules.
- Existing UI behavior is preserved.
- New pure seam tests pass.
- `npm test` and `npm run check` pass in `web/`.
