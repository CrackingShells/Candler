# env_image_e2e leaf: findings

A real env image, built through the core's image service over the real docker CLI, shows the three things the leaf asks for. The pruned file is absent, the pinned package is read back with its distribution and version, and an experiment with no requirements gets only the seed packages. Both guards pass locally and are red under their bites. One spec defect (F1, the commit subject is 73 characters) is resolved by a 68-character subject. Two CI notes (F2, F3) need the coordinator.

## Commits

| sha | subject |
|:--|:--|
| 2824cfb | test(isolation): build a real env image from experiment requirements |

The report commit follows (`reports(isolation): record the env_image_e2e leaf`).

## Gates

| gate | result | last line |
|:--|:--|:--|
| cargo metadata workspace root | PASS | prints the worktree |
| X1 `binary(guard_container_e2e) & test(/env_image/)`, `--run-ignored only --no-tests=fail` | PASS | 2 tests run: 2 passed, 9 skipped |
| e2e removes what it built | PASS | `docker images -q` and `docker ps -aq` equal the files recorded before the first run, after every run (clean, bitten, clean); `docker images -q --filter label=candler.lock` prints nothing |
| CI covers it | PASS, with F2, F3 | the filter `binary(guard_container_e2e) \| binary(guard_cli_e2e)` selects the module, which `#[path]` puts in that binary |
| STD, `-p candler-core` (fmt, clippy `-D warnings`, nextest, doc `-D warnings`) | PASS | 164 tests run: 164 passed, 14 skipped |
| LINUX (`x86_64-unknown-linux-gnu`, `-p candler-core --tests`) | PASS | clippy finished |
| SPAWN (`guard_signal` after `cargo clean -p candler-core`) | PASS | 9 passed |
| SUBJ (`cog check 8f3a103..HEAD`, subject lengths) | PASS | No errored commits (after F1) |
| OWNED (`git diff --name-only 8f3a103..HEAD`) | PASS | the test module, the fixture, `guard_container_e2e.rs`, then the report |

Not run: DRIFT (no schemas change), G5a (no vector), the workspace form of STD (the coordinator's), the Grit reviewer agent (checked by hand against the trigger checklist: BORROW comments on conversions, no `as`, no indexing).

## Guards and bites

| id | Guards | Oracle | Bite and the failing message seen |
|:--|:--|:--|:--|
| X1 `x1_env_image_prunes_the_named_file_and_reads_the_pin` | an experiment's requirements file and prune list build an env image in which the pruned file is absent and the pinned package is read back | the pin `colorama==0.4.6` in `fixtures/env_image/requirements.txt`; the wheel's own `RECORD` in the container (it lists `colorama/tests/isatty_test.py`, so the file was shipped, which stops the guard from passing vacuously); a sibling file (`ansi_test.py`) that must remain; `uv pip list --format json` in the container | the copy of the Dockerfile in the test's scratch dir loses its prune loop's list (`for name in ${PRUNE};` becomes `for name in ;`): `isatty_test.py is still under /opt/env`, left `/opt/env/lib/python3.12/site-packages/colorama/tests/isatty_test.py`, right empty |
| X1 `x1_env_image_without_requirements_has_only_seed_packages` | an experiment with no requirements gets an env image with no third-party package | `uv pip list` in the container against `pip`, `setuptools`, `wheel` (the packages `uv venv --seed` installs per uv's documentation); the read of `colorama` is empty lists | the lock carries a package the experiment did not name (the test given the fixture requirements): `third-party packages present: ["colorama"]` |

Both bites were applied to the test file after the commit and restored with `git checkout --`; the repository's `assets/docker/env/Dockerfile` was never edited, because the service builds from a copy that the test makes. The guards were not seen red before the code in the usual sense: the code under test already exists, so the bite is their red. The first run was green on the first try, then both bites turned them red.

The fixture package is `colorama` 0.4.6: one `py2.py3-none-any` wheel, no `requires_dist`, and a file name (`isatty_test.py`) that no seed package ships. `tomli` was rejected (PyPI also lists mypyc cp312 wheels, so it is not pure) and so was `packaging` (pip vendors a `_musllinux.py` of the same name).

## Docker footprint

- Before the first run: `docker images -q` (11 ids) and `docker ps -aq` (3 ids) recorded to `$TMPDIR/env_image_e2e-{images,ps}-before.txt`. After every run (green, two bitten, green) both lists were equal to them.
- The base image `candler-base:src-<digest>` was absent before, so the test created it and removes it; the env images carry `candler.lock=<hex>`. The test removes by exact id: the ids of images with its lock label and of the base tag that were not in the "before" snapshot, newest first, and each container it makes with `docker create` by the id `create` printed. A base image that was already there is never removed.
- Ids of the final green run, all removed: `sha256:71c0b678...80b0f8d` and `sha256:bec13f60...a29bf27` (first test), `sha256:33148a01...5af5f1`, `sha256:d0735a93...7fe80e` (second test). The test prints its ids on stderr (`env_image_e2e created and removed the images [...]`). The ids of the earlier runs were not recorded; their cleanup is shown by the equal lists.
- The cleanup also runs on a panic (`Drop`): both bitten runs left nothing.
- The target dir `/private/tmp/claude-501/m2c-target-env_image_e2e` was deleted before this report.

## Findings

- **F1 (spec defect, resolved): the leaf's commit subject is 73 characters.** `test(isolation): build a real env image from an experiment's requirements` fails SUBJ (`awk 'length>72'` exits 1; `cog check` does not see it). I committed `test(isolation): build a real env image from experiment requirements` (68). The coordinator's merge needs no change.
- **F2 (CI, needs the coordinator): the job's expected-guard list does not name the new guards.** `container-e2e.yml` selects `binary(guard_container_e2e)`, so both tests run in the job, and a failure fails it. Its "Check every expected guard ran and passed" step lists ten names and not these two, so a rename or an `#[ignore]` that nothing runs would pass silently. Adding `x1_env_image_prunes_the_named_file_and_reads_the_pin` and `x1_env_image_without_requirements_has_only_seed_packages` to that list is a `.github/` edit, so it is not done here.
- **F3 (CI risk, needs the coordinator): the 180 s test cap on a cold runner.** `.config/nextest.toml` gives the group `slow-timeout = { period = "60s", terminate-after = 3 }`. Locally the first build, with the base image absent, took 28 s here (the runner has the Python and uv layers cached; the 290 MB Claude Code `ADD` was downloaded). A cold ubuntu runner pulls the Python image and uv and downloads Claude Code before the env build, plus the lock compile. If it nears 180 s the group's period needs raising, which is outside OWNED. Not measured on CI.
- **F4 (note): the image service builds the base image on a clean daemon.** `ensure` builds `candler-base` (a download of Claude Code from downloads.claude.ai) even for an env with no requirements, so the second guard needs the network too; its `#[ignore]` reason says Docker only. No defect.
- **F5 (note): the base image and the shared daemon.** The test removes the base image it built, so each run on a clean daemon downloads again unless BuildKit's cache holds the layers (it did here: 5 s for the later runs).
- No `src/` or Dockerfile defect found: the prune loop and the pinned `FROM` work on Docker Desktop (containerd store) on macOS arm64. Linux was not run here.
