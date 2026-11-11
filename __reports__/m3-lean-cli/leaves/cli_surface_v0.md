# cli_surface, leaf report v0

The binary now carries its container assets and runs from a copy outside its source tree, every verb has a row that runs it from an unrelated directory, every option's help ends with a stamp, `show` and `diff` take a bare evaluator version, `show --rates` infers the evaluator, the README lists the verbs the binary has, and the refusals a caller causes carry a code. The implementer was Sonnet. Branch `m3/cli_surface` is cut from a72342b and holds ten commits: one per step, then a path refactor and a doc-line format commit that the gates asked for.

## Commits

| Commit | Subject |
|:--|:--|
| 68135af | `feat(cli): embed the image assets and cache them by digest` |
| 0ef2095 | `feat(cli): infer the evaluator and take a bare version` |
| 3f5767e | `feat(cli): state why each option is not derived` |
| f07bfd3 | `test(cli): run every verb from an unrelated directory` |
| e37a588 | `docs(readme): list the verbs of the lean cli` |
| 12e008e | `fix(cli): code the refusals a caller causes in each verb` |
| 9ae6208 | `refactor(cli): embed the assets by a path relative to the source` |
| a9e45aa | `test(cli): give each surface guard its three doc lines` |
| c3d25dd | `fix(cli): carry the failing job's error without new error enums` |
| (last) | `fix(cli): register the cause carriers of kill and ps` |

## Production

| | Time (commit timestamps, +09:00) |
|:--|:--|
| Base a72342b | 2026-11-12 07:09:37 |
| First commit, 68135af | 07:24:16 |
| Last code commit, e970b9c | 2026-11-12T07:53:50+09:00 |
| Elapsed | see the timestamps; the base-to-first-commit gap includes reading the spec |

The build directory peaked near 2.1 GiB (candler and its dependencies, tests included); it was removed before the final report.

## The asset cache

**Location: `<store root>/assets/<digest hex>/docker/`**, where the digest is `digest_of` of the lines `<relative path>\0<digest of the bytes>\n` over the eleven files in path order. The spec proposed this place; the binary has no `sha2` dependency, and `ConfigEnv` knows no cache home, so the store root is the one directory every verb already holds.

- The write goes to `<store root>/assets/.staging-<pid>-<nanos>` and is renamed onto the final directory. A rename that finds the directory present counts as a lost race.
- A second call finds `docker/` and writes nothing (guard U3, second test, compares modification times).
- A new binary whose assets differ writes a new directory beside the old one. Nothing removes an old one: each is about 44 KB.
- The store root gains an `assets/` directory. Every guard that walks a store passed with it present.

## Guards and bites

Each bite was run on the working tree, seen red, and reverted, except where the table says the guard was red before the code existed.

| Guard | Bite (mutation) | Result |
|:--|:--|:--|
| `u3_a_copied_binary_runs_run` | `asset_paths` back on `CARGO_MANIFEST_DIR` (the recorded context sits in the source tree) | red: "is under the source tree" (also red before `assets.rs` was wired) |
| `u3_the_embedded_assets_equal_the_tree` | the `gw.js` entry reading `entrypoint.sh` | red: the written directory differs from the tree |
| `u5_diff_takes_bare_versions`, `u5_show_takes_a_bare_version` | the `name@` prefix required again (`evaluator_of` parses every text as `name@version`) | red; red |
| `u5_show_rates_infers_the_only_evaluator` | `requires = "evaluator"` back on `--rates` | red: clap's usage error |
| `u2_every_option_help_ends_with_a_stamp` | the stamp of `kill --all` deleted; a sentence added after the stamp of `kill --yes` | red, naming both options |
| `u1_each_verb_runs_from_an_unrelated_directory` | `ls` taking its working directory from the binary's directory | red: "the row of `ls` failed", `a.toml` not listed |
| `u1_every_verb_has_a_row` | the `analyze` row removed (stands for a verb added to clap with no row) | red, naming `analyze` |
| `u6_each_caller_mistake_carries_a_code` | the sixteen sites as they were (`CliError::refused(String)`, a text `source`) | red: fourteen sites print `candler.internal` (clap's two print no record at all); the two sites already coded passed |
| `u1_the_store_comes_from_the_project_or_the_state_home` | not bitten separately: it fails if the project store falls back to the state home, because its first half asserts `.candler/store` holds the run and the state home holds none | not run |

## Options and their stamps

Twenty-three options, for the level briefing. The thirteen marked **new** were written here, the thirteen entries of the scaffold's list; the others were stamped by earlier leaves and are listed so the table is complete. Each help line ends with the sentence below.

| Verb | Option | Stamp |
|:--|:--|:--|
| (global) | `--store` **new** | not an experiment setting: it chooses where the results of every experiment are kept |
| (global) | `--error-format` | cannot be derived: what parses the caller's stderr is not visible from the terminal; cannot live in the experiment file: a refusal can come before any file is read |
| `run` | `--set` **new** | cannot be derived: an override is what the caller types |
| `run` | `--force` **new** | cannot be derived: whether to accept differences is the caller's decision, taken after the plan has shown them |
| `kill` | `--all` **new** | not an experiment setting: it chooses which jobs the call stops |
| `kill` | `--yes` **new** | not an experiment setting: it answers the question the call would ask |
| `watch` | `--json` **new** | not an experiment setting: it is how the output is read |
| `watch` | `--live` | not an experiment setting: it is how the output is drawn |
| `ingest` | `--rules` | cannot be derived: how a legacy folder maps to runs is the importer's input, written by whoever owns the results |
| `evaluate` | `--runs` | cannot be derived: which runs to grade again is the caller's choice |
| `show` | `--rates` **new** | not an experiment setting: it chooses what the call prints |
| `show` | `--evaluator` **new** | not an experiment setting: it narrows which stored outcomes the call prints |
| `show` | `--format` **new** | not an experiment setting: it is how the output is read |
| `diff` | `--from` **new** | cannot be derived: which versions to compare is the question the caller asks of the store |
| `diff` | `--to` **new** | cannot be derived: which versions to compare is the question the caller asks of the store |
| `diff` | `--equivalence` **new** | not an experiment setting: it is the caller's list of rows to excuse from this one comparison |
| `diff` | `--format` **new** | not an experiment setting: it is how the output is read |
| `resolve` | `--set` | cannot be derived: an override is what the caller types |
| `resolve` | `--json` | not an experiment setting: it is how the output is read |
| `analyze` | `--evaluator` | not an experiment setting: names which stored outcomes to analyse |
| `analyze` | `--experiment` | not an experiment setting: names the file the recorded factors are checked against |
| `analyze` | `--format` | not an experiment setting: chooses how the tables are written |
| `analyze` | `--table` | not an experiment setting: chooses which table is written |

Points for Eliott's reading:

- `--store` is not "cannot be derived" because it can be: `store.root` in a configuration file or the project default names a store. The stamp says it is not an experiment setting.
- `show --evaluator` keeps its stamp as a filter. With `--rates` the evaluator is now inferred, and no stamp claims otherwise.
- `kill --yes` and `kill --all` are stamped "not an experiment setting" because they answer the call, not the experiment.

## Relaxations

Inputs a verb refused before and accepts now. None is marked; each is for a ruling.

1. `show --evaluator 1.0.0` and `diff --from 1.0.0 --to 2.0.0` accept a bare version when exactly one evaluator name holds it. Two holders are refused, naming both (`analysis.ambiguous_evaluator_name`).
2. `show --rates` with no `--evaluator` accepts a store that holds one evaluator. With none or several it is refused with the `analysis.*` code of `choose_evaluator`; before, clap refused the missing option with a usage error (exit 2 both ways).
3. `--rates` no longer carries `requires = "evaluator"`.

Not relaxed: a `name@version` is read as written, so a version the store lacks gives what it gave before (empty counts or rates in `show`, "nothing compared" in `diff`). This is why `evaluator_of` calls `choose_evaluator` only for a bare version or no request; the spec's wording (always) would have newly refused `show --rates --evaluator name@missing`.

## Behaviour notes (none newly refuses an input)

- The text form of every existing output is unchanged. The check order of `show` and `diff` is kept: a `name@version` that does not parse is refused before the store is opened (`check_evaluator_text`).
- The refusal that `diff` and `show` print for a malformed evaluator, `kill` without `-y`, a missing experiment file and the `inspect` selectors keeps its text and exit status. Its JSON record now has a code (below).
- `kill` and `ps` records gain entries in `causes`: the error behind the first failing job. The text form is unchanged.
- Under `--error-format json`, a usage error prints one `refusal/1` record instead of clap's text. `--help` and `--version` are unchanged. Without `--error-format json`, the usage text and status are unchanged (guarded).
- The store root gains an `assets/` directory (see above).

## Step 6 sites

Each site was run through the binary under `--error-format json` before any edit.

| Site | Before | After |
|:--|:--|:--|
| `resolve`, `run` of a missing experiment file | `candler.internal` | `config.experiment_unreadable`, side `config`, the file named in the record |
| `kill` without `-y` | `candler.internal` | `config.kill_not_confirmed`, `config` |
| `kill -y` of a missing job | `candler.internal` (a text source) | `jobs.job_not_found`, `config`, the job error carried as the source |
| `ps` with an unreadable job | `candler.internal` (a text source) | `jobs.io_failed`, `environment`, carried the same way |
| `inspect` of a selector that is none of the three forms | `candler.internal` | `config.inspect_target_invalid`, `config` |
| `inspect` of an unknown prefix | `candler.internal` | `config.no_such_run`, `config` |
| `inspect` of an ambiguous prefix | `candler.internal` | `config.ambiguous_run`, `config` |
| `inspect` of a directory that is not a run directory | `candler.internal` | `config.run_directory_invalid`, `config` |
| `show --evaluator`, `diff --from`, `diff --to` that are not `name@version` | `candler.internal` | `config.evaluator_invalid`, `config` |
| `show --evaluator <bare version>` that no evaluator holds | already `analysis.*` through `evaluator_of` | `analysis.unknown_evaluator`; the guard row holds it |
| `diff` with an evaluator the store lacks | `candler.internal` ("nothing compared") | `config.nothing_compared`, `config` |
| clap usage errors (an invalid `--set`, an unknown option) | clap's text only | `cli.usage_invalid`, `config`, clap's text as the message |
| `watch` of an unknown job | already `jobs.job_not_found` | not edited; the guard row holds it |
| `evaluate` of an unreadable suite | already `evaluation.suite_unreadable` (probed with a project that lists a plug-in directory) | not edited |

Nine rows were appended to `[cli]` of `docs/contract/codes.toml`; none is renamed or removed. The contract-page drift test (`contract_pages`) and the wrapper guard of the refusal codes both pass at the head; no page was blessed.

`CliError::refused(...)` or `CliError::Refused` still print `candler.internal` at these sites, none in Step 6's list: `evaluate.rs:149`, five in `ingest.rs` (the rules file), `ls.rs` and `resolve.rs` ("no working directory"), `resolve.rs` (the summary after printed failures), `run.rs` ("no working directory"), and `preflight.rs`'s `refused` helper. Two more are unreachable: the `no job` branch of `inspect` (a job id always looks like a key prefix, so the branch before it answers) and `show --rates needs an evaluator` (`evaluator_of` has answered by then).

## Spec defects

1. **Step 1 and the final gate contradict.** Step 1 puts `env!("CARGO_MANIFEST_DIR")` in `assets.rs` for `include_bytes!`; the gate `! git grep -n 'CARGO_MANIFEST_DIR' -- 'crates/candler/src/**'` then fails on it. The same gate failed on the `r1` guard in `refusal.rs`. Done instead: `include_bytes!` with a path relative to `assets.rs`, and `include_str!` for `codes.toml` in `r1`. Both reach the same bytes, and the gate passes.
2. **The G-verbs command is wrong at the base too.** `awk '/^Commands:/{f=1;next} f&&NF{print $1}'` does not stop at the blank line, so it also prints `Options:`, `--store`, `--error-format`, `-h,` and `-V,` (clap puts the global options after the verbs). With `f&&!NF{exit}` the diff is empty. It was not run as written.
3. **G-text against a26fe53 is red before this leaf.** `guard_cli.rs` shows 34 added and 4 deleted lines against a26fe53 in a72342b already (an earlier leaf's rewrite). This leaf touches no `guard_cli*.rs` file other than `guard_cli_surface.rs`.
4. **The embedded table cannot be reached from `guard_cli_surface.rs`.** The binary has no library, so a test crate cannot read `ASSETS`. `u3_the_embedded_assets_equal_the_tree` runs `resolve` from a copied binary and compares the written directory with the tree, in both directions (no missing file, no extra one).
5. **`u3_a_copied_binary_runs_run`'s "each build context exists under the assets" holds only for two of the three kinds.** The base and gateway contexts are the written assets. The env layer's context is staged under `<store>/images/ctx-<key>` and removed after its build, so it cannot exist afterwards. The guard checks that every context is outside the source tree and that the ones that outlive their build are the written assets, equal to the tree.
6. **The fake docker cannot be asked to build.** `support/cli_run.rs` answers every `image inspect` as found, so no `docker build` is ever called. That file is not owned. The guard puts a small wrapper first on the path that reports an image absent until a build made it, and passes every call on to the world's fake, whose log stays the record.
7. **The `resolve` row needs a stand-in for the endpoint.** The world's endpoint is `http://127.0.0.1:9`, so `resolve` fails its gateway check (the same defect `observe` reported). The row serves a local list that answers 200.
8. **The `evaluate` row needs a project, a run and `uv` on the path.** The seeded store of a sealed run is not gradeable by the plug-in, and the `candler.toml` that lists the plug-in makes the store `<working directory>/.candler/store`. The row runs `run` first (so the store is the project's), then `evaluate`, and puts the caller's `PATH` after the fakes so `uv` is found.
9. **The doc-line gate counts lines that start with `/// Guards:`, `/// Oracle:` and `/// Bite:`.** The scaffold's guards had the three in one paragraph. They are now on lines of their own.
10. **`stop_job` returned `Result<String, String>`.** Carrying the first failing job's error as the source needed a typed error, so its signature changed and `evaluate.rs` (its other caller) changed by one line. `evaluate.rs` is within the OWNED regex but not named in the spec.
11. **A typed cause needs a type, and the workspace guard `r1_every_error_enum_is_coded_or_internal` registers every error type, hand-written or `thiserror`.** `NotStopped`, `KillFailed` and `RowFailed` carry the first failing job's error. The guard asked for whole-type `[internal]` rows, so three were appended to `[internal]` of `codes.toml`: an edit outside the `[cli]`-only limit of OWNED, made because the workspace suite is otherwise red. They are append-only and code nothing. If the coordinator would rather own those rows, drop the last commit's `codes.toml` hunk and put them in `refusal_late`'s section.
12. **A verifier added a site during the leaf** (`diff --from a@1`, a version that is not SemVer, and `show --evaluator a@1`). Both already took the `config.evaluator_invalid` path; two guard rows now hold them.

## Files touched

All inside the OWNED regex:

- `crates/candler/src/commands/`: `assets.rs` (new), `mod.rs`, `preflight.rs`, `common.rs`, `show.rs`, `diff.rs`, `kill.rs`, `ps.rs`, `inspect.rs`, `refusal.rs`, `resolve.rs`, `run.rs`, `watch.rs`, `evaluate.rs`
- `crates/candler/src/main.rs`
- `crates/candler/tests/guard_cli_surface.rs`
- `docs/contract/codes.toml`
- `README.md`

## Gates

| Gate | Result |
|:--|:--|
| OWNED | PASS (the report and the three `[internal]` rows noted in defect 11 aside) |
| U1 `test(/^u1_/)` | PASS (3 tests) |
| U2 `test(/^u2_/)` | PASS |
| U3 `test(/^u3_/)` | PASS (2 tests) |
| U4 quick wins, all names exist | PASS (11 tests run, no name missing) |
| U5 `test(/^u5_/)` | PASS (3 tests) |
| No pending list; no `CARGO_MANIFEST_DIR` in the binary's sources | PASS |
| G-verbs | PASS with the blank-line stop (defect 2); no `m3 scaffold` left |
| README lists every verb, no `import` | PASS |
| G-text | red at the base, not by this leaf (defect 3) |
| G-digest | PASS |
| `q1_` platform guard | PASS (inside the workspace run) |
| Three doc lines per `u1`..`u5` guard | PASS |
| STD, workspace forms | PASS: fmt, clippy `-D warnings`, nextest 599 passed, doc `-D warnings` |
| LINUX clippy | PASS |
| `cargo check --workspace --all-targets` | PASS |
| SUBJ: `cog check a72342b..HEAD`, subjects at most 72 | PASS |
| `contract_pages` (D1) | PASS, nothing blessed |

