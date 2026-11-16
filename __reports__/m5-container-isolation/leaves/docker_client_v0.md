# docker_client: leaf report (v0)

Type: findings. Branch `m5/docker_client`, cut from `feat/m5-container-isolation` at e7aa7bb.

## 1. Commits

| sha | subject |
|:--|:--|
| 15f2344 | feat(isolation): inspect many containers in one call and create by id |
| a7d7a63 | refactor(isolation): build every docker process in one client |
| 1c009be | test(isolation): build the e2e images through the core |

## 2. Gates

| gate | result | last relevant line |
|:--|:--|:--|
| worktree is its own checkout | PASS | `cargo metadata` prints the worktree path |
| one client (`SpawnRequest::builder`, `"docker"`) | PASS | both `test -z` forms hold: only `isolation/docker.rs` matches |
| agent argv unchanged | PASS | `git diff --quiet e7aa7bb -- harness_baseline`; `guard_harness_facts::` 9 passed |
| `control_kind_` / `client_env_` | PASS | 2 tests run: 2 passed |
| `inspect_many_maps_each_id` | PASS | 1 test run: 1 passed |
| real-image guard, Docker up | PASS | `control_each_kind_runs_on_the_real_base` passed in 0.9 s |
| E2E (full) | PASS | 14 tests run: 14 passed |
| SPAWN | PASS | 9 tests run: 9 passed |
| STD, `-p candler-core -p candler` | PASS | fmt, clippy `--all-targets -D warnings`, doc `-D warnings` clean; nextest 406 passed, 24 skipped |
| LINUX | PASS | `clippy --target x86_64-unknown-linux-gnu --workspace --tests` clean |
| DIGEST | PASS | vectors unchanged; `guard_cell_digest` 9 passed; the six oracle/import guards 45 passed |
| SUBJ | PASS | `cog check e7aa7bb..HEAD`: no errored commits; all subjects at most 72 characters |
| OWNED | PASS with notes | see finding 2 for the paths outside the OWNED line |
| DRIFT, GOLDEN | not run | no schemas change; no import, resolve or run-path change of data (the gateway and probe argv only) |
| Sonnet review | closed | finding 6 |

## 3. Guards

| guard | `Guards:` line | oracle | bite and the failing message seen |
|:--|:--|:--|:--|
| `guard_container_guard::inspect_many_maps_each_id` | one docker call reads many containers; the missing id alone is `NotFound`, the others keep their facts in the order asked, a short id never reaches docker, and the recording fake answers the same way in one recorded call | the output of `docker container inspect --format '{{json .}}' A MISSING B C` on Engine 29.9.2 (exit 1, three JSON lines on stdout, one `No such container` line on stderr), recorded in `tests/fixtures/docker_inspect_many`; the spawn log of a fake binary | (a) the missing id mapped to `Failed`: `the missing id alone is NotFound: Err(Failed { detail: "Error response from daemon: No such container: ff..." })`. (b) one `inspect` per id instead of one spawn: panicked at `.expect("a found container")` (the fake prints the same three-container output for every spawn, so the per-id mapping reads the wrong answers). Both red, both restored from a copy. |
| `guard_container_run::control_kind_flags_and_limits` | every control container removes itself, wears `candler.role=control` and no job label, has a read-only root, no capabilities and no new privileges, and keeps its own network and limits | the flags of each kind's argv at 18142ec, typed by hand (probe and package probe 64 pids, 256m, `/tmp` 64m, no cpu cap; compile 256 pids, 2g, bridge, bare `/tmp`; catalog and collect the caller's limits with a cpu cap and `/tmp` 256m; fake model no `--network`, `-d`, `/fake` tmpfs only) | `--cap-drop` dropped: `version probe: capabilities`, left `[]` right `["ALL"]`. Limits unified upward (pids 512): `version probe: pids`, left `["512"]` right `["64"]`. `--rm` dropped: `version probe: it removes itself`, left `["run", "--init"]`. |
| `guard_container_run::client_env_is_the_allowlist_on_every_spawn` | whatever the caller's environment holds, a docker child sees only `DOCKER_HOST`, `DOCKER_CONTEXT`, `HOME` (and the supervisor's `CANDLER_RUN`) on a read, a control container and a gateway-style call; the agent's attached client sees only those of the process | the allowlist written in the test, and the environment a fake docker binary printed with `/usr/bin/env` for a client given `PATH`, a key and a secret | `PATH` added to the allowlist: `capture: the child's environment names`, left `[..., "HOME", "PATH"]`. |
| `guard_container_e2e::control_e2e::control_each_kind_runs_on_the_real_base` | the version probe, package probe, catalog capture and collect run on the real base built through `DockerImageService::base`, each through its `ControlSpec`; the probe's home is writable | the image's own `candler.cc_version` label, the distribution `pip` of the image's python, and the strings the commands print and write | `probe_env` without `HOME`: `a probe with a writable home exits 0: Failed { detail: "the control container exited with status 1: " }`. A service that passes no probe environment at all is not caught here: the real `claude --version` runs without `HOME` (see findings). |

Existing guards that now hold the byte-equality claim: `guard_harness_facts::` (all pass; `harness_baseline/` untouched, the probe argv compared whole).

## 4. Claims checked

| claim | what the code showed |
|:--|:--|
| spawn sites: six in the plan | At e7aa7bb: `isolation/docker.rs` (the client), `gateway.rs` (own `DockerCli`, bare `docker`), `container/run.rs` (agent request, own binary lookup and env list), `image.rs` twice (`compile`, `control_output`), `catalog.rs`. `controls.rs` and `exec.rs` build argv only and run through `DockerDriver::capture`. Seven spawn paths, not six: the probe and the compile were two sites in `image.rs`. All now go through `docker.rs`. |
| five hardening lists | Five at e7aa7bb (agent `hardening()`, gateway `create_args`, `compile_argv`, `control_argv_with`, `fake_args`), plus the collect and the catalog which reused the agent's. Limits differ per kind and are kept: the probe, package probe and fake model have no `--cpus` and `/tmp` is 64m, bare, or absent. |
| gateway `PATH` | See finding 1. |
| gateway `--cidfile` unused | True: the file was written by docker and never read (`dir.rs` skips `.cid` files); the id comes from stdout. Removed. The agent's `--cidfile` is read (`collect_and_destroy`) and stays for `container_create`. |
| six control kinds, all `--network none` except compile | Four are `none`, compile is `bridge`, and the fake model names no network (a `--network none` container cannot be joined to the egress network afterwards, and the flag was never there). `ControlNetwork::Default` carries it. |
| the probe's argv is the baseline | `--network none` sits between `--init` and `--read-only` in `argv.json`, so `hardening()` starts with `--init` and `ControlSpec` places the network right after it. |
| `DockerImageService::{ensure, base}` signatures | Unchanged; `DockerImageService::new`, `CommandCatalogProvider::new` and `PackageProbe::new` also keep theirs (the binary crate's `preflight.rs` calls them). |
| exit mappings | `image.rs` had two hand-written mappings (probe, compile) with different wording; `control::outcome` is the one. `catalog::interpret` keeps its own, because `CatalogError::{Timeout, Command{status}}` is a recorded error taxonomy with its own guards and a deadline end that maps to `Timeout`. |

## 5. Findings

1. **The gateway's `PATH` (decision for R01): removed.** The gateway's client ran the bare program name `docker` under a cleared environment (`env_clear`), so the child had to find `docker` through the `PATH` in its own environment: that was the one use. No credential helper is involved: the sidecar is created from a local image id, so `create`, `network connect`, `start`, `exec -i` and `logs` never pull, build or push. The one client runs the absolute path it found at start, with `DOCKER_HOST`, `DOCKER_CONTEXT` and `HOME`, like every other docker call; the gateway e2e (b17, b18, b37), the supervision gates and the controls e2e pass without it. R01 note: a docker context that needs a helper on `PATH` (an `ssh://` host) fails for every client alike, not for the gateway alone.
2. **Edits outside OWNED, each needed to compile or to keep a caller on the one client:** `jobs/orchestrate/session.rs` (builds the gateway's client from `DockerCli::new` and passes the marker; it called the deleted `gateway::DockerCli`), `isolation/container/mod.rs` (one line: `collect_spec(..).args()`), `isolation/mod.rs` (two `mod` lines), the `mod` line in `tests/it/guard_container_e2e.rs`, and the existing e2e files edited in step 3. The coordinator may veto; the first is the only one a later leaf (`plugin_environment`, which also touches the gateway start) could conflict with.
3. **Public signature changes:** `DockerGateway::new` takes a `docker::DockerCli` and a `Marker` instead of the gateway's own `DockerCli`; `gateway::DockerCli` is gone (so `GatewayLogs` lost `Debug`); `run::control_argv` and `exec::collect_args` became `catalog_spec` and `collect_spec` (returning `ControlSpec`); `image::{probe_spec, timeout_spec, compile_spec, probe_env}`, `controls::fake_spec` and `docker::{Call, AttachedRun, attached_request, PROGRAM}` are new public items (the guards in the integration crate build the specs).
4. **Behaviour changes, none recorded or digested:**
   - flag order of the collect, catalog, compile and fake argv (the agent's and the probe's are byte-identical); `--env` is `-e`;
   - the gateway's `docker create` now runs through `DockerDriver::create`: marker of an ordinary control call instead of `gateway`, deadline 60 s instead of 90 s, and no cidfile, so a client killed between the create and its answer leaves a sidecar the job-label sweep finds, not a `.cid` file nobody read;
   - control-container error texts are reworded (`the probe: ...`, `the compile: ...`), and a probe failure now carries the tail of the container's stderr;
   - the agent's client lookup requires an absolute `PATH` entry, and a non-UTF-8 `HOME` is dropped rather than converted lossily.
5. **Spec observations:** `inspect_many` has no production caller yet (A1 builds on it); `Role::Egress` has none until `egress_proxy`. The guard names the version-probe row's bite as "a builder without `HOME`", but the real Claude probe runs without `HOME` on this base (its image sets `HOME=/home/agent`), so only the direct `probe_env` row turns red; a service that ignored `probe_env` is caught by `guard_harness_facts` (whole argv), not by the e2e.
6. **Review (Sonnet, `rust-conventions-reviewer`):** the byte-equality, control-kind table and `PATH` attacks found no defect. Fixed: error wording (finding 4), the `collect_args` wrapper, a stale `# Errors` line. Left, reported here: the gateway create's marker and deadline (4), duplicated limits struct literals in `timeout_spec` and `fake_spec`, `PROBE_TMPFS` re-spelling `tmp_tmpfs(64)`, and `catalog.rs` keeping its own timeout exit code with `interpret`.
7. **Request:** CI's `.github/workflows/container-e2e.yml` step "Build the testagent image" (`docker build --label candler.test=1 -t candler-testagent:test assets/docker/testagent`) must go; nothing reads that tag any more. Add `control_each_kind_runs_on_the_real_base` to the expected-guard list (the list names every e2e by its last path segment).
8. **How `test_image` treats an image that already exists:** the tag is `candler-test-<context dir>:<12 digits of the context digest>` (or a tag the caller names), and the image carries the label `candler.test_image=<those digits>`. If docker holds the tag the image is used as it is (`Built::Found`) and never removed by that test; otherwise it is built through `DockerDriver::build` (`Built::Made`) and removed by id when the test's `TestImage` drops. A context that changed gets a new tag, so an old image is never taken for it. `candler-testagent:test` (8e64ae823a0a) and `candler-gateway:test` (a7b4108cc6f4) are no longer read by any test and were not touched. The controls e2e still needs the production tag `candler-gateway:latest`, so it passes that tag to `TestImage::tagged`.

## 6. Docker

- Before (start of work): 15 images, 3 containers, 5 networks. After the full E2E run: the same containers, and no image labelled `candler.test_image` remains. One image and one network differ from the snapshots, and neither is mine (made by other sessions on the shared daemon; mine are labelled and gone).
- Measured once: `docker container inspect` with one missing id exits 1, prints the found containers on stdout in the order asked and names the missing id on stderr (`No such container: <id>`); recorded under `crates/candler-core/tests/fixtures/docker_inspect_many`. Three throwaway containers made for it were removed by id.
- The e2e tests (14, all pass): `guard_container_e2e::container_run_e2e::b06_kernel_facts`, `::b08_self_expiry`; `::container_stop_e2e::b12_sweep_scope`, `::b13_client_killed_container_gone`; `::control_e2e::control_each_kind_runs_on_the_real_base` (new); `::controls_e2e_runs::controls_e2e_pinned_harness_passes`; `::env_image_e2e::x1_env_image_prunes_the_named_file_and_reads_the_pin`, `::x1_env_image_without_requirements_has_only_seed_packages`; `::gateway_e2e::b17_canary_absent`, `::b18_internal_network_no_route`, `::b37_untrusted_upstream_gets_no_credential`; `::supervision_gates_e2e::b36_supervisor_sigkill_nothing_left`; `::version_probe_e2e::opencode_probe_reads_each_real_base_image_version`; `guard_cli_e2e::kill_e2e::b40_kill_all_destroys_lost_jobs_containers`.
- Pulled: none (the pinned python image was already present).
