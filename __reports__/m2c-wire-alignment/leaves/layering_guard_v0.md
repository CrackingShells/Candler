# Leaf report: layering_guard (v0)

Branch `m2c/layering_guard`, cut from `feat/m2c-wire-alignment` at 8f3a103. Nothing pushed. The pre-condition greps all held on the base (worktree root, ancestry plus the `land design_record` merge, and the two landing greps).

## 1. Commits

| sha | subject |
|---|---|
| 5b2dad5 | `test(repo): match the i1 guard on identifier segments across the tree` |
| (this file, last commit) | `reports(repo): record the layering_guard leaf` |

Only `crates/candler-core/tests/guard_layering_i1.rs` and `crates/candler-core/tests/support/mod.rs` changed (OWNED check passes).

## 2. Gates

| gate | result | last line |
|---|---|---|
| L1 `test(/workspace_holds_no_domain_segment/)` | PASS | 1 test run: 1 passed |
| L2 `test(/plant/)` | PASS | 2 tests run: 2 passed |
| L3 `test(/renamed_domain/)` | PASS | 1 test run: 1 passed |
| whole `binary(guard_layering_i1)` | PASS | 6 tests run: 6 passed |
| STD `-p candler-core` (fmt, clippy `-D warnings --all-targets`, nextest, doc `-D warnings`) | PASS | 167 passed, 12 skipped |
| SPAWN (`guard_signal`) | PASS | 9 tests run: 9 passed |
| LINUX (`--target x86_64-unknown-linux-gnu`, `-p candler-core --tests`) | PASS (scoped to `-p candler-core`, plus its dependencies) | Finished |
| SUBJ | PASS | No errored commits |
| DRIFT, G5a | not applicable | no schemas or vector touched |

## 3. Guards

| id | Guards | Oracle | Bite |
|---|---|---|---|
| `workspace_holds_no_domain_segment` (L1) | I1: no file under any crate's `src` (listed from `crates/`), the schemas' `schema/`, `assets/` or the evaluator helper holds a denied segment; each folder reads a file | the deny literals of the file | `assets/` removed from the scan: L1 stays green, L2 red (below). Substring matching: L1 red, `segment: "weightnotallowed"` in `candler-schemas/src` |
| `plant_table_is_found_or_passes_as_written` (L2) | I1: the planted symbols are found, and `run_plan`, `ContainerPlan`, `WeightNotAllowed`, `frontier`, `problem_count`, `HTTPServer` are not | the hand-written 13-row table | see the four bites |
| `plant_inside_a_gzip_file_is_found` | I1: a name inside a gzip stream is found | the hand-written plant | not bitten separately (the gunzip path is the one `scan` uses) |
| `segments_split_on_case_and_an_allowlist_entry_excuses_one_line_only` | I1: the splitter cuts `HTTPServer` and `WeightNotAllowed`; an allowlist entry excuses its line in its file only | hand-written splits and a two-file tree | covered by the case-split bite |
| `a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store` (L3) | I1: with `tier` renamed to `band`, the oracle's run and status counts hold and neither the copy nor the store holds a denied segment | `oracle::expected_runs` and `expected_rows` over the original mini-corpus | the entry params left at `tier` would be refused by the importer (I12 guard of `import_rules`); not re-bitten |

Bites on the guard (each injected, seen red, restored):

| bite | red test and message |
|---|---|
| `assets/` out of the scan | L2: `plant "gtpyhop-core>=2\n" at assets/docker/env/z.in` left `[]` right `["gtpyhop"]` |
| splitter without case boundaries | L2: `plant "struct PlannerAxis;\n" at crates/candler/src/x.rs` left `[]` right `["planner"]` |
| whole-word matching (split on whitespace) | L2: same `PlannerAxis` row, left `[]` |
| substring matching | L2 `PlannerAxis` row left `["planneraxis"]`; L1 red with `segment: "weightnotallowed"` (the `WeightNotAllowed` identifiers of the schemas' sources) |

Pre-check of the verifier's four mutations on the real tree (appended, L1 run, restored by `git checkout`): `struct PlannerAxis;` in `commands/supervise.rs` (segment `planner`), `// planner_package` in a core file (`planner`), `gtpyhop` in `assets/docker/base/.dockerignore` (`gtpyhop`), `// tier` in `candler-schemas/src/canonical.rs` (`tier`). Each turned L1 red. The tree is clean after.

## 4. Probe answers

None for this leaf.

## 5. Findings

- **Real tree is clean (G6e probe).** The scan over every crate `src`, the schemas' `schema/`, `assets/` and `python/candler-evaluator/src` finds zero hits, so no code or schema finding, and the allowlist (`ALLOWED_LINES`) is empty. The 82 baseline hits are gone with L0 and L1.
- **Deviation, spec defect: RENAMES gains more than `("tier","band")`.** The mini-corpus holds `planner` (52 times), `problems` (1048, including the agent's `problems.py` outputs and the gz streams) and `arango`. The leaf requires the store to hold no denied segment, which is not reachable with the one row. I added `planner`->`scheduler`, `problems`->`cases`, `arango`->`graphdb`, plus the capitalised and upper-case variants (`Tier`, `TIER`, `Problems`, `PROBLEMS`, `Planner`, `Arango`), in the owned file. `TIER` is needed also for `TIER_X`; the marker pattern in the rename test is now `BAND_X_REPLAY`. The counts and statuses still equal the oracle's.
- **Deviation: `rules_without_markers` takes the entry key.** The importer refuses a named entry key that the entry lacks (I12), so one list holding both `tier` and `band` cannot serve both tests. The original-corpus test names `tier`, the renamed one names `band` (the `["scale","text","band"]` of L3).
- **Matching details.** A line is also compared by the lowercase of each whole alphanumeric run, so `PlanTrace` matches `plantrace` (the splitter alone gives `plan`, `trace`). Digits split from letters (`tier2` gives `tier`, `2`). Literals (`mcp-python-ingestion`, `D-E`) match on a boundary: no alphanumeric directly before or after, so `D-E` does not match inside a longer word.
- **Process incident, no residue.** An early `git checkout` reverted uncommitted work; it was re-applied and committed before the bites. Nothing outside the worktree was touched.
- Target dir `/private/tmp/claude-501/m2c-target-layering_guard` is deleted before this report is final.

## Fix round 1

The code commit is amended by fixup and autosquash onto 8f3a103 (the branch holds two commits; shas are in the final message). Four should-fix items, all done in the two owned files; `cargo check --all-targets -p candler-core` passes on each commit.

| item | change | L2 rows (found unless noted) | bite, seen red |
|---|---|---|---|
| 1 multi-word names, any separator | a chunk (alphanumerics, `_`, `-`, `.`) is split into segments; denied sequences `plan,trace` and `mcp,python,ingestion` match as consecutive segments. `D-E` stays an exact literal. The whole-run check is gone: `PlanTrace` now hits as `plan_trace` | `PlanTrace`, `plan_trace`, `Plan_Trace`, `PLAN-TRACE`, `plan.trace`, `plantrace`, `mcp_python_ingestion`, `mcp-python-ingestion`; passing: `run_plan`, `ContainerPlan`, `plan planning tiered` | sequences dropped: `plant "struct PlanTrace;\n" at crates/candler-harness/src/u.rs`, left `[]`, right `["plan_trace"]` |
| 2 file and folder names | every file and folder name below a scanned target is split too (reported at line 0) | `assets/planner/x`, `assets/gtpyhop_notes.txt` | path components skipped: `plant "clean\n" at assets/planner/x`, left `[]`, right `["planner"]` |
| 3 shipped files outside the folders | targets listed from `crates/`: `src`, `Cargo.toml`, `build.rs`, `build_infra.rs` (where present); plus `python/candler-evaluator/{pyproject.toml,scripts}`. A symlink below a folder fails the scan naming it; a file that is neither UTF-8, gzip of UTF-8 nor of a listed binary extension fails naming it | `crates/candler-core/build.rs`, `crates/candler/build_infra.rs`, a crate `Cargo.toml`, `pyproject.toml`, `scripts/p.py`; separate test for the symlink and a binary file under `assets/` (message names `link.txt` and `blob.bin`) | `build.rs` dropped from the list: `plant "// planner\n" at crates/candler-core/build.rs`, left `[]`, right `["planner"]` |
| 4 exact allowlist | entry = exact path below the workspace root plus exact trimmed line | self-test: `a.rs` line 2 excused; `xa.rs` and `sub/a.rs` with the same line are found | suffix match restored: left `[("a.rs", 1)]`, right `[("a.rs", 1), ("sub/a.rs", 1), ("xa.rs", 1)]` |

- **Left out of the scan, and why.** `CHANGELOG.md` (history, which names removed things by design) and READMEs (prose, not shipped code), as asked. Also not listed, since the fix round did not name them: `python/candler-evaluator/{examples,tests}`, `uv.lock` and the crates' `tests/`.
- **Binary list.** One extension, `pyc`: an untracked, git-ignored `__pycache__` that a local `uv run` leaves under `python/candler-evaluator/src` (seen on this machine). Every tracked file in the scanned folders is text, so nothing else is listed; its bytes are not read, its name is still checked.
- **Real tree.** Still zero hits across code, manifests, build scripts, paths and the schemas, so no finding and the allowlist is still empty.
- **F4 limit.** One-word lowercase forms such as `htnplanner` are not found; the guard's doc says so.
- **Gates.** `binary(guard_layering_i1)` 7 passed; STD `-p candler-core` 168 passed, 12 skipped (fmt, clippy `-D warnings`, doc `-D warnings` clean); SPAWN 9 passed; LINUX Finished; SUBJ and the real-tree mutations below.
- **Real-tree mutations, re-run, each red then restored:** `PlannerAxis` in `commands/supervise.rs` (`planner`), `planner_package` in a core file (`planner`), `gtpyhop` in an `assets/` file (`gtpyhop`), `tier` in a schemas source (`tier`), plus `// plan.trace` in `crates/candler/build.rs` (`plan_trace`).

## Fix round 2

The scan skips, by exact name and without entering a skipped folder, the files `.DS_Store` and the folders `__pycache__`, `.pytest_cache`, `.mypy_cache` and `.venv` (`SKIPPED_NAMES`, field `skip` of the rules), so untracked tool and OS junk cannot fail it on a developer's machine. The `pyc` entry of the binary list is removed: it was only there for `__pycache__`, so `pyc` is not needed and the list is empty. New rows in the symlink/binary test: a binary `.DS_Store` under `assets/` passes, a `__pycache__/x.pyc` under the evaluator's `src` passes, and a binary `notes.dat` fails naming it (and passes once `dat` is listed). Bite (skip dropped): the test is red at the `.DS_Store` row, and `workspace_holds_no_domain_segment` is red on the local `__pycache__/*.pyc` ("is neither UTF-8 text, gzip of UTF-8 text nor a listed binary type"). Gates: `binary(guard_layering_i1)` 7 passed; STD `-p candler-core` and SUBJ below.

## Fix round 3

F6: a committed file under a skipped name was never read. `workspace_tracks_no_file_under_a_skipped_name` lists the tracked files with `git ls-files -z` at the workspace root and fails naming each tracked path, inside a scanned target, that has a `SKIPPED_NAMES` entry as a path component; untracked junk stays skipped. Guard `tracked_check_names_the_tracked_file_under_a_skipped_name_only`: a temp git repo with a committed `src/__pycache__/x.rs` and an untracked `.DS_Store`; the check names the tracked file only (oracle: the hand-written repository). Bite (the skipped-name filter made to keep nothing): `left: []`, `right: ["src/__pycache__/x.rs"]`. The allowlist's doc states the known limit: an entry excuses every line of that exact text in that file, so two identical lines of one file are excused together (the list is empty).

- **Scope deviation, with a finding.** The first version checked the whole repository and failed on the real tree: `crates/candler-core/tests/fixtures/mini-corpus/20260806-035940_v0.2.3_..._htn-creator-v0.2.3/.DS_Store` is tracked (a Finder file inside the legacy fixture, binary). It is outside every scanned target (a test fixture, never shipped or scanned), so the check is scoped to paths under the scanned targets, and the real tree passes. Report: remove that tracked `.DS_Store` from the fixture if the coordinator wants the check repo-wide; not done here (outside OWNED).
- Real tree: `workspace_tracks_no_file_under_a_skipped_name` PASS; whole `binary(guard_layering_i1)` 9 passed.
