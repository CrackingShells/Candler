# agent_env leaf: findings

The environment and the recorded packages come from the experiment (`isolation.build.requirements`, `isolation.build.prune`, `isolation.record_packages`). Outside `manifest/compare.rs`, nothing in `crates/*/src` or `assets/` names a planner, `gtpyhop` or an HTN package. All gates below pass. One finding (F1) concerns the shared target dir and needs the coordinator.

## P3

**Question.** Do the six HTN requirements, compiled from an experiment's requirements file instead of from `stable.in`, give the same lock bytes? That decides whether live HTN cell digests move.

**Method.** The `COMPILE_SCRIPT` invocation of `isolation/image.rs` (`uv pip compile --python-version 3.12 --python-platform "$(uname -m)-unknown-linux-gnu" --no-header`), run in a control container of the pinned base (`python:3.12-slim-bookworm@sha256:935c3c5b...` plus `uv:0.9@sha256:538e0b39...`, the same layers as `assets/docker/base`), network `bridge`, `UV_NO_CACHE=1`. Two source files with the same six lines (`stable.in` as it is at b017d31, and a copy named `requirements.txt`), compiled in the same hour. The scripts live outside the repository and are not committed.

**Result.**

| | `stable.in` | `requirements.txt` |
|:--|:--|:--|
| lines | 121 | 121 |
| resolved packages | 40 | 40 |
| sha256 of the bytes | 23968d10... | ffcbefc7... |
| sha256 with `#` lines removed | f8445e0a... | f8445e0a... |

The pins are identical. The six differing lines are all `# via -r lock-src/<file name>` annotations, which name the source file. Nothing else differs.

**Decision.** The lock digest is the sha256 of the bytes, and it feeds `isolation.image`, hence the cell digest. So an HTN experiment that declares the six lines in `isolation.build.requirements` gets a different lock digest from the b017d31 one, whatever name it gives the file, and every live HTN cell digest moves. Two causes add up:
- the annotations name the new source file;
- the compiler resolves the newest versions at compile time (`gtpyhop-core==2.0.2`, `mcp==2.3.0` today), so a lock compiled later differs from a recorded one whatever its file names are.

Golden-import and parity-fixture digests do not depend on a compile and are unaffected (G5c). The smoke report lists every live HTN cell digest that moved, and attributes the move to the lock bytes, not to the key's definition.

## Commits

| sha | subject |
|:--|:--|
| baa6ae2 | reports(isolation): record whether the htn lock moves without stable.in |
| eae02e1 | feat(isolation)!: take the agent environment from the experiment |
| df9d1fc | feat(isolation)!: record the packages an experiment names, not a planner |

The report commit follows these (`reports(isolation): record the agent_env leaf`). Fix round 1 is folded into df9d1fc (fixup, autosquashed onto 2343ecd); `cargo check --all-targets -p candler-core -p candler` passes on each of the four commits.

## Gates

Run in a private target dir (see F1). Base for SUBJ is 2343ecd.

| gate | result | last line |
|:--|:--|:--|
| planner gone (both greps), assets clean, no `skill_version`, `stable.in` absent | PASS | greps print nothing |
| A1 `guard_image & test(/prune/)` | PASS | `a1_prune_is_part_of_the_env_key` |
| A2 `guard_run_evidence & test(/record_packages/)` | PASS | `a2_record_packages_are_measured_never_comparable` |
| `guard_resolve_surface` | PASS | `b19_every_path_class`, tuple (17, 19, 5), two rows added, none removed |
| remaining guards (`guard_layering_i1`, `guard_job_runtime`, `guard_job_orchestrate`, `guard_run_exec`, `guard_cli_kill_live`) | PASS | 21 + 3 tests passed |
| G5c (`guard_cell_digest` 9, import/legacy/oracle/context/engine/seams 33) | PASS | 33 tests run: 33 passed |
| G5a (no vector edited) | PASS | awk exit 0 |
| STD, `-p candler-core -p candler` (fmt, clippy `-D warnings`, nextest, doc `-D warnings`) | PASS | 158 tests run: 158 passed, 17 skipped (after fix round 1) |
| LINUX (`x86_64-unknown-linux-gnu`, `-p` form) | PASS | clippy finished |
| SPAWN (`guard_signal` after `cargo clean -p candler-core`) | PASS | 9 passed |
| SUBJ (`cog check`, lengths) | PASS | No errored commits |
| OWNED (`git diff --name-only`) | PASS | only the leaf's paths and the report |
| Grit reviewer | PASS | two BORROW comments, applied; no open finding |

Not run: GOLDEN (not named by the leaf's gates; G5c covers the cell digests), the workspace form of STD (the coordinator's), DRIFT (no schemas change).

## Guards and bites

| id | Guards | Oracle | Bite and the failing message seen |
|:--|:--|:--|:--|
| A1 `a1_prune_is_part_of_the_env_key` | one lock with two prune lists gives two env images, one list in any order gives one, none gives the lock's own tag | the three keys are the sha256 of the lock hex, then NUL and each sorted name, computed with `shasum`; the PATH-fake docker's log counts builds per tag | env tag keyed on the lock alone: `one list, one image` left 0, right 1. Seen red first (before the key existed) and again on the final code |
| A2 `a2_record_packages_are_measured_never_comparable` | the named packages are measured as `provenance.packages.x.{distribution,version}`, the unnamed `y` is not read or recorded, `comparable` holds no `provenance.*`, no setting gives no read and no entry | hand-written rows from the fake image's `x` (x-dist 1.2.3) and `y` (y-dist 4.5.6) | setting ignored in `orchestrate/mod.rs`: `the container read asked for the named package only` left [], right [["x"]] (seen red first); path written as `provenance.package.*`: left None, right Some("1.2.3") |
| A2 probe `a2_package_read_pairs_each_name_with_its_distributions` | a read gives a string pair for one distribution, lists for none or several, and refuses non-import names before a container runs | the container's printed JSON and the readings, both by hand | single distribution read as lists: left `(Array ["x-dist"], Array ["1.2.3"])`, right `(String "x-dist", String "1.2.3")` |
| F4 `image_step_refuses_malformed_names_before_docker` | the image preflight step refuses a malformed `isolation.build.prune` or `isolation.record_packages` before any docker call, naming the setting and the bad value | 15 hand-written rows from the two name rules (a `/`, `.`, `..`, empty, a space, an uppercase, a digit start, `-`, `.` in an import name, a non-string entry, a non-list value); the PATH-fake docker's log is empty on every row, and a positive control with well-formed lists reaches docker | accept every prune name (`prune_name_ok` true): `isolation.build.prune = ["ok.py","a/b.py"] was accepted`; accept every import name: `isolation.record_packages = ["x","Foo"] was accepted`; validate after the lock compile instead of before: `... reached docker before it was refused` |
| `b19_every_path_class` (existing, two rows added) | `isolation.build.prune` and `isolation.record_packages` resolve as settings and never reach a cell digest | `field_paths.toml` rows | `isolation.record_packages` dropped from `is_setting`: `isolation.record_packages resolves as a setting` |

The leaf asked for the bite "values written into `comparable`" on A2. I did not inject it into the job path: no seam writes readings into a cell. The plausible form of that bug is the setting being classed as a cell value, which `b19` turns red (last row). A2 asserts the same property on the sealed manifest.

## Decisions and deviations

- **Env key.** Nothing pruned keeps the key equal to the lock digest, so every existing `candler-env:<lock hex>` tag and the b05 guard stay as they are. A prune list gives the sha256 of the lock hex followed by NUL and each sorted, de-duplicated name. `candler.lock` stays the lock hex.
- **Empty lock.** A `LockRequest` without sources returns an empty lock and runs no container; `uv pip install -r` of an empty file succeeds (checked with local uv: warning, exit 0).
- **Where the settings are validated.** In the image preflight step, not in `layers.rs`: `b19` resolves each row with the value `"a"`, so a refusal at layer level would fail it. `prune_of` refuses a malformed prune list; `check_record_packages` refuses a malformed `isolation.record_packages` (in the same step, which already reads `isolation.build.*`). `ensure` and `PackageProbe::read` refuse bad names again before any docker call.
- **Dockerfile.** `ARG PRUNE` is declared after `FROM` (before it, it would be invisible to the stage); the build passes `PRUNE` only when the list is not empty.
- **Names.** `PackageProbe`, `PackageReading`, `PackageReadings` (map alias), `package_name_ok`, `prune_name_ok`, `parse_packages`, `JobHost::packages(rt, image, names)`, `CellRun::with_packages`, `record_packages(&Resolution)`. The shared constraints' names (`isolation.build.prune`, `isolation.record_packages`, `provenance.packages.<import>.{distribution,version}`) are used as given.
- **Empty reading.** A named package the image does not hold is recorded with empty lists, as the planner axis did for no distribution.
- **Edits beyond the letter of OWNED, all inside owned files:** `guard_layering_i1.rs` also loses its now-unused `scan` import and gains `crates/candler/src` in the folder list (the allowance hunk, nothing else); `support/job_orchestrate.rs` gains `FakeReads` and `Rig::host_reading`; `runner/exec/mod.rs` also loses a `comparable` binding that only the deleted call used.
- **`lock_digest` of the empty lock and live HTN digests.** See P3: live HTN cell digests move.

## Findings

- **F1 (needs the coordinator): the shared target dir gives false results across worktrees.** Cargo derives a path package's metadata hash from its path relative to the workspace root, so every worktree's `candler-core` writes to the same artifact names in `/private/tmp/claude-501/m2c-target`. A sibling's artifacts were reused for my sources: nextest listed one test instead of two, and a build failed with `BuildSetting has no field named prune` against a library that had it. A green run there may be a sibling's code. I switched to `/private/tmp/claude-501/m2c-target-agent_env` (4 to 5 GB; sccache keeps rebuilds short) for every gate in this report, and the guards of steps 2 and 3 were re-run there. I delete that dir at the end. The other leaves should do the same, or the coordinator's per-merge run should be the only trusted one.
- **F2 (spec note, no defect): `isolation.build.prune` needed no `layers.rs` change.** `is_setting` already matches every `isolation.build.*` path (`b19` confirms it); only `isolation.record_packages`, which is outside `isolation.build`, needed an arm.
- **F3 (for the design leaf): G6d.** The measured paths are built with `format!("provenance.packages.{name}.…")`, so the G6d grep sees the literal `"provenance.packages.` only. R01 §Run manifest must contain `provenance.packages.` for it to print nothing. The `provenance.planner.sandbox.*` literals of the legacy import (`guard_oracle_mini`, mini-corpus fixtures) are fixture data and untouched.
- **F4 (fixed in round 1):** `image_step_refuses_malformed_names_before_docker` in `guard_image` drives `ImageIdStep` through a `Resolver`, over the real `DockerImageService` and the PATH-fake docker. The filter prefix is `image_step_` (`binary(guard_image) & test(/image_step/)`). Three changes came with it:
  - the step now validates both settings before it compiles the lock (the lock compile is a docker call, and the table sets a requirement source so that an out-of-order check turns the guard red);
  - the messages name the setting and the bad value;
  - `.` and `..` are refused as prune names (they match the character class but are not file names).
  Duplicates are not refused: the key de-duplicates the prune list, and a duplicate import name only repeats a path.
- **F5 (not run here):** X1 (the real env image build in CI's `container-e2e`) and the Dockerfile's prune loop were not run; `env_image_e2e` owns them.
