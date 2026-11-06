# Image: pinned base and env layers

**Goal**: `isolation::image::DockerImageService` implements `ImageService`: it builds the digest-pinned `candler-base` once and `candler-env:<lock digest>` once per lock digest under a per-tag `flock`, returns an `ImageRef {id, cc_version}` pinned by local image ID, and reads the Claude Code version from the image through a labelled, self-removing control container.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas): ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on the scaffold's `ImageService`, `ImageRef`, `BuildRequest`, `DockerDriver::{build, image_inspect}` and `isolation/layout.rs` (the per-tag lock path)
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] `docker version --format '{{.Server.Version}}'` (Steps 1 and 3 build and run real images)
- [ ] [cmd] DEP(container_stop), needed from Step 4 only: `git log --first-parent --format=%s feat/m2b-execution-parity | grep -qx 'merge(.*): land container_stop'`. `isolation/docker.rs`, the real `DockerDriver` that B05 runs against the PATH-fake `docker`, is that leaf's. Steps 1 to 3 run against the recording `DockerDriver` fake from `test-support`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_image)'` passes, holding B05: two concurrent `ensure` calls for one lock digest make exactly one `build` line for `candler-env:<digest>` in the PATH-fake `docker` log (the oracle), the second call returning the image the first built; the fake answers `image inspect` from a marker file that its `build` writes and takes about a second to build, so the calls really overlap
- ⬜ [cmd] Every pin is by digest: `! grep -E '^(FROM|COPY --from)' assets/docker/base/Dockerfile | grep -v '@sha256:[0-9a-f]\{64\}'` and `grep -q '^ARG BASE$' assets/docker/env/Dockerfile`
- ⬜ [cmd] The built images satisfy the feature map's provisioning checks, run with `EXEC` set to `docker run --rm --init --network none --read-only --mount type=volume,dst=/opt/env --label candler.role=control --label candler.job=preflight <env image id>` (control containers are the one place `--rm` is used): `venv_contents.sh`, `venv_not_hardlinked.sh` and `venv_scoping.sh` from `__roadmap__/m2b-execution-parity/feature_map/checks/` each exit 0. The command and the three outputs go in the leaf report
- ⬜ [cmd] The env image's `RootFS.Layers` starts with the base's whole layer list, and `claude --version` through `DockerImageService::cc_version` equals the `candler.cc_version` label: both read by a class-A `#[ignore]` test against the real daemon, run once, output in the leaf report
- ⬜ [cmd] `cargo clippy --target x86_64-unknown-linux-gnu --workspace --tests -- -D warnings` passes (LINUX)
- ⬜ [cmd] STD and SUBJ pass
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/isolation/image.rs crates/candler-core/tests/guard_image.rs`
- ⬜ [cmd] No destroy verb and no prune in this leaf: `! git grep -n -E '"(rmi|prune|kill|stop)"|image rm' -- crates/candler-core/src/isolation/image.rs assets/docker/base assets/docker/env`
- ⬜ [nl] The leaf report records the bite mutation for B05 and the probe's amd64 build (emulated), and no image is removed by the leaf (M2b has no prune, R22)
- ⬜ [nl] No verifier is allocated to this leaf; every gate above is a `[cmd]` except the bite record
**References**: [R01 §Agent environment and secrets, §Run manifest](../../__design__/architecture.md) — `isolation.image` is a local image ID, never a tag; [R03 §2, §3 decision 2, §8, §10.2](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — the env layer, the control container (R11), no prune (R22); [R04 B05](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — the guard and its bite; [R05b image probe](../../__reports__/m2b-execution-parity/03-image_probe_v0.md) — the base (R3), the digests, the pinned-`FROM` mechanism, the writable paths, the two layering traps; [R06 feature map](feature_map/README.md) — `venv_*` checks, rows S3.R6 to S3.R9, S3.R12, X17, X18

## Step 1: Pin the base and env image sources
**Goal**: Two Dockerfiles build the same Claude Code and uv environment from digest-pinned inputs only.
**Implementation Logic**:
1. `assets/docker/base/Dockerfile` follows the probe's sketch: `FROM python:3.12-slim-bookworm@sha256:935c3c5b12357a7e10caa9c1e6ca5692344d97cf229832ef584be2cec9fc03df`; a `useradd -m -u 10001 -d /home/agent agent`; `COPY --from=ghcr.io/astral-sh/uv:0.9@sha256:538e0b39736e7feae937a65983e49d2ab75e1559d35041f9878b7b7e51de91e4 /uv /uvx /usr/local/bin/`; the `claude` 2.1.311 glibc binary for `TARGETARCH` (`arm64` sha256 `2e2496c9…ef61f3`, `amd64` sha256 `86dc1fcc…489ada`, full values in R05b) installed with `COPY --chmod=755` or `ADD --checksum=sha256:<hash> --chmod=755`, and the checksum verified in a `RUN` that bind-mounts `claude.sha256`; `LABEL candler.cc_version=2.1.311` from the same `ARG` that selects the binary; `USER agent`, `ENV HOME=/home/agent`, `WORKDIR /home/agent`. The R05b traps stay out: no `mv` or `chmod` after `COPY` (a second 290 MB layer), and a per-Dockerfile `.dockerignore` so only the right binary enters the context. musl and Alpine are out (R3): coreutils `timeout` exits 124.
2. `ADD --checksum=sha256:<hash>` brings the claude binary in (D20); no Rust code downloads and S1 adds no HTTP crate.
3. `assets/docker/env/Dockerfile` follows the probe's sketch: `ARG BASE` (never a default), `FROM ${BASE}`, `USER root`, `ENV UV_LINK_MODE=copy UV_PYTHON_DOWNLOADS=never UV_NO_CACHE=1 PYTHONDONTWRITEBYTECODE=1`, `uv venv /opt/env --seed --python 3.12`, one `uv pip install --python /opt/env/bin/python --no-build -r <lock>` (an sdist is a build failure, so nothing compiles), then `find /opt/env -type f ( -name domain.py -o -name problems.py ) -delete` (contract I8), `chmod -R a+rwX /opt/env` (the agent installs into its own copy under any `--user`; ruling A13), `USER agent`, `ENV VIRTUAL_ENV=/opt/env UV_PROJECT_ENVIRONMENT=/opt/env PATH=/opt/env/bin:$PATH`. The path is `layout::CONTAINER_ENV`. The lock is the resolved requirement set, a file in the build context (R05b: open ranges resolved to different versions, so the digest must hash the resolved set).
4. `claude -p` with a read-only HOME exits 0 and persists nothing (R05b). The images keep HOME writable only through the per-run bind mount `container_run` adds; nothing in the image writes under HOME.
**Deliverables**: `assets/docker/base/{Dockerfile,.dockerignore}`; `assets/docker/env/{Dockerfile,.dockerignore}`
**Consistency Checks**: `! grep -E '^(FROM|COPY --from)' assets/docker/base/Dockerfile | grep -v '@sha256:[0-9a-f]\{64\}' && grep -q '^ARG BASE$' assets/docker/env/Dockerfile` (expected: PASS)
**Commit**: `feat(isolation): pin the agent base and env image sources`

## Step 2: Build the base and the env layer once, pinned by image ID
**Goal**: `ensure` returns the same pinned image for a lock digest however many callers race, and a base that changed rebuilds the env tag.
**Implementation Logic**:
1. `DockerImageService::new(sup: Supervisor, docker: Arc<dyn DockerDriver>, assets: ImageAssets) -> DockerImageService`; `impl ImageService`: `ensure<'a>(&'a self, lock: &'a LockDigest, build: &'a BuildSetting) -> BoxFuture<'a, Result<ImageRef, ImageError>>`. A private `ensure_base(&self) -> Result<BaseImage {tag, id}, ImageError>` builds `assets/docker/base` as `candler-base:src-<sha256 of the base context, 12 hex>`, a lookup alias only. The identity is the image ID read back with `DockerDriver::image_inspect`.
2. Per tag, in this order: take an exclusive `rustix::fs::flock` on the tag's lock file (the path comes from `isolation/layout.rs`; run it in `spawn_blocking`, since two separate opens of one file conflict even inside one process), then ask `image_inspect(tag)`, then build if absent, then release. The check sits after the lock; taking the lock after the check is B05's bite. Applies to both the base tag and `candler-env:<lock digest>`.
3. The env build passes `BASE=candler-base:src-<id12>@sha256:<full base image id>` as a build arg (R05b: `tag@digest` pins, the digest wins, a wrong digest fails closed). `FROM <image id>` and `docker-image://<id>` do not work and are not used. It labels the result `candler.lock=<digest>`.
4. After a build, and on every cache hit, read the layers of base and env from `DockerDriver::image_inspect`, which returns `Option<ImageInfo { id: ImageId, layers: Vec<LayerDigest>, labels: Labels }>` (D20), so this leaf never shells out for them. The env list must start with the base's whole list. A cache hit whose prefix fails rebuilds the tag (a changed base; the old image stays and cells pinned to its ID stay valid). `.Parent` is absent on the containerd store, so the prefix is the only check. The pinned `FROM` was proven on the containerd store only; the S1 `container-e2e` CI job probes it on overlay2 (D20).
5. `ensure` recomputes the sha256 of the lock bytes carried by `BuildSetting` (D20) and refuses a mismatch with the `LockDigest` (`ImageError::LockMismatch`, naming neither value's content).
6. The `docker` client env is the allowlist `DOCKER_HOST`, `DOCKER_CONTEXT`, `HOME` (README, Docker practice), separate from the agent's. It comes from `ClientEnv` in `isolation/docker.rs` (container_stop's).
7. Class A, in `#[cfg(test)] mod scaffold` of `image.rs`: the recording `DockerDriver` fake, then a sequence of `image_inspect` answers (absent; present; present with a wrong prefix) with literal call lists; plus an `#[ignore]` real-daemon test that builds both images with `--label candler.test=1` and asserts the prefix and the label (its output is the gate in the header). No image is removed.
**Deliverables**: `crates/candler-core/src/isolation/image.rs` (`DockerImageService`, `ImageAssets`, `ensure`, private `ensure_base`, `BaseImage`, `rootfs_prefix`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(/isolation::image/)' && cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(isolation): build the pinned env image once per lock digest`

## Step 3: Read the Claude Code version from the image
**Goal**: `cc_version` reports what the binary says, through a control container that removes itself.
**Implementation Logic**:
1. `cc_version<'a>(&'a self, image: &'a ImageRef) -> BoxFuture<'a, Result<Version, ImageError>>` runs one `docker run` through `run_captured` with the labels `candler.role=control` and `candler.job=preflight` (R11), `--rm`, `--network none`, `--read-only`, `--cap-drop ALL`, `--security-opt no-new-privileges`, `--init`, pids and memory limits, an entrypoint `timeout` of a few seconds (coreutils, so an expired probe exits 124 and is an error here), and the full image ID as the image. The guard never destroys a control container, so none is exempted, and none is created without `--rm`.
2. The stdout is `2.1.311 (Claude Code)\n`. The parser is `^(\d+\.\d+\.\d+) \(Claude Code\)$` on the trimmed line (R05b); any other output, a non-zero exit or a timeout is `ImageError::VersionUnreadable`. Class A: a table of literal outputs (the exact bytes, `v2.1.311`, an empty string, two lines, a prerelease).
3. After a build, `ensure` compares this readout with the `candler.cc_version` label that `DockerDriver::image_inspect` returns in `ImageRef.cc_version`, and refuses a difference. The binary is the authority; the label only lets `image_inspect` answer without a container.
4. The control call records into a temporary record dir under the store with marker `control` (R03 §16, finding 16), never into a job's `children/`.
**Deliverables**: `crates/candler-core/src/isolation/image.rs` (`cc_version`, `parse_cc_version`, `ImageError::VersionUnreadable`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(/isolation::image/)'` (expected: PASS)
**Commit**: `feat(isolation): read the Claude Code version from the image`

## Step 4: Prove one build per lock digest
**Goal**: A race for one tag never builds twice.
**Implementation Logic**:
1. `tests/guard_image.rs`, B05 `b05_one_build_per_digest`. Doc lines: `/// Guards: B05 — two concurrent ensure_env calls for one lock digest make exactly one build (plan D2, flock per tag).` `/// Oracle: the PATH-fake docker's argv log, counting build lines for the tag.` `/// Bite: take the flock after the "tag exists" check, not before.`
2. Two `ensure` futures over one real `DockerCli` (container_stop's `isolation/docker.rs`, with its binary path set to the PATH-fake) joined concurrently on a multi-thread runtime, with the same `LockDigest`. Asserts: one `build` line naming `candler-env:<digest>`; both calls return an `ImageRef` with the same ID. The fake is the S3 PATH-fake; its `image inspect` answer comes from a marker file its `build` writes (D20). A control case: a second `ensure` after both finish adds no `build` line.
3. The expected count is a literal with a provenance comment, never read from the code.
**Deliverables**: `crates/candler-core/tests/guard_image.rs` (`b05_one_build_per_digest`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_image)'` (expected: PASS)
**Commit**: `test(isolation): prove one build per lock digest`

## Step 5: Delete the scaffolding and record the bites
**Goal**: Only the permanent guard remains, and the bite is on record.
**Implementation Logic**:
1. Delete `mod scaffold` from `image.rs` and any `tests/scaffold_image.rs`.
2. Try B05's mutation (lock after the check), confirm `guard_image` fails, and restore with `git checkout -- crates/candler-core/src/isolation/image.rs`, never `mv`. This mutation does not signal or destroy, so the pre-spawn check is not needed.
3. Run the three feature-map checks and the RootFS and version checks (gates above) once against the built images and paste the outputs. Write the leaf report with the bite record and any open points found.
**Deliverables**: `crates/candler-core/tests/guard_image.rs`; the leaf report under `__reports__/m2b-execution-parity/`
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_image)' && ! git grep --untracked -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/isolation/image.rs crates/candler-core/tests` (expected: PASS)
**Commit**: `test(isolation): drop the image scaffolding and record the bite`
