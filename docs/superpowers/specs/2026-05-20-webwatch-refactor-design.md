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
- No new dependencies.
- No project rename.
- No CDP browser code changes.
- One operator-visible migration is in scope: splitting `config.toml`'s `[[targets]]` blocks into a separate `targets.toml`. See §14.

## Headline targets

| Metric | Today | After |
|---|---|---|
| `src/` total LOC | ~2,400 | ~1,500 |
| `db.rs` LOC | 726 | 4 files × ~150 |
| Config examples | 3 | 1 service + 1 targets (+ docker) |
| Condition match-arms in `evaluator.rs` | 9 | 5 + a `negate` flag |
| `Error` enum variants | 26 | 23 (§5 collapses 5→1, §14 adds 1) |
| Model layers for targets/conditions | 2 (wire + domain) | 1 |
| Discord embed fields in alert | 2 | 0 |
| Orphan target rows after deleting from TOML | possible | impossible (purge on load) |

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
7. **Split targets into `targets.toml` + add startup purge.** See §14. Operator-visible migration; README documents the one-time move.

Total: 7 commits, each safely revertable, each preserving the operator-facing contract.

## §12. What this changes for each user

- **Operator:** service knobs live in `config.toml`, watch list lives in `targets.toml` — edit the file that matches what you're changing. One config example each, terser README on the JS-rendered path. Removing a target now actually removes it (no orphan SQLite rows). Logs become linear (one tick → one outcome).
- **Discord recipient:** alerts have a clickable real URL on the embed, no `example.invalid` placeholder, status reports lose the `Engine` / `Price` / `Conditions: 1/1 matched` clutter, one block per target.
- **Maintainer:** `db.rs` is four readable files instead of one 726-line file; one model layer instead of two; evaluator dispatch is half as wide; the error enum has one browser variant instead of five; targets have a single source of truth (`targets.toml` → DB).

## §13. Out of scope (explicitly)

- Dropping Cargo features for SQLx/SeaORM — kept in use.
- Dropping the CDP browser fallback — kept in use.
- Dropping HTTP API endpoints or the bearer-token auth — kept in use.
- Dropping multi-target support — kept in use.
- Rewriting the CDP client — works as-is.
- Adding new condition kinds or new alert channels.
- Adding `POST /targets` / `DELETE /targets/:id` endpoints — see §14, rejected option B.
- Any change to `Cargo.toml` beyond what the `src/` move requires (nothing expected).

## §14. Split config: `config.toml` + `targets.toml`

**Problem.** Today's `config.toml` mixes two things with different lifecycles:

| Section | Lifecycle | Source of truth |
|---|---|---|
| `sqlite_path`, `user_agent`, `[server]`, `[scheduler]`, `[browser]` | set once | TOML |
| `[[targets]]` + conditions | edited often | ambiguous — TOML *and* SQLite |

On startup `main.rs` upserts each `[[targets]]` block into the `targets` SQLite table via `ensure_target`. The scheduler reads targets from the in-memory `AppConfig`, but `/targets` reads them from SQLite. There is no purge: if you delete a `[[targets]]` block from TOML and restart, SQLite keeps the orphan row, and `/targets` returns a target the scheduler isn't watching.

**Decision.** Split into two files. `config.toml` carries service knobs only. `targets.toml` carries the watch list. The DB remains the runtime store; TOML remains the seed; we add a startup purge to keep the two in sync.

**File layouts.**

`config.toml` (service config — written once):

```toml
sqlite_path = "webwatch.sqlite3"
user_agent = "webwatch/0.1 (+https://example.invalid; low-frequency page monitor)"
targets_path = "targets.toml"   # optional; default shown

[server]
bind = "127.0.0.1:3000"

[scheduler]
default_interval_secs = 300
jitter_secs = 30
http_timeout_secs = 20

# [browser]                     # uncomment to enable CDP fallback
# cdp_url = "ws://127.0.0.1:9222"
# wait_ms = 5000
```

`targets.toml` (watch list — edited weekly):

```toml
[[targets]]
id = "tom-ford-soleil-neige-eye-color-quad"
name = "Tom Ford Soleil Neige Eye Color Quad - 01 Chalet Mink"
url = "https://www.tomfordbeauty.com/products/soleil-neige-eye-color-quad?variant=52128415318229&collection=last-look"
enabled = true

[[targets.conditions]]
kind = "text_appears"
value = "Add to Bag"
```

**Code changes.**

- `AppConfig` loses its `targets: Vec<Target>` field; that data moves to a new `TargetsFile { targets: Vec<Target> }`.
- `AppConfig` gains `pub targets_path: Option<String>` with `#[serde(default = "default_targets_path")]` returning `"targets.toml"`.
- `AppConfig::load(path)` returns `(AppConfig, TargetsFile)`. The TargetsFile is resolved by joining `targets_path` against the parent dir of `config.toml` (or used absolute if absolute).
- `main.rs` is updated:
  ```rust
  let (config, targets) = AppConfig::load(&config_path)?;
  let config = Arc::new(config);
  let persistence: Arc<dyn db::Persistence> = ...;
  persistence.migrate().await?;
  persistence.sync_targets(&targets.targets).await?;   // new
  ```
- `Persistence` gains one method:
  ```rust
  async fn sync_targets(&self, targets: &[Target]) -> Result<()>;
  ```
  Default impl: `for t in targets { self.ensure_target(t).await?; } self.purge_targets_not_in(targets).await?;`. Each backend implements `purge_targets_not_in` with a single `DELETE FROM targets WHERE id NOT IN (?, ?, …)` parameterized over the current ID list (or `DELETE FROM targets WHERE 1=1` if empty — though `EmptyTargetsSnafu` already prevents an empty list). The `ON DELETE CASCADE` on `target_state` and `checks` cleans up child rows automatically.
- `scheduler::spawn_all(config, targets, db, client)` takes the targets slice explicitly instead of reading them out of `AppConfig`.
- `HttpState` keeps the same shape; `notify_status` iterates `targets` instead of `state.config.targets`. Either pass targets into `HttpState`, or have the scheduler/HTTP server share the same `Arc<Vec<Target>>`. Recommended: `HttpState.targets: Arc<Vec<Target>>`.

**Validation.**

- Missing `targets.toml` → `Error::ReadTargets { path, source }` (new variant; net error count 22 → 23 after this phase — still down from 26).
- Empty `targets.toml` → existing `EmptyTargets` error.
- `targets_path` may be absolute or relative; relative resolves against the directory of `config.toml`.
- `WEBWATCH_TARGETS` env var overrides `targets_path` (parallel to `WEBWATCH_CONFIG`).

**Backward compatibility.** None. webwatch is pre-1.0 with a single operator. The migration is: copy `[[targets]]` blocks from `config.toml` into a new `targets.toml`. README documents the one move in two lines.

**Docker.** `docker-compose.yml` mounts both files:

```yaml
volumes:
  - ./config.docker.toml:/app/config.toml:ro
  - ./targets.toml:/app/targets.toml:ro
  - webwatch-data:/data
```

**Tests.**

- New `config::tests::loads_split_files` — writes a tempdir `config.toml` + `targets.toml`, asserts they merge correctly.
- New `config::tests::targets_path_resolves_relative_to_config` — `targets_path = "subdir/targets.toml"` resolves correctly.
- New `db::tests::sync_targets_purges_removed_rows` (per backend) — insert target A and B, call `sync_targets(&[A])`, assert B is gone from `targets` table.
- Existing `tests/persistence_backend.rs` updated to call `sync_targets` instead of `ensure_target` directly (or both, since `ensure_target` stays).
- Existing `tests/http_fixture.rs` updated to construct an in-memory targets list separately from `AppConfig`.

**Rejected alternatives.**

- *B. DB-as-source-of-truth + `POST/DELETE /targets`.* Cleanest model, but expands the HTTP API surface and requires either an admin-only auth tier or expanded use of the existing bearer token. Not worth the complexity for a homelab tool with file-based config that's already in git.
- *C. Keep one file, add purge only.* Smallest change, fixes the orphan bug, but doesn't address the "scroll past infra to find targets" UX issue.
