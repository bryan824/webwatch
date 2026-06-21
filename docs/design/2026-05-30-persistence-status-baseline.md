# Persistence Status Baseline

Date: 2026-05-30

## Context

The architecture survey identified target lifecycle as the highest-leverage refactor area, but the current Rust test baseline is not fully green: `tests/reload.rs::create_target_via_api` fails while decoding the response from `GET /targets` as `Vec<TargetStatus>`.

Before deeper lifecycle work, the baseline must be trustworthy and the persistence status lookup should be deepened enough that later application-service seams can rely on direct target status queries.

## Goal

Stabilize the current target lifecycle API test baseline and implement a narrow persistence improvement: direct backend lookup for `Persistence::status(target_id)`.

## Non-goals

- Do not introduce `TargetLifecycle` yet.
- Do not change scheduler responsibilities yet.
- Do not change condition modeling or validation yet.
- Do not change frontend code.
- Do not change API wire formats or response shapes except to fix an unintended bug revealed by tests.

## Current contracts to preserve

- `GET /targets` returns a JSON array of all target statuses.
- `POST /targets` returns `201 Created` and a single `TargetStatus` JSON object for the created target.
- `Persistence::statuses()` returns all persisted targets, including disabled targets and targets with no successful checks.
- `Persistence::status(id)` returns `Ok(Some(TargetStatus))` for an existing persisted target and `Ok(None)` for a missing target.
- The database remains the source of truth for the watch list.

## Recommended approach

### Slice 1: Diagnose and fix the failing create API test

Investigate `tests/reload.rs::create_target_via_api`, specifically the response from `GET /targets` after creating a target.

Determine whether the decoded map is:

- an error response from authorization, routing, or persistence,
- an SPA/static fallback response being served incorrectly,
- an API handler returning the wrong JSON shape,
- or a test setup issue.

Fix the smallest cause while preserving the contracts above.

### Slice 2: Add direct Diesel status lookup

Override `Persistence::status(&self, target_id: &str)` in `src/db/diesel.rs`.

Use the same projection and row-to-domain conversion as `statuses()`, but constrain the query to a single target ID. Keep the public trait shape unchanged for this slice; the override deepens the implementation without forcing broad callers to change.

If useful, extract shared internal mapping so list and single-status paths cannot drift.

### Slice 3: Add focused persistence tests

Add tests to `tests/persistence_backend.rs` for:

- `status("target")` returns the expected status projection for an existing target.
- `status("missing")` returns `None`.

Keep existing persistence tests intact.

## Verification

Run targeted tests first:

```bash
cargo test --test persistence_backend
cargo test --test reload
```

If both pass, run:

```bash
cargo test
```

No frontend commands are required for this slice because no frontend files should change.

## Rollback boundary

This slice should be easy to revert:

- Revert the test-baseline fix if it proves incorrect.
- Remove the Diesel `status(id)` override to fall back to the current default `statuses().find(...)` behavior.
- Remove only the new persistence tests.

No schema migration, API contract change, or frontend change should be involved.

## Acceptance criteria

- `cargo test --test persistence_backend` passes.
- `cargo test --test reload` passes, including `create_target_via_api`.
- `cargo test` passes.
- `src/http.rs` does not gain new lifecycle complexity.
- `src/scheduler.rs` behavior remains unchanged.
