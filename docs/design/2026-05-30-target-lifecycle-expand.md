# Target Lifecycle Expansion

Date: 2026-05-30

## Goal

Move remaining target lifecycle orchestration out of `src/http.rs` and behind `TargetLifecycle` while preserving API behavior.

## Non-goals

- Do not slim scheduler internals yet.
- Do not change API request or response shapes.
- Do not change frontend code.
- Do not change condition modeling.

## Scope

Add lifecycle methods for delete, enable/disable, reload, list/status queries, and manual checks. HTTP keeps authentication, DTO parsing, targets-file loading, response status mapping, and JSON wrapping. Lifecycle owns scheduler/DB coordination and post-mutation status lookup.

## Contracts

- Delete returns whether a target existed.
- Enable/disable returns `None` for missing targets and `Some(TargetStatus)` after successful mutation.
- Reload preserves existing upsert-without-purge behavior and report shape.
- Manual check returns `None` for a missing/non-running target and `Some(TargetStatus)` after recording the check.
- Existing HTTP tests remain the API contract.

## Verification

Run `cargo fmt` and `cargo test`.
