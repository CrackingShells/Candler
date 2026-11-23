# shed_dead_dup: leaf report v0

Leaf: `__roadmap__/diet/shed/shed_dead_dup.md`. Branch `diet/shed_dead_dup`, cut from `feat/diet` at `01f137a1`.

## First reads

Each item of the leaf was re-checked with colgrep before it was touched. Planning-time line numbers had moved, and two of the five walkers were already on `tree.rs Walk`.

| Item | State at the tip | Action |
|:--|:--|:--|
| `PlanOutcome::summary`, `PlanSummary` | live code; only caller was `guard_run_plan::the_summary_of_a_plan_holds_the_caps_of_the_file`; no re-export | removed with the test, and the `Budget` import it used |
| `jobs/runtime/session.rs digest_of` | hand-written hex loop; callers in `launch.rs` (`spec_digest`, public) and `evaluate.rs`, both outside OWNED | body is now `Ok(Digest::of_bytes(..))`; `read_spec` calls `Digest::of_bytes` directly |
| `import/read.rs digest_of` | pass-through, but re-exported by `import/mod.rs` and used by `jobs/orchestrate/controls.rs`, both outside OWNED | kept |
| `docker_failed` in `image.rs` and `container/guard.rs` | return `ImageError` and `GuardError`, which are different types with different detail text | kept, two mappers |
| `experiment/inputs.rs`, `resolve/steps/ground_truth.rs` walkers | already call `Walk` (the table in `tree.rs` lists both) | nothing to do |
| `import/raw.rs walk`, `import/read.rs list/walk/walk_outputs` | differ from `Walk` (below) | kept |

## Removed and folded

| Change | Lines |
|:--|--:|
| `PlanSummary`, `PlanOutcome::summary` (`runner/plan.rs`) | -65 |
| their test (`guard_run_plan.rs`) | -73 |
| `session.rs digest_of` body, one `digest_of` call in `read_spec` | about -8 |

`cargo nextest run -p candler-core`: 348 tests before, 347 after (the summary test), 22 skipped both times.

## Kept, with the reason

| Item | Why it stays |
|:--|:--|
| `import/raw.rs walk` | sorts per directory by name, not by the joined path string (`a-b` against `a/b` order differs); skips a link only when its name is a candidate and records it as `SymbolicLink`; records a non-UTF-8 name as `Unreadable` and goes on; reports an unreadable subdirectory as skipped and goes on. `Walk` fails the whole walk on each of these. |
| `import/read.rs Reader::list/walk/walk_outputs` | typed `ImportError` per path; a link or special entry is a `Layout` error, a non-UTF-8 name is an `ImportError::Path`; per-directory sort; `walk_outputs` records empty directories; `skip_reason` skips entries by format rules. `tree.rs` already states that the import readers keep their own walkers. Folding would change the error rules. |
| `import::digest_of` (read.rs) and `session::digest_of` | their callers are outside OWNED (`controls.rs`, `import/mod.rs`, `launch.rs`, `evaluate.rs`). `session.rs` carries `#[expect(clippy::unnecessary_wraps)]` for the `Result` its callers chain. A follow-up that owns those files can inline both. |
| both `docker_failed` | different error types and detail text (`ImageError::Build` through `build_failed`, `GuardError::Docker`). One shared mapper would need `From` impls in the error modules, which is more code than the two lines it saves. |
| `commands/ls.rs` | as the plan said. |

## Probe output (step 3)

A throwaway test (`tests/probe_codes.rs`, deleted) listed every row of `candler_core::refusal::catalog()` whose `Enum::Variant` appears nowhere in the sources of the four crates. Five rows had no prefixed site:

| Code | Variant | Verdict after reading the sources |
|:--|:--|:--|
| `isolation.image_service_unavailable` | `ImageError::Unavailable` | no construction site anywhere (`ImageError::Unavailable` finds nothing, no `Self::Unavailable` in `image.rs`): proven unemitted |
| `candler.internal` | `SlotError::Unavailable` | no site found, but the code is the shared fallback, not a row of its own |
| `candler.internal` | `JobViewError::Unavailable` | same |
| `isolation.plugin_environment_missing` | `PluginEnvironmentRefusal::Missing` | built with `Self::Missing` in `from_check`: emitted |
| `isolation.ground_truth_exposed` | `GroundTruthRefusal::Exposed` | built with `Self::Exposed` in `from_check`: emitted |

So one row is proven, not eight. The M6b "eight" were an unproven count; the probe could not confirm seven of them.

**No `retired` was added, and `codes.toml` is unchanged.** `guard_refusal_codes::r1_every_variant_code_equals_its_row` demands that a retired row names a live replacement row of the same variant and side, and that no refusal table emits a retired code. `ImageError::Unavailable` still exists and its table row (in `isolation/refusal.rs`, outside OWNED) still emits `isolation.image_service_unavailable`; no replacement code exists. Retiring the row would turn that guard red. The honest ways out are a lift sentence plus removing the variant and its table row, or removing the variant and retiring, both of which touch files outside OWNED and change the public enum. That is a decision for the owner. No `fix(schemas)` commit was made.

## Bites

| Guard | Bite in product code | Seen red | Reverted |
|:--|:--|:--|:--|
| `guard_build_context` (file) | `Walk::list` sorts descending (`tree.rs`) | one of three red: `the_files_of_a_run_leave_out_links` (the run's file list order). `guard_build_context` and `a_staged_copy_has_the_digest_of_its_listing` stay green: the digest is over a `BTreeMap`, so listing order does not reach it. | yes, `git checkout` of `tree.rs` |

The ordering of `Walk` is therefore held by the run-files row only, not by the digest vector.

## Gates

| Gate | Result |
|:--|:--|
| `cargo nextest run -p candler-core` | 347 passed, 22 skipped |
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | pass |
| DIGEST (`cell_digest|context_digest|build_context`) | 20 passed, no vector edited |
| GOLDEN | 10 passed, no vector edited |
| Ownership | prints nothing (see below) |

## Follow-ups

- Inline `import::digest_of` and `session::digest_of` in a leaf that owns `import/mod.rs`, `controls.rs`, `launch.rs`, `evaluate.rs`.
- Decide `ImageError::Unavailable`: remove the variant and its row from `isolation/refusal.rs`, then lift `isolation.image_service_unavailable` in `codes.toml`.
- Check the two `Unavailable` variants that map to `candler.internal` (`SlotError`, `JobViewError`): no construction site was found.

## Verifier

A fresh Sonnet verifier given the leaf, this report and the diff: **PASS with follow-ups**. No behaviour changed, no code row deleted, every gate command passes (347 passed, 22 skipped; DIGEST 20, GOLDEN 10, no vector edited; ownership clean). It confirmed each kept item's reason in code: the two `docker_failed` mappers return different error types; `inputs.rs` and `ground_truth.rs` were already on `Walk`; the `ImageError::Unavailable` row cannot take `retired` while `isolation/refusal.rs` still emits it (`guard_refusal_codes::r1_every_variant_code_equals_its_row`). Gates left unmet, with reasons it accepted: one `docker_failed` mapper (needs `From` impls outside OWNED), the walker fold (the three import walkers differ in ordering, error typing and empty-directory recording), the retirement (needs the variant removed first). The leaf's bite named `guard_build_context`; Walk's ordering is in fact guarded only by `the_files_of_a_run_leave_out_links`. One load-sensitive flake seen outside the diff: `guard_container_record::stopped_run_has_its_agent_recorded_before_the_hook_sweeps` failed once under load and passed on rerun. No fix round: nothing it found is fixable inside OWNED.
