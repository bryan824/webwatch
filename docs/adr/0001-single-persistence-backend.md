# Single persistence backend (Diesel)

Status: accepted (2026-05-25), supersedes the "keep the `persistence-{diesel,sqlx,seaorm}` matrix" non-goal in `docs/specs/2026-05-20-webwatch-refactor-design.md`.

## Context

Persistence sat behind a `Persistence` trait with three interchangeable adapters — Diesel, SQLx, and SeaORM — selected by mutually-exclusive Cargo features. All three targeted the same SQLite database with the same SQL; the seam varied by *crate*, not by behaviour. Two of the three were dead weight in any given build, and every change to the interface had to be mirrored and re-validated across all three.

## Decision

Keep one adapter (Diesel) and the `Persistence` trait; delete the SQLx and SeaORM adapters, their Cargo features and dependencies, and the `cfg`/`compile_error!` selection machinery.

## Consequences

The `Persistence` trait is retained deliberately even though only one adapter implements it: it is the test seam (handlers and the scheduler depend on `Arc<dyn Persistence>`) and the place a future backend would slot in. Do not inline it away. A second adapter, if one is ever needed, should be an in-memory fake for tests — not another ORM over the same SQLite file.
