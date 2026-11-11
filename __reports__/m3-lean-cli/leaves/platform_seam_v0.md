# platform_seam: leaf report (v0)

## What changed

| Commit | Change |
|---|---|
| `test(repo): freeze the unix-only platform surface in an inventory` | `tests/vectors/platform_inventory.toml`: 21 `[[file]]` entries, all in `candler-core`; `tests/guard_platform_seam.rs` with `q1_unix_apis_stay_inside_the_inventory` and `q1_every_inventory_entry_names_its_use_and_a_windows_counterpart` |
| `build(repo): refuse to build the binary for a non-unix target` | one `compile_error!` under `cfg(not(unix))` in `main.rs` that names WSL2; `q1_the_binary_states_its_targets_with_one_compile_error`; the seven `cfg(unix)` lines of `common.rs`, the `cfg(unix)` on `evaluate::run` and its `cfg(not(unix))` twin are gone |

Inventory rows per module: `supervise` 6, `jobs` 6 (`lock`, `slots`, `events`, `runtime/session`, `runtime/reap`, `runtime/sweep`), `isolation` 2, `runner` 2, `store` 3, `evaluation/context` 1, `import` 1. Total 21. The binary, `candler-schemas`, `candler-harness` and the adapter have none.

## Bites

| Guard | Mutation | Result |
|---|---|---|
| `q1_unix_apis_stay_inside_the_inventory` | empty inventory (header only) | red, names the 21 files |
| same | `use std::os::unix::fs::PermissionsExt as _;` appended to `commands/evaluate.rs` | red, names `crates/candler/src/commands/evaluate.rs`; reverted |
| `q1_every_inventory_entry_names_its_use_and_a_windows_counterpart` | the first entry's `windows` set to `""` | red, names `supervise/signal.rs`; restored |
| `q1_the_binary_states_its_targets_with_one_compile_error` | before the macro was added; then the macro removed from `main.rs` | red both times; restored |

## Found beyond the plan

- `common.rs` had `#[cfg(unix)]` gates and no `cfg(not(unix))` arm (D7 says two arms): only `evaluate.rs` held a stub.
- `candler-core`'s `store/{blob,index,sync}.rs` use unix APIs although `lib.rs` does not gate `store`, so core does not compile on a non-unix target today. They are in the inventory and not gated.
- `supervise/mod.rs` calls `Signal::KILL` only in the `test-support` `kill_all`.
- `rustfmt` splits the `compile_error!` call over three lines, so the third guard reads the call up to its `);`.
- `rustup target list --installed` was not consulted; the Windows target check of Step 2.5 was not run.

## Behaviour change, reported and not marked

R10 §3: the binary refuses to compile on a non-unix target and says so naming WSL2. It never ran there. The coordinator rules before the merge. The commit is `build(repo)`, which `cog.toml` allows and which does not release.
