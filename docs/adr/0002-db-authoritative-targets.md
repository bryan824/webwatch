# Targets are database-authoritative; targets.toml is a seed

Status: accepted (2026-05-25), reverses the "no creating/editing targets from the UI" non-goal in `docs/specs/2026-05-24-webwatch-frontend-design.md`.

## Context

The watch list was file-authoritative: `targets.toml` was the source of truth and the database was a projection that was *purged* to match the file on every load/reload (`purge_targets_not_in`). That left no place to add a target interactively — a UI/API write to the database would be wiped on the next reload. We wanted to add/remove/enable/disable targets from the web UI.

## Decision

Make the database authoritative. `targets.toml` seeds the database only when it is empty (first run). The scheduler loads its running set from the database. New endpoints — `POST /targets`, `DELETE /targets/:id`, `PATCH /targets/:id` (all bearer-token protected) — manage targets at runtime. `POST /targets/reload` is redefined from "purge to match the file" to "import the file (upsert, never delete)". `purge_targets_not_in` is removed.

## Considered Options

File-authoritative (server rewrites `targets.toml`, then reloads) was the alternative. Rejected: rewriting the file loses comments/formatting, needs the file to be writable (Docker bind-mounts may be read-only), and needs a write lock. DB-authoritative removes code instead of adding it and lets SQLite own concurrency.

## Consequences

The `targets` table gained an `interval_secs` column so a database-reconstructed `Target` keeps its per-target interval; this bumped the schema to `user_version = 2`, which drops and recreates tables on upgrade (acceptable — targets re-seed from the file). Hand-edits to `targets.toml` after first run do nothing until an explicit reload/import. `TargetStatus` now carries `enabled` so the UI can show and toggle state.
