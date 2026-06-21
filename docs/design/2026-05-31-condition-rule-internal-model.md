# Condition Rule Internal Model

Date: 2026-05-31

## Goal

Replace the backend condition optional-field bag with an internal `ConditionRule` enum while preserving the legacy TOML/JSON/DB wire shape.

## Non-goals

- Do not change API payload shape.
- Do not change stored `conditions_json` shape.
- Do not change frontend code.
- Do not change condition behavior.

## Scope

`Condition` keeps `id` plus a rule enum. Serde remains legacy-compatible through the existing raw wire shape. Evaluation matches on `ConditionRule` directly so missing-field recovery helpers can be removed from the evaluator.

## Contracts

- All existing legacy condition strings still deserialize.
- Serialization still emits the same legacy condition fields.
- DB JSON round trips remain compatible.
- Existing evaluator behavior and browser fallback policy remain unchanged.

## Verification

Run `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.
