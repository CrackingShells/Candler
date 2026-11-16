# plugin_environment leaf report (findings)

Base db16953. Steps 1 and 2 are done. Step 3 (the MCP real-image guard) is stopped on a spec defect (F1).

## 1. Commits
| sha | subject |
|---|---|
| 9ebe74d | feat(isolation): provision a plug-in's agent environment as a layer |
| 56fc66e | feat(isolation): append a plug-in environment's lists and inputs |
| (this) | reports(isolation): record the plugin_environment leaf |

No step-3 commit.

## 2. Gates
| gate | result | last line |
|---|---|---|
| worktree is its own checkout | PASS | workspace_root is the worktree |
| `guard_plugin_environment::` (9) + `guard_settings_out_of_cell::` | PASS | 21 passed with `guard_declared_inputs` |
| STD, `-p candler-core -p candler` | PASS | fmt ok; clippy -D warnings; doc -D warnings; 419 passed, 24 skipped |
| LINUX | PASS | Finished |
| SPAWN | PASS | 9 passed |
| DIGEST | PASS | numstat of vectors zero; `guard_cell_digest` 9; core set 45 |
| GOLDEN | PASS | 2 passed; 1 passed |
| E2E (full, Docker) | PASS | 14 passed |
| G7 | PASS | 38 matches at 18142ec and at HEAD |
| G8 | PASS | no added `tools/list` or `jsonrpc` |
| SUBJ | PASS | No errored commits; no subject over 72 |
| gate `plugin_environment_serves_live_mcp_tools` | BLOCKED | F1 |
| OWNED | PASS with additions | F5 |
| Grit reviewer (Sonnet) | done | no violation; notes 10, 19-21 below |

## 3. Guards (all in `crates/candler-core/tests/it/guard_plugin_environment.rs`; each has Guards and Oracle in its doc)
| guard | Guards | bite and failing message |
|---|---|---|
| `equivalent_values_and_lock` | selecting `p/web` gives the cell, env lock, lock sources and prune of the hand-written experiment | paths joined to the cwd: `the requirement source .../crates/candler-core/req.txt cannot be read` |
| `equivalent_appended_requirements` | plug-in sources first, then the experiment's with a `treatment:` entry, repeats once; same lock as by hand | plug-in list replaced: left `["extra.txt", "SKILL.md", "req.txt"]` (wrong order and set) |
| `layer_precedence_records_both_sources` | file scalar wins, source ExperimentFile; plug-in-only value is Derived from `p/web`; report holds origins; no `request.inputs` | derived ranked above the file: left `Some(String("plug-cmd"))` |
| `layer_unknown_plugin_and_environment` | unknown plug-in names it; unknown environment is `isolation.plugin_environment_missing` | refusal routed as failed step: left `"resolve.preflight_failed"` |
| `layer_paths_stay_in_the_plugin_directory` | absolute, `..`, symlink out (value and input) refused | symlink check dropped: `link: Fragment {..}` loaded |
| `layer_lists_append` | prune and egress append and dedupe; cell egress is the canonical union; invalid and bridge refused per cell | union dropped: left `Some(Array [a.com:443, c.com:443])` |
| `layer_egress_merge_reads_the_plugin_report_only` | only the plug-in's passing report is read; a stored (serde round-tripped) resolution plans the same digest | any report read: `no passing plug-in report`, left `other.com:443` |
| `layer_pair_from_two_layers` | half pair, and a pair split between plug-in and experiment, refused; whole pairs resolve | check dropped: `split: resolved` |
| `layer_inputs_mount` | inputs resolve against the plug-in directory into the hand-written mounts; collision and declared-twice refused | base dir of the first group used: `Refused { kind: "tools", problem: "does not exist" }` |

Bites were injected from copies and restored from them. Red was seen on the finished code (the guards could not compile before the types existed).

## 4. Claims checked
| claim | code showed |
|---|---|
| `Resolution.preflight` holds only passing reports, losers included | true (`preflight.rs`: `passed` only; a loser's report is passing) |
| `DerivedTwice`, `is_setting`, `SettingAsFactor` | as described; the step derives no `request.inputs` |
| step order | `register_m2b_steps`: treatment, plugin_environment, image, harness, catalog, session, egress |
| `preflight` is not persisted | FALSE: `JobSpec.resolution` serialises the reports, and the supervisor plans from it; guarded by a serde round trip. A stored cell replays from its `Comparable` anyway |
| "the existing not-found code" for an unknown plug-in | no such code exists (F2) |
| the catalog can capture the server | FALSE (F1) |

## 5. Findings
- **F1 (spec defect, step 3 stopped).** `CommandCatalogProvider` runs its fixed command in a control container built by `catalog_spec` from the image alone: no mounts, no network, no inputs. The fixture server is an agent input, mounted at run time, so the evaluator-side catalog cannot list it; the CLI's catalog command prints `servers: []`. Capturing it needs a mount parameter on `catalog_spec` (`container/run.rs`, `container_create`'s file) and a provider that knows the inputs. Needs a ruling: drop the catalog assertion, or add that seam. No fixture or e2e was built.
- **F2 (name).** No plug-in-not-found code exists. An unknown plug-in fails the step (`resolve.preflight_step_failed`, listed under `resolve.preflight_failed`).
- **F3 (stricter than the text).** A Dockerfile/context pair split between plug-in and experiment is refused (the merged result alone would accept it). `--set` plus file halves are not touched.
- **F4 (behaviour).** With a plug-in selected, two requirement sources with one file name are refused (the lock compiler stages by file name and would overwrite). Without a plug-in nothing changes (Grit note 21).
- **F5 (outside OWNED, each needed).** `resolve/preflight.rs` and `resolve/refusal.rs` (a `ResolveError::PluginEnvironment` variant, same route as egress), `runner/plan.rs` as named, `guard_refusal_codes.rs` (a case row), `docs/contract/{codes.toml,index.md}` (code appended, FragmentError as an `[internal]` row, index regenerated), `main.rs` mod line.
- **F6.** Fragment paths are absolute before the image step, so "source base directories" needed no struct: a plug-in source reads from the plug-in directory because the step joined it. A failed load in the image step is tolerated (the plug-in step refuses it with its code), so a doomed resolution can still build the experiment's image. A plug-in `lock` beats the experiment's `requirements` (scalar rule), silently.
- **F7 (attack list).** Symlink and `..`: refused at the reader (value and input paths). Two plug-ins sharing a name: `Registry::load` refuses (`plugin.name_repeated`). Absolute plug-in path in a digest: locations are `isolation.build.*` settings, which never reach a cell; the `inputs` root is content digests. Fragment path outside the allowed set: the manifest reader refuses it (wire_shapes). Case-insensitivity on macOS: containment compares canonical paths; no row spells a path in another case. Egress union, passing reports only, replan stability, merged digest: guarded above. A verifier is not run here.
- **Grit review notes left:** `selected_fragment` and `fragment_of` drop the load error on purpose; three loads per run re-read the manifest.

## 6. Docker
Before: 16 images, 3 containers, 5 networks. After the E2E run: containers and networks unchanged; one image id differs each way (not mine: my tests build none). Pulled: none. New e2e tests: none (step 3 not built).

Target dir removed.
