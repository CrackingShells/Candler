# silent_acceptance: leaf report (v0)

Status: partial. Three of the six cases are refused; two need files outside the leaf's OWNED set.

## First reads
- `__roadmap__/diet/shed/silent_acceptance.md`, diet plan "What the codebase sheds" and "Compat and breaking changes", K5.
- `resolve/layers.rs` (`ResolveInput`, `Layered::new`), `resolve/steps/env.rs` (`check`), `resolve/preflight.rs` (`ResolveError`), `resolve/refusal.rs`.
- `runner/plan.rs` (`Levels::replicates`), `runner/refusal.rs`, `jobs/orchestrate/servers.rs`, `jobs/refusal.rs`.
- The leaf's `runner/preflight.rs` does not exist; the CLI's `check_replicate_count` is in `crates/candler/src/commands/preflight.rs`.

## Bites
Guard: `crates/candler-core/tests/it/guard_silent_acceptance.rs`, seven rows. The Step 1 commit is red by design: all seven rows returned `None` on the tip.

| Case | Red on tip | Green | Code | Pointer | Check lives in |
|:--|:--|:--|:--|:--|:--|
| `env` name equals `model.token_secret` | yes | yes | `experiment.env_token_secret` | `/env/<NAME>` | `resolve/steps/env.rs::check`, called from `Layered::new` |
| `replicates.<x>` path | yes | yes | `experiment.replicates_path` | `/replicates/x` | `resolve/layers.rs::check_replicates_input` |
| replicate count is a string | yes | yes | `experiment.replicates_not_count` | `/replicates` | same |
| replicate count is a fraction | yes | yes | same | `/replicates` | same |
| `[fixed] replicates` differs from the file's count | yes | still red, `#[ignore]` | `experiment.replicates_conflict` (planned) | `/replicates` | planned: `Levels::replicates` in `runner/plan.rs` |
| MCP server `command` not a string | yes | still red, `#[ignore]` | `jobs.server_command_not_string` (planned) | `/mcp/servers/<key>/command` | planned: `stdio_servers` in `servers.rs` |
| MCP server `args` not all strings | yes | still red, `#[ignore]` | `jobs.server_args_not_strings` (planned) | `/mcp/servers/<key>/args` | same |

`ResolveInput::secret_names` is required: `ResolveInput::new(secret_names)` replaces `Default` and the `.secret_names()` setter. This is a compile-time fact, so it has no row.

## Breaking changes
- Refused now: an `env` name equal to `model.token_secret`; a `replicates.<x>` path; a `replicates` value that is not a whole number (string, fraction, negative, boolean).
- Library: `ResolveInput::default()` and `.secret_names()` are removed.
- No existing fixture or test input became refused; the suite passes with no vector edited.
- Not yet refused: the three deferred rows above.

## Ownership deviations
- Mechanical `ResolveInput::default()` to `ResolveInput::new(Vec::new())`, and the three `.secret_names(..)` sites folded into `new(..)`: `crates/candler/src/commands/{common,preflight}.rs` and the test files `crates/candler-core/tests/it/{guard_declared_inputs,guard_egress_setting,guard_ground_truth_refusal,guard_image,guard_plugin_environment,guard_registry,guard_resolve,guard_resolve_surface,guard_run_plan,guard_secrets_step,guard_session,guard_settings_out_of_cell,guard_treatment_context,guard_treatment_source}.rs` and `support/{harness_facts,plugin_env}.rs`.
- `src/jobs/orchestrate/servers.rs` is owned: `stdio_servers` and `Stdio` made `pub` so the guard reads them.
- Not edited, by the brief: `runner/refusal.rs` and `jobs/refusal.rs`.

## Follow-ups
- Own `runner/refusal.rs` and `jobs/refusal.rs` (the leaf's regex `refusal[a-z_/]*\.rs` matches only `src/refusal*.rs`), then:
  1. `PlanError::ReplicatesConflict { fixed, file }`: in `Levels::replicates`, refuse when the first level's `replicates` setting has source `ExperimentFile` and differs from `file.replicates()`; take the count at the top of `plan_runs`, before the lock. Add its row (`experiment.replicates_conflict`, pointer `/replicates`) in `runner/refusal.rs` (`plan_error_path`, `plan_answer`, the table).
  2. `ServerCheckError::{CommandNotString, ArgsNotStrings}`: make `stdio_servers` return `Result<Vec<Stdio>, _>`, `?` it in `check_servers`, add a path function and rows in `jobs/refusal.rs`.
  3. Append the three codes to `codes.toml`, run the bless command, and replace the guard's `drop(stdio_servers(..)); None` with `.err()`, move the rows from `DEFERRED` to `ROWS`, and drop the `#[ignore]`.
- Second owner of the replicate count: `check_replicate_count` in `crates/candler/src/commands/preflight.rs` still refuses a non-integer count with `NotPositive`, ahead of the resolve check. Remove it in the same follow-up so the check lives once.
- `Levels::replicates` still falls back silently to the file's count for a `0` or a count over 32 bits; the CLI refuses those first, the core API does not.
- `guard_known_truth::kt_two_levels_end_to_end` failed once under a loaded machine and passed alone and in the final full run.

## Verifier

A fresh Sonnet verifier given the leaf, this report and the diff: **PASS with follow-ups**, no dishonest claim. Confirmed: `codes.toml` only appends (3 rows, 0 lines removed), each new code is emitted in `resolve/refusal.rs` and pinned by a guard row that runs; the `#[ignore]`d `DEFERRED` rows are counted as cover nowhere and their codes are not in `codes.toml`; the ownership deviations are the mechanical `ResolveInput::new(..)` swap only; no commit touches `crates/candler-schemas/`; no existing fixture is newly refused (495 passed; DIGEST 20, GOLDEN 10). Unmet, disclosed: the replicates conflict and non-string server `command`/`args` (blocked because `runner/refusal.rs` and `jobs/refusal.rs` fall outside the leaf's OWNED regex); the ownership grep prints the 18 deviation files. New follow-ups it found:
- The replicate count is now checked twice: the CLI's `check_replicate_count` (`crates/candler/src/commands/preflight.rs:269-283`) refuses non-integers first as `NotPositive`, so `experiment.replicates_not_count` is reachable only through the core API.
- `Layered::new` finds `model.token_secret` only among the file and explicit layers; a value from defaults or config would not be compared with `env` names.
- Non-string server `env` values are dropped silently too, a sibling case the leaf does not list.
No fix round: the remaining cases need files outside OWNED, which the permission system refused the implementer; they go to a follow-up piece that owns `runner/refusal.rs`, `jobs/refusal.rs` and the CLI preflight.
