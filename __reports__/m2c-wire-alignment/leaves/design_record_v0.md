# design_record: leaf report (v0)

Type: findings. Branch `m2c/design_record`, cut from `feat/m2c-wire-alignment` at 3307ca8. Docs only; no cargo, test or Grit run applies.

## 1. Commits

| sha | subject |
|---|---|
| 8f2f50e | docs(design): record the experiment's inputs, sources and provenance |
| a7bfa5a | reports(experiment): withdraw the planner hand-off to m4 |
| this commit | reports(design): record the design_record leaf |

## 2. Gates

| Gate | Result | Last relevant output |
|---|---|---|
| Pre: own checkout | PASS | `toplevel ok` |
| Pre: branches off the milestone after L0 | PASS in substance, FAIL literally | `merge-base --is-ancestor feat/m2c-wire-alignment HEAD` exits 1: the milestone tip is now 4ca847c (`rdm(experiment): close the m2c wire level…`, two README edits under `__roadmap__/`, no code). The L0 half holds (`merge(isolation): land agent_env` is on the first-parent line). See finding F1. |
| G6d provenance paths described | PASS | prints nothing. The paths the code writes: `provenance.eval_infra.{commit,commits_ahead,dirty,tag}`, `provenance.sandbox_contract`, `provenance.packages.`. All six are on this branch, because `eval_infra_origins`, `verdict_gating` and `agent_env` landed. No path comes from an unmerged leaf. |
| §Experiment file names each new field | PASS | prints nothing |
| §Run manifest names the new fields | PASS | prints nothing (`evaluation_inputs`, `` `from` ``, `bare sha256`, `commit`) |
| `request.treatment_source` gone from R01 | PASS | `! grep -nF` exits 0 |
| Only the owned sections moved | PASS | seven hunks, read against `grep -n '^##'`: 207 (§Experiment file, line 204), 448 (§Agent environment and secrets, line 417), 583 (§Plug-ins, line 572), 628 (§Evaluator protocol), 720 and 733 and 765 (§Run manifest) |
| Step 1 consistency check | PASS | grep for `request.treatment_source` empty, `evaluation_inputs` found |
| Step 2 consistency check | PASS | `1 file changed` against the base, one line |
| SUBJ | PASS | `cog check 3307ca8..HEAD` exits 0, and no subject exceeds 72 characters |
| [nl] final `spec-verifier` run | NOT RUN | belongs to the milestone's close-out |

The diff gate was read against 3307ca8, the branch point. The tip's one extra commit touches no file I own.

## 3. Guards, probes

None: no code, no test, no probe.

## 4. What R01 now says, and where each statement was checked

Statements checked against code on this branch (`git grep` on `crates/*/src`):

| R01 statement | Code checked |
|---|---|
| `TABLE_VALUED_PATHS` is `["prompt.values"]`; folding in `fixed` and in levels; `Duplicate`, `PathOverlap`, `EmptyTable`; the JSON edge | `candler-schemas/src/execution.rs` (`TABLE_VALUED_PATHS`, `fold_tables`, `leaves_of`, `flatten_into`) |
| `ExperimentInput {kind, path, at?, audience}`, `build_inputs` reserved, `at` absolute or with `..` refused, audience `agent` default | `execution.rs` (`ExperimentInput::new`, `InputAudience`) |
| `evaluation_inputs` absent when empty, sorted by kind and digest, a nuisance path | `RunManifest` serde attributes; `manifest/compare.rs::NUISANCE_PATHS` |
| `ValueSource::Derived.from` optional | `candler-schemas/src/values.rs` |
| `isolation.build.prune`: name pattern, env key rule; `isolation.record_packages`: name pattern; both validated in `resolve` | `isolation/image.rs` (`env_key`, `prune_name_ok`, `package_name_ok`), `resolve/steps/image.rs` (`prune_of`, `check_record_packages`) |
| The provenance table, gating, "refused by name, never equal" | `runner/evidence/provenance.rs`, `manifest/compare.rs` (`gates_verdict`, `provenance_refused`, `is_recorded`) |
| The four origins, their order, the vcs file winning over a surrounding checkout, `dirty` rules, unset packager names | `crates/candler/build_infra.rs` (`resolve`, `from_git`, `parse_vcs`, `from_package`, `from_packager`) |

Statements written from D2 and D4 and the README's shared constraints, **not** checked against code, because those leaves are in flight:

- `source` resolves against the experiment file, a tag without a source is refused naming the path, a moved tag is refused at resume naming `treatment.commit`, and a live reference always carries its commit (D4, guards T1 and T2).
- An agent input is mounted read-only, file by file, and enters `comparable.inputs[]`; an evaluators input stays out of the mounts, `tree.json` and `job.json`; the refusal list (missing path, symlink, empty directory, repeated or reserved kind) (D2, guards I1 to I3).
- The input digest layout (blob per file, list blob in RFC 8785 form, digest is the list blob's key) (shared constraints).
- `request.inputs` as a setting in the configuration table, and the run context serving `inputs[]` from `comparable.inputs` and `evaluation_inputs` (EV1).

`provenance.packages.<import>.{distribution,version}` appears literally, so the G6d grep for `provenance.packages.` finds it.

## 5. Findings

| # | Flag | Finding |
|---|---|---|
| F1 | request | Pre-condition 2 fails as written once the coordinator commits to the milestone branch after L0: `--is-ancestor feat/m2c-wire-alignment HEAD` is false at 4ca847c. The leaf file should test for the L0 merge subject and ancestry of 3307ca8 or of the merge, not of the moving tip. No action needed from this leaf. |
| F2 | stale doc, outside OWNED | `runner/evidence/provenance.rs`: the doc of `EvalInfra::commit` says "the abbreviated commit". The build records the full 40-hex commit, and R01 says so. Someone who owns that file should fix the line. |
| F3 | deviation (none) | No name differs from the shared constraints. Every name R01 uses (`TABLE_VALUED_PATHS`, `TreatmentSpec {name, tag?, commit?, source?}`, `evaluation_inputs`, `from`, the two settings, `provenance.packages.<import>.*`) is the shared one. |
| F4 | note | R01's Plug-ins paragraph carries D9's "tags unchanged" statement as a property ("each field is optional and absent when unset"), with no history. Eliott may prefer it in §Experiment file or §Run manifest. |
| F5 | for the fix round | The fix round that checks D2 and D4 against landed code should re-read the §Experiment file bullets "Treatment" and "Inputs", the "Inputs" and "treatment reference" bullets of §Run manifest, and the `request.inputs` row of the configuration table. |
| F6 | note | The imported-run rows of §Golden corpus (outside OWNED) still say `provenance.*` leaves enter `comparable.values` for imports. That is true of the importer and untouched. |

OWNED check: `git diff --name-only 3307ca8..HEAD` lists `__design__/architecture.md`, the M2b KT and this report only.

## Fix round 1

Base cc5e497 (all L1 code leaves landed). The statements written from D2 and D4 are now checked against code, and corrected to the rulings.

| Statement | Change | Code checked |
|---|---|---|
| Treatment source | A file's `source` resolves against the file, a `--set request.treatment` one against the cwd. `sources[].from` is the absolute, lexically normalised path (no `.`, `..` applied to the text, no trailing slash, no symlink followed), not the text as written. A treatment with no source is refused naming `treatment.source`. A moved tag is refused at resume naming `treatment` with both commits (the ruling; I did not find the resume check itself in the code I read). The retired setting is refused by `--set`, by name, and is no longer in R01 as a name. | `resolve/layers.rs` (`treatment_source_under`, `lexical`), `resolve/steps/identity.rs` (`TreatmentIdentityStep`), `commands/run.rs` (`RETIRED_TREATMENT_SOURCE`) |
| Commit always present | Narrowed. A live treatment carries a commit when it names a tag or a commit. A bare name with a source resolves nothing and records no commit. The earlier "always carries its commit" was wrong. | `identity.rs`: `revision` is `None` for `(None, None)` |
| Inputs, refusal list | Replaced: empty, absolute or `..` path; symlink at any depth; special file; non-UTF-8 name; comma, newline, CR or NUL in a name or `at`; colliding mounts; duplicate kind; `build_inputs`; empty directory. Empty files are valid. Refused before any job directory exists. | `experiment/inputs.rs` (`walk`, `walk_dir`, `resolve_inputs`, `UNMOUNTABLE`). The NUL and the "before any job directory" parts are from the ruling; the code I read names comma and line break in its message. |
| Mounts | Each agent file is mounted read-only at `at/<path>` from its blob. | `experiment/inputs.rs` module doc |
| Stubs and leaks | Mount-point stubs are not outputs; an empty-content match is no leak. | `runner/evidence/leak.rs` (`EMPTY`) for the leak; the stub rule is from the ruling |
| I8 line, §Run manifest | `job.json` removed from the absent list. The agent's container sees the argv, mounts, workspace, `tree.json` and `comparable`; the job record may hold `{kind, digest}`; the job directory is never mounted. | `jobs/model.rs` (`JobSpec.evaluation_inputs`), `session.rs` |
| Run context and `needs` | A need for K is served `{kind, path, digest}` from either list, `path` a read-only tree (directories writable). `build_inputs` refused by name at suite check; any other unmet need refused per run, by name. | `evaluation/registry/needs.rs` (`check`), `evaluation/context/assemble.rs` |
| E4 origin 3 | The packager's variables are inputs to the build; only what it proves is recorded, parsed strictly: 40 lowercase hex commit, `dirty` `true` or `false`, else unrecorded and refused by the verdict. `CANDLER_BUILT_*` is not named. | `crates/candler/build_infra.rs` (`from_packager`, `is_full_commit`, `parse_flag`) |

Gates on the new base: G6d, both section greps and the `treatment_source` grep print nothing; the seven hunks sit in §Experiment file, §Agent environment and secrets, §Plug-ins, §Evaluator protocol and §Run manifest (headings at 204, 425, 580, 617, 731); SUBJ over cc5e497..HEAD is recorded in the final message.

### New finding

| # | Flag | Finding |
|---|---|---|
| F7 | outside OWNED | The doc of `TreatmentSpec.source` in `crates/candler-schemas/src/execution.rs` says "as the experiment file wrote it". The recorded `from` is the absolute normalised path, and a `--set` source resolves against the cwd. The doc should follow R01. Not touched. |
| F8 | note | The corrections rest partly on rulings I could not find in code: the resume refusal naming `treatment`, the NUL refusal and the stub rule. The next verifier should check them. |
