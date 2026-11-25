# Leaf exec_bit: outputs keep their execute bit

Base `42714bd1` (`origin/feat/integrity`), tip commits `90d6a989` (runner) and `ddf834e9` (docs).

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Run guard `guard_run_context_files::an_executable_output_materialises_as_0555_and_a_plain_one_as_0444` (real `candler run` over the `World` fake agent, read through `assemble` and `materialise`) | PASS | Base, with only the test and the fixture helper copied in: `assertion left == right failed: the mode of the executable, left: 292, right: 365` (0444 against 0555), so it fails on the bit, not on setup. Tip: passes; the plain file is 0444. |
| Import guard `guard_run_context::an_imported_executable_materialises_as_a_private_0555_copy` | DEVIATION | Passes on the base as well: import already records the bit (`import/read.rs`, `import/assets.rs`), so the brief's "fails on the base" does not hold for it. It guards the import producer. |
| Hand-sealed executable case deleted | PASS | `an_executable_entry_materialises_as_a_private_0555_copy` removed; the `executable` field of `support::sealed_run::File` and its mode handling removed. `seal_workspace` stays: the `.claude` file-rule case still seals through it, and the runner never collects `.claude`. |
| Workspace suite | PASS | `cargo nextest run --workspace`: 752 run, 752 passed, 31 skipped (container-e2e and other gated tests). `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` clean. Contract page guards pass. |
| Digest gates | PASS, none changed | No digest fixture or vector changed: no existing fixture run has an executable output, so every existing tree digest is unchanged. |

## Digest fixtures that changed

None. Old and new values: not applicable.

## Producers checked

- `runner/exec/gather.rs` `collect_files` (feeds `collect_outputs`): now reads the mode; any of the `0o111` bits counts, as in import.
- `runner/evidence/tree.rs` `tree_json`: the only place a run's `TreeEntry` is written; it now takes the bit from `OutputDigest`.
- `import/read.rs` and `import/assets.rs`: already record the bit.
- `evaluation/context/materialise.rs`: already writes 0555 and 0444; unchanged.
- Fixture: the fake agent copies with `cp -Rp`, which keeps the mode, so `World::agent_leaves_executable` only sets the mode of the file the agent leaves.
