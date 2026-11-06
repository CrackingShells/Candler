# Run evidence: catalog, census, leaks

**Goal**: `candler_core::runner::evidence` captures the catalog snapshot through a command run in a control container, writes the census of the tools and MCP servers the agent saw as `measured[] census.*`, refuses an output that equals an input by digest as `invalid(input_leak)`, and builds the run's `tree.json`.
**Pre-conditions**:
- [ ] [cmd] The Phase 0 scaffold is landed: `for s in '^docs(design): ' '^build(repo): ' '^ci(repo): ' '^feat(schemas)!*: ' '^feat(orchestrator): scaffold'; do git log --format=%s origin/main..feat/m2b-execution-parity | grep -q "$s" || exit 1; done && test -s __roadmap__/m2b-execution-parity/feature_map/feature_map.json`
- [ ] [nl] api_review closed with no open finding on `CatalogProvider`, `Evidence`, `Inputs`, `Session` and `RunTree` against M2a, M3, M4 and M5
- [ ] [cmd] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path
- [ ] [cmd] `ls crates/candler-harness-claude-code/tests/fixtures/streams/multi_init crates/candler-harness-claude-code/tests/fixtures/streams/synthetic_mcp`
**Success Gates**:
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_run_evidence)'` passes, holding two B tests. B24: an output whose sha256 equals an input's gives `invalid(input_leak)`, and the same file with one byte changed stays valid (oracle: the OS `sha256` of hand-built files). B25: the census from a real init frame records `tools` and `mcp_servers` under `measured[] census.*` with `census.init_tools` sorted (oracle: `jq` over a stream from `candler-harness-claude-code/tests/fixtures/streams`, read by relative path)
- ⬜ [cmd] STD, LINUX and SUBJ pass
- ⬜ [nl] The bite record gives a tried mutation per B test: B24 compares sizes instead of digests; B25 writes `mcp_servers` into `census.init_tools`
- ⬜ [cmd] No scaffolding: `! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/evidence crates/candler-core/tests/guard_run_evidence.rs`
- ⬜ [cmd] No leaf edit outside the owned paths: `git diff --name-only <base>..HEAD | grep -v -e '^crates/candler-core/src/runner/evidence/' -e '^crates/candler-core/tests/guard_run_evidence.rs$' -e '^crates/candler-core/tests/scaffold_' | wc -l` prints 0
- ⬜ [nl] No verifier is required for this leaf; every gate above is `[cmd]` or the bite record
**References**:
- [R01 §Raw traces, §Run store, §Ends, I8, I10, I11](../../__design__/architecture.md) — invalid reasons, `tree.json`, measured values
- [R03 §7, §8, §10.2, §13](../../__reports__/m2b-execution-parity/00-architecture_v0.md) — `CatalogProvider`, `Evidence`, `Inputs`, control containers (R11), the ends table
- [R04 B24, B25](../../__reports__/m2b-execution-parity/01-test_definition_v0.md) — oracles and bites
- [M1 trace and schemas](../m1-contracts/README.md) — `Session`, `RunTree`, `MeasuredValue`

## Step 1: The command catalog provider
**Goal**: `CatalogProvider::capture` runs the catalog command in a control container and returns the catalog.
**Implementation Logic**:
1. `CommandCatalogProvider::new(Arc<Supervisor>, Arc<dyn DockerDriver>, CatalogCommand)` implements `CatalogProvider`. `capture<'a>(&'a self, image: &'a ImageRef, cell: &'a Cell<'a>) -> BoxFuture<'a, Result<Catalog, CatalogError>>` runs the command through `Supervisor::run_captured` in a container labelled `candler.role=control` and `candler.job=preflight`, with `--rm` and an inner `timeout` (R11); the argv comes from `isolation::container::run::run_argv` with a control plan, not from this leaf. The container removes itself and the guard never destroys it.
2. The command's stdout is the catalog JSON; a non-zero exit, a timeout, or output that does not parse is `CatalogError::{Command, Timeout, Parse}`. The catalog digest is the sha256 of the canonical JSON, and the catalog keeps no absolute path.
3. The command runs on a control call: no stop hook is attached (R7), so nothing recurses.
4. Class A in `#[cfg(test)] mod scaffold`: a PATH-fake `docker` answering canned catalog output, a failing exit, and malformed JSON.
**Deliverables**: `crates/candler-core/src/runner/evidence/catalog.rs` (`CommandCatalogProvider`, `CatalogCommand`, `Catalog`, `CatalogError`, `catalog_digest`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(catalog): capture the catalog through a control container`

## Step 2: The census
**Goal**: The tools and MCP servers the agent saw are measured values, so a drift in them shows in the comparability verdict.
**Implementation Logic**:
1. `census(sessions: &[Session]) -> Result<Vec<MeasuredValue>, CensusError>` reads the first init of the first session and returns three measured values: `census.tools`, `census.mcp_servers` and `census.init_tools`. `census.init_tools` is the init's tool names, sorted and deduplicated, because the init tool list is what `stratum_verdict` compares (`comparability`). The `mcp_servers` list never enters `census.init_tools`.
2. A stream with no init frame gives `CensusError::NoInit`, which `run_exec` records as the end `unknown` with `invalid(infrastructure)`; it is never an empty census.
3. The values carry their source (the raw frame) as the measurement says; no absolute path.
4. Class A: ordering and deduplication over a hand-built session; a multi-init stream reads only the first.
**Deliverables**: `crates/candler-core/src/runner/evidence/census.rs` (`census`, `CensusError`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(census): measure the tools and servers the agent saw`

## Step 3: Input leaks and the tree builder
**Goal**: An output that equals an input by content invalidates the run, and the run's tree is built from the evidence.
**Implementation Logic**:
1. `input_leak(&Evidence, &Inputs) -> Option<InvalidReason>` compares each `OutputDigest` with each `InputDigest` by sha256 and returns `Some(InvalidReason::InputLeak)` on the first equal digest. Sizes, names and paths are never compared (a copied input is renamed, and a different file may share a size). It runs on collected output only (R2 step 9).
2. `tree_json(&Evidence) -> Result<RunTree, EvidenceError>` builds the run tree through `RunTreeBuilder` from the sessions and outputs, with store-relative paths only (I14). The workspace path is a container path and is never stored. It lists no path under a directory that holds a `pyvenv.cfg` (feature_map X35, S3.R1-R3) and none under a read-only input mount point (decision 13); both rules belong here, not to the driver (A26).
3. Class A: equal digest under a different name, equal size with different bytes (stays valid), and no inputs (stays valid).
**Deliverables**: `crates/candler-core/src/runner/evidence/leak.rs` (`input_leak`); `crates/candler-core/src/runner/evidence/tree.rs` (`tree_json`, `EvidenceError`)
**Consistency Checks**: `cargo nextest run -p candler-core -E 'test(scaffold)'` (expected: PASS)
**Commit**: `feat(runner): refuse input leaks and build the run tree`

## Step 4: B24 and B25 guards, scaffold deletion and bite record
**Goal**: The leak check and the census are held by two guards with independent oracles, and no scaffolding stays.
**Implementation Logic**:
1. `tests/guard_run_evidence.rs` (`#![cfg(test)]`). B24 `b24_digest_equal_output_is_a_leak`: the test writes two hand-built files, takes their digests with the OS `sha256` (literal values in a provenance comment), then asserts that an output equal to an input gives `invalid(input_leak)` and that the same file with one byte changed stays valid (positive control). Doc lines: `Guards: I10 — an output that copies an input`, `Oracle: OS sha256 of hand-built files`, `Bite: compare sizes instead of digests`.
2. B25 `b25_census_from_a_real_init`: reads a stream from `candler-harness-claude-code/tests/fixtures/streams/synthetic_mcp` by relative path, reads it through `ClaudeCodeReader`, and asserts that `measured` holds `census.tools`, `census.mcp_servers` and a sorted `census.init_tools`, against literals taken from `jq` over the same stream (command in a provenance comment). Doc lines: `Guards: I10 — the census`, `Oracle: jq over the fixture stream`, `Bite: write mcp_servers into census.init_tools`.
3. Delete every `#[cfg(test)] mod scaffold` and `tests/scaffold_*`. Try each mutation, restore with `git checkout -- <file>`, and write the bite record in the leaf report.
**Deliverables**: `crates/candler-core/tests/guard_run_evidence.rs` (`b24_digest_equal_output_is_a_leak`, `b25_census_from_a_real_init`); the leaf report's bite record
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_evidence)' && ! git grep -n -e 'mod scaffold' -e 'scaffold_' -- crates/candler-core/src/runner/evidence crates/candler-core/tests/guard_run_evidence.rs` (expected: PASS)
**Commit**: `test(census): guard the input leak check and the census`
