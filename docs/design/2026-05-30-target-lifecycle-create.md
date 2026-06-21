# Target Lifecycle Create Slice

Date: 2026-05-30

## Context

The architecture survey identified target lifecycle as the highest-leverage backend seam. Today `src/http.rs` builds targets, generates slugs, applies defaults, validates, calls `Scheduler::add_target`, and then queries persistence for the created status. This makes the HTTP layer know too much about lifecycle ordering.

Earlier slices stabilized persistence status lookup and condition validation. This slice introduces the first lifecycle seam for create-target only.

## Goal

Introduce a backend target lifecycle module with `TargetLifecycle::create(CreateTarget) -> Result<TargetStatus>` and move create-target lifecycle behavior behind it.

## Non-goals

- Do not move delete, toggle, reload, list/status, or manual check paths yet.
- Do not slim scheduler responsibilities yet.
- Do not change persistence trait shape.
- Do not change API request or response shapes.
- Do not change condition model or frontend code.

## Contracts and invariants

`TargetLifecycle::create` owns:

- reading existing target IDs.
- generating a unique slug from the submitted name.
- applying defaults, including `enabled.unwrap_or(true)`.
- constructing a `Target`.
- validating the target.
- persisting/reconciling through the existing scheduler path.
- returning the created `TargetStatus`.

HTTP owns only:

- authentication.
- request DTO deserialization.
- DTO-to-lifecycle-command conversion.
- mapping successful create to `201 Created`.
- mapping validation errors to `400 Bad Request` through the existing `bad_request` path.

Existing behavior must be preserved:

- `POST /targets` returns a single `TargetStatus` JSON object.
- First `Campfire Mug` becomes `campfire-mug`.
- A duplicate slug becomes `campfire-mug-2`.
- Invalid targets are not inserted.

## Recommended approach

Create a new module, likely `src/targets.rs`, and expose it from `src/lib.rs`.

Add:

- `CreateTarget` command type.
- `TargetLifecycle` struct containing the dependencies needed for create:
  - persistence handle.
  - scheduler handle.
- `TargetLifecycle::create(CreateTarget) -> Result<TargetStatus>`.

Move `unique_slug` and `slugify` from `src/http.rs` into the lifecycle module for now. They can stay private helpers.

Keep `Scheduler::add_target()` unchanged in this slice. It remains the persistence/reconcile implementation used by lifecycle create.

If `db.status(&id)` unexpectedly returns `None` after scheduler create, return a database-style error rather than reintroducing status lookup logic in HTTP.

## Tests

Add lifecycle-level tests covering:

- Create target returns status with generated slug `campfire-mug`.
- Slug collision returns `campfire-mug-2`.
- Invalid target returns an error and is not inserted.

Keep existing HTTP create tests as API contract coverage.

## Verification

Run:

```bash
cargo test
```

## Rollback boundary

Rollback is limited to removing `src/targets.rs`, removing the module export, and restoring the previous create-target logic in `src/http.rs`. Existing scheduler and persistence behavior should remain unchanged.

## Acceptance criteria

- `src/http.rs` create handler is thinner and no longer owns slugging or target construction details.
- `TargetLifecycle::create` is covered by direct tests.
- Existing HTTP create tests still pass.
- `cargo test` passes.
