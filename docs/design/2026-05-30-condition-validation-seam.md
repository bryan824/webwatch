# Backend Condition Validation Seam

Date: 2026-05-30

## Context

The current backend condition model is a legacy-compatible optional-field bag: `Condition` stores `kind`, `negate`, `value`, `selector`, `threshold_cents`, and `price_selector`. The evaluator currently recovers missing-field invariants at runtime through helper functions like `required_value`, `required_selector`, and `required_threshold`.

Before replacing the condition model with a deeper `ConditionRule` enum, move the important invariant earlier: accepted targets should not contain conditions missing the fields required by their kind.

## Goal

Reject invalid conditions during target validation/API creation instead of allowing them to fail later during evaluation.

## Non-goals

- Do not introduce a `ConditionRule` enum yet.
- Do not change TOML/JSON wire format.
- Do not change DB JSON shape.
- Do not rewrite evaluator logic.
- Do not change frontend code.
- Do not reject irrelevant extra fields in this slice.

## Contracts and invariants

After this slice, `Target::validated()` and `TargetsFile::resolve_and_validate()` must guarantee:

- text conditions require `value`.
- selector conditions require `selector`.
- selector-text conditions require both `selector` and `value`.
- price comparison conditions require `threshold_cents`.
- price-observed / `price_changed` conditions do not require an extra field.
- `price_selector` remains optional for all price-related conditions.
- Missing-field errors identify the condition ID; auto-assigned IDs like `condition-1` are acceptable.

Existing valid targets remain valid. The legacy condition wire strings remain unchanged.

## Recommended approach

Add a small backend validation helper over the existing `Condition` representation, likely as a method on `Condition` in `src/config.rs`.

Call it from `Target::validate_and_resolve()` after auto-assigning missing condition IDs, so errors can reference a stable condition ID.

Use the existing `MissingConditionField` error variant for required fields. This preserves error style and avoids new API response shape.

## Tests

Add Rust tests for target validation:

- `text_appears` without `value` is rejected.
- `selector_exists` without `selector` is rejected.
- `selector_text_contains` missing either required field is rejected.
- `price_below` without `threshold_cents` is rejected.
- `price_changed` without `price_selector` remains valid.

Add an API test for `POST /targets` returning `400 Bad Request` when a submitted condition is missing a required field.

Existing config, evaluator, persistence, and reload tests should continue to pass.

## Verification

Run:

```bash
cargo test
```

No frontend verification is required because no frontend files should change.

## Rollback boundary

Rollback is limited to removing the validation helper, removing its call from target validation, and removing the new tests. Wire format, persistence, and evaluator behavior remain unchanged.

## Acceptance criteria

- Invalid condition definitions are rejected before evaluation.
- Existing valid targets still load and validate.
- `POST /targets` returns `400` for missing required condition fields.
- `cargo test` passes.
