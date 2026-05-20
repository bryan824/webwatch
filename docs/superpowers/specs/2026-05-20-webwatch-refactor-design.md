# webwatch refactor — "less is more"

Date: 2026-05-20
Status: design, not yet implemented
Approach: tighten in place. No feature removal, no config migration, no API break.

## Goals

- Cut redundant surface area without dropping any feature in use.
- Make every layer obvious on first read.
- Keep behavior identical: same TOML configs work, same `/health` `/targets` `/notify/status` shapes, same SQLite schema (`PRAGMA user_version = 1`), same Discord webhook contract, same `--features persistence-{diesel,sqlx,seaorm}` matrix.

## Non-goals

- No new features.
- No condition vocabulary added or removed.
- No config migration step for the operator.
- No new dependencies.
- No project rename.
- No CDP browser code changes.

## Headline targets

| Metric | Today | After |
|---|---|---|
| `src/` total LOC | ~2,400 | ~1,500 |
| `db.rs` LOC | 726 | 4 files × ~150 |
| Config examples | 3 | 1 (+ docker) |
| Condition match-arms in `evaluator.rs` | 9 | 5 + a `negate` flag |
| `Error` enum variants | 26 | 22 |
| Model layers for targets/conditions | 2 (wire + domain) | 1 |
| Discord embed fields in alert | 2 | 0 |

## Module layout

```
src/
  main.rs            // unchanged shape
  lib.rs             // pub mod list
  config.rs          // single model layer (Target, Condition, AppConfig)
  evaluator.rs       // flat dispatch on 5 kinds + negate
  http.rs            // axum router + handlers
  discord.rs         // terse renderer
  scheduler.rs       // unchanged structure; retry-once removed
  browser.rs         // unchanged
  error.rs           // browser variants grouped into one
  db/
    mod.rs           // Persistence trait, connect(), backend_name(),
                     //   shared SQL constants, status_from_parts,
                     //   engine_to_str / str_to_engine
    diesel.rs        // #[cfg(feature = "persistence-diesel")]
    sqlx.rs          // #[cfg(feature = "persistence-sqlx")]
    seaorm.rs        // #[cfg(feature = "persistence-seaorm")]
```

`src/models.rs` is deleted. Its types move into `config.rs`.

## §1. Single model layer

`TargetConfig` ↔ `Target` and `ConditionConfig` ↔ `Condition` collapse:

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Target {
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "deserialize_url")]
    pub url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub interval_secs: Option<u64>,
    pub conditions: Vec<Condition>,
}

// Condition does NOT derive Deserialize/Serialize. See §2 for the manual
// impls that map legacy 9-string `kind` values to (ConditionKind, negate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    pub id: Option<String>,         // resolved post-load to "condition-N"
    pub kind: ConditionKind,
    pub negate: bool,
    pub value: Option<String>,
    pub selector: Option<String>,
    pub threshold_cents: Option<i64>,
    pub price_selector: Option<String>,
}
```

`AppConfig::resolve_env_and_validate` keeps doing condition-required-field checks and id assignment in a single pass. The URL is validated by `deserialize_url` calling `url::Url::parse`.

`Target::from_config` and its tests go away. `models.rs` is deleted.

## §2. Conditions: 5 kinds + negate

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    Text,             // value in page text
    Selector,         // selector matches at least one element
    SelectorText,     // selector matches an element whose text contains value
    Price,            // observed price compared to threshold (negate flips below↔above)
    PriceObserved,    // any price observable (replaces price_changed)
}
```

Backward compatibility for TOML, HTTP, and stored JSON uses a manual `impl Deserialize for Condition` / `impl Serialize for Condition` pair (not `derive`d) that maps the existing 9 `kind` strings to `(kind, negate)` and back. Implementation sketch: deserialize into an intermediate `#[derive(Deserialize)] struct ConditionRaw` whose `kind` is a string, then translate to `Condition`. Serialize reverses the map. The mapping is the table below:

| Wire string | kind | negate |
|---|---|---|
| `text_appears` | `Text` | false |
| `text_disappears` | `Text` | true |
| `selector_exists` | `Selector` | false |
| `selector_missing` | `Selector` | true |
| `selector_text_contains` | `SelectorText` | false |
| `selector_text_not_contains` | `SelectorText` | true |
| `price_below` | `Price` | false |
| `price_above` | `Price` | true |
| `price_changed` | `PriceObserved` | false |

`evaluate_condition` becomes:

1. Match on the 5 base kinds to compute `matched` and `evidence`.
2. If `condition.negate`, flip `matched` and rewrite the evidence verb (`contains` → `does not contain`, `matched N` → `did not match`, `below` → `above`).
3. Return `ConditionResult` unchanged in shape.

The `should_try_browser` table compresses to one line: `matches!(kind, Text | Selector | SelectorText | Price | PriceObserved)` — the browser fallback is allowed on any positive (non-negated) miss when the page looks JS-shell. Negative conditions don't trigger browser fallback (current behavior preserved).

## §3. `db.rs` split

`src/db/mod.rs` holds, once:

- `pub trait Persistence` (unchanged signature).
- `SCHEMA_VERSION`, `DROP_TABLES`, `CREATE_TABLES`, `STATUS_SQL` constants.
- `pub async fn connect(path) -> Result<Box<dyn Persistence>>` delegating to the active backend module.
- `pub fn backend_name() -> &'static str` delegating likewise.
- `engine_to_str` / `str_to_engine`.
- `StatusParts` struct + `status_from_parts` + `parse_json`.
- Shared status default `Persistence::status(&self, target_id)` (the `find` fallback already lives here).

`src/db/diesel.rs`, `sqlx.rs`, `seaorm.rs` each contain only:

- The struct + `Pool`/`Connection` field.
- The 5 `impl Persistence` methods (`migrate`, `ensure_target`, `record_success`, `record_error`, `mark_alert_sent`, `statuses`).
- A small backend-local `db_err` adapter and any backend-specific helpers (e.g., the diesel `spawn_blocking` wrapper, the sqlx `db_err`, the seaorm `stmt`/`sea_get`).

Backend files reference the shared constants by `use super::{SCHEMA_VERSION, CREATE_TABLES, …}`.

Target sizes: `mod.rs` ~180 lines, `diesel.rs` ~200 lines, `sqlx.rs` ~140 lines, `seaorm.rs` ~160 lines.

## §4. Discord output

`send_condition_alert`:

- `content`: `"🚨 {name} — {first evidence line or 'condition matched'}"`.
- One embed with `title = name`, `url = target.url`, `description` containing remaining evidence lines (newline-joined, max 5 lines, truncated at 1024 chars).
- No `fields[]`. Engine and price are inlined into the description only when meaningful (`Engine: BrowserCdp` or a non-`None` price).

`send_status_report` / `render_status_report`:

- Header: `Checked N target(s): M matched, E error(s).`
- One block per target:
  ```
  {icon} **{name}** — {state}
  {url}
  {last_check or 'never checked'} · {first evidence line or 'Error: ...'}
  ```
- No engine line, no price line, no conditions-summary line.
- `send_status_report` passes the rendered string as the message `content` directly; the dummy `https://example.invalid` embed URL goes away.

Target: `discord.rs` from 247 → ~120 lines. Existing test `status_report_includes_counts_url_and_condition_summary` is updated to match the new block format; the new format still contains the URL and the matched/total counts header, so the assertion intent is preserved.

## §5. Error enum

Replace these five variants:

- `BrowserConnect { url, message }`
- `BrowserSend { method, message }`
- `BrowserRead { method, message }`
- `BrowserProtocol { method, message }`
- `BrowserResponseMissing { method, field }`

with one:

```rust
#[snafu(display("browser CDP {stage} failed: {message}"))]
Browser { stage: &'static str, message: String },
```

Call-sites pass `stage = "connect" | "send" | "read" | "protocol" | "response_missing"` and pack url/method/field into `message`. Net: 26 → 22 variants, and `browser.rs` simplifies its error mapping closures.

## §6. Scheduler retry

`check_with_retry` is removed. `scheduler::run_once` calls `evaluator::check_target` once; failures go straight to `db.record_error` and the next tick is scheduled normally (5 min ±30 s).

Rationale: a 5-second in-tick retry doesn't meaningfully recover transient HTTP errors at low-frequency polling, and removing it makes the log story linear (one tick, one outcome). Alerts only fire on transitions, so a single dropped check has no Discord effect.

## §7. Config example consolidation

Delete:

- `config.toml.example`
- `config.advanced.toml.example`

Keep:

- `config.docker.toml.example` (the docker path differs).
- A new single `config.toml.example` whose body is the basic block, with the `[browser]` block and a second `[[targets]]` block both commented out underneath, each preceded by a short `# Uncomment to ...` comment.

README's "JavaScript-rendered pages" section becomes: "Uncomment the `[browser]` block in `config.toml` and run a Lightpanda container."

## §8. HTTP API surface

No route changes. Internal cleanups only:

- `check_target_by_id` keeps its current behavior (run a live check, record success or error, return `Result<(), Response>`). The `mark_manual_report` flag is preserved.
- Authorization helpers (`authorize_optional`, `authorize_required`, `authorize_token`) stay; `authorize_token` is the only one that touches headers, the others just gate it. No simplification beyond what's already there.

## §9. Persistence schema

No schema change. `PRAGMA user_version` stays at `1`. The `conditions_json` column stores the new `Condition` shape with the `negate` field present; the legacy `kind` strings remain on the wire thanks to the custom `Serialize` impl, so existing databases continue to parse without a migration.

## §10. Test plan

Unit tests that stay green without change:

- `models::tests::builds_generic_target_from_url_and_conditions` is replaced by a `config::tests` equivalent that loads a TOML literal and asserts the resolved `Target`/`Condition`.
- `evaluator::tests::extracts_first_price` — unchanged.
- `evaluator::tests::evaluates_selector_text_condition` — unchanged.
- `scheduler::tests::delay_never_zero` — unchanged.
- `browser::tests::*` — unchanged (config-only).

New unit tests:

- `config::tests::deserializes_legacy_condition_strings` — every one of the 9 legacy strings round-trips through `(kind, negate)`.
- `config::tests::serializes_back_to_legacy_strings` — `Condition { kind: Text, negate: true, … }` serializes `kind = "text_disappears"`.
- `evaluator::tests::negates_text_condition` — confirms `negate: true` flips the result and rewrites the evidence verb.
- `discord::tests::condition_alert_uses_target_url_as_embed_url` — confirms the `example.invalid` URL is gone.

Integration tests unchanged:

- `tests/http_fixture.rs` — three tests against the in-process axum fixture, all pass without modification because the public types (`AppConfig`, `TargetConfig`, `ConditionConfig`, `evaluator::check_target`) keep their names. Internally `TargetConfig` becomes an alias `pub use Target as TargetConfig;` and `ConditionConfig` becomes `pub use Condition as ConditionConfig;`. These aliases stay until the integration tests are updated in a follow-up commit; nothing forces their removal.
- `tests/persistence_backend.rs` — runs against the compiled-in backend; passes for all three Cargo features.

Verification matrix to run after each phase:

```bash
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --no-default-features --features persistence-sqlx --all-targets -- -D warnings
cargo test --no-default-features --features persistence-sqlx
cargo clippy --no-default-features --features persistence-seaorm --all-targets -- -D warnings
cargo test --no-default-features --features persistence-seaorm
```

## §11. Phasing

Each phase is one commit, independently verifiable.

1. **`db/` split.** Move `src/db.rs` → `src/db/mod.rs` + 3 backend files. No behavior change. Full verification matrix passes.
2. **Model collapse.** Delete `src/models.rs`, move types into `src/config.rs`, add `deserialize_url`, add `pub use` aliases so external callers don't change. Full matrix passes.
3. **Conditions: 5 kinds + negate.** Add `negate` field, custom `Deserialize`/`Serialize` for legacy strings, collapse `evaluate_condition` match. Add new unit tests. Full matrix passes.
4. **Discord trim.** Rewrite `send_condition_alert` and `render_status_report`. Update existing Discord test. Full matrix passes.
5. **Error enum group + scheduler retry removed.** Collapse 5 browser variants into one, delete `check_with_retry`. Full matrix passes.
6. **Config example consolidation.** Delete two example files, write the unified one, update README. No code change; `cargo build` only.

Total: 6 commits, each safely revertable, each preserving the operator-facing contract.

## §12. What this changes for each user

- **Operator:** one config example, terser README on the JS-rendered path, identical TOML works after upgrade. Logs become linear (one tick → one outcome).
- **Discord recipient:** alerts have a clickable real URL on the embed, no `example.invalid` placeholder, status reports lose the `Engine` / `Price` / `Conditions: 1/1 matched` clutter, one block per target.
- **Maintainer:** `db.rs` is four readable files instead of one 726-line file; one model layer instead of two; evaluator dispatch is half as wide; the error enum has one browser variant instead of five.

## §13. Out of scope (explicitly)

- Dropping Cargo features for SQLx/SeaORM — kept in use.
- Dropping the CDP browser fallback — kept in use.
- Dropping HTTP API endpoints or the bearer-token auth — kept in use.
- Dropping multi-target support — kept in use.
- Rewriting the CDP client — works as-is.
- Adding new condition kinds or new alert channels.
- Any change to `Cargo.toml` beyond what the `src/` move requires (nothing expected).
