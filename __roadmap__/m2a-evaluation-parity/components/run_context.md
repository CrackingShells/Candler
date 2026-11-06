# Run context: assembly, digest, materialiser

**Goal**: `evaluation::context` builds, for one stored run and one checked suite, the `RunContext` a plug-in receives. Its artifacts are chosen by the need globs under R03's file rule and its metadata comes from the run manifest. It materialises those artifacts into a per-request workspace that cannot write back into the store, and computes the `context_digest` from exactly what it materialised (RC1–RC3).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and DEP(protocol_wire)
- [ ] [cmd] `uv run --with rfc8785 python -c 'import rfc8785'`
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^crates/candler-core/(src/evaluation/context/|tests/(guard_run_context\.rs|oracle/vectors/(gen_live_context\.py|live_context_runs\.toml)|public_surface\.rs)$)` with only the `run_context_surface` block of `public_surface.rs` changed
- ⬜ [cmd] `cargo nextest run -p candler-core -E 'binary(guard_run_context)'` passes (RC1–RC3)
- ⬜ [cmd] `uv run --with rfc8785 python crates/candler-core/tests/oracle/vectors/gen_live_context.py --check crates/candler-core/tests/fixtures/mini-corpus` regenerates `live_context_runs.toml` byte for byte
- ⬜ [nl] The adversarial verifier holds no open BLOCKER or MAJOR: the projection, links against digest inputs, AppleDouble, an empty `outputs/`, I11 write-through
**References**: [R01 §Evaluator protocol (Digests of a request), §Run store](../../../__design__/architecture.md) — `context_digest`, I11; [R03 §Needs, §File rule, §Environments](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); `crates/candler-schemas/src/evaluation.rs` — `RunContext::builder`, `ContextArtifact`, `context_digest`; `crates/candler-core/src/store/` — `blob`, `run`, `layout`; `crates/candler-core/tests/oracle/vectors/{gen_vectors.py,context_digest_runs.toml}` — the M1 generator's conventions; [R04 RC1–RC3](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md)

## Step 1: Assemble the context from needs
**Goal**: The artifacts and metadata of a context are exactly what the suite entry's needs name, chosen by one written file rule.
**Implementation Logic**:
1. `assemble(store: &Store, run: &RunKey, needs: &Needs) -> Result<Assembled, ContextError>`: list the run's stored `outputs/` files, keep those matched by any artifact glob, and drop every path with a component starting with `.` unless a glob names that component literally. `assemble` takes the schemas' `Needs` and compiles the globs itself; `snapshots` and `inputs` that are not empty are `ContextError::Unsupported`. That drops `.claude/`, AppleDouble `._*` and `.DS_Store`. Paths are relative to `outputs/` with `/` separators. An empty `outputs/` gives no artifacts and is not an error.
2. Metadata: one key per metadata need, the dotted path itself, the value the run manifest's `comparable.values` JSON leaf; a missing path is a `ContextError` naming it.
3. Snapshots and inputs are empty (A–C scope).
4. If the stored run cannot give a rule's inputs, stop and report a spec defect rather than inventing a source.
**Deliverables**: `crates/candler-core/src/evaluation/context/{mod,assemble,rule}.rs` (`assemble`, `Assembled`, `ContextError`, `file_rule_keeps`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): assemble run contexts from suite needs`

## Step 2: Materialise the workspace and digest what was written
**Goal**: The plug-in reads private copies, and the digest is computed from those copies' bytes.
**Implementation Logic**:
1. `materialise(assembled, workspace: &Path) -> Result<RunContext, ContextError>`: copy each artifact's blob into `<workspace>/outputs/<path>`, never a hard link or symlink to a store blob, with files made read-only after writing. Then hash each written file. Build the `RunContext` with `RunContext::builder()`, whose `context_digest` the schema computes, from those hashes.
2. If a written file's hash differs from the store's recorded blob digest, return a `ContextError` (a corrupt store), never a context.
3. The workspace lives under the request's temporary directory, outside the store root. Its removal is the engine's job.
**Deliverables**: `crates/candler-core/src/evaluation/context/materialise.rs` (`materialise`, `Workspace`)
**Consistency Checks**: `cargo clippy -p candler-core --all-targets -- -D warnings` (expected: PASS)
**Commit**: `feat(evaluation): materialise contexts into private workspaces`

## Step 3: Guard the digest, the file rule and write isolation
**Goal**: RC1–RC3 decide the context by independent Python vectors, hand-listed sets and the store's own blobs.
**Implementation Logic**:
1. `tests/oracle/vectors/gen_live_context.py` (`uv run --with rfc8785`, stdlib otherwise): for the three real mini-corpus runs, it reads the raw `outputs/` files and `experiment.json` and applies the file rule as R03 writes it in prose. With needs `outputs/**` and metadata `plan.prompt_domain_name`, it writes `live_context_runs.toml` (projection and digest per run), headed like `context_digest_runs.toml`.
2. `tests/guard_run_context.rs` (`#![cfg(test)]`), on a store holding the M1 mini-corpus import:
   - RC1 (R9): each run's materialised context digest equals `live_context_runs.toml`; a twin run with one artifact byte changed gives a different digest. Oracle: the Python `rfc8785` vectors. Bite: omit artifacts from the digest.
   - RC2 (R9): a table of synthetic `outputs/` trees (nested match, `.claude/x.py`, `._domain.py`, `.DS_Store`, an empty `outputs/`); the materialised path set and the digest's path list each equal a hand-listed set. Oracle: hand literal. Bite: keep `.claude`.
   - RC3 (R9, I11): append to a materialised file (after `chmod u+w`, as a careless plug-in would); the store blob's sha256 is unchanged. Oracle: the store blob's recorded sha. Bite: materialise with a hard link.
3. Fill `run_context_surface`, delete scaffold tests, record bites, dispatch the adversarial verifier.
**Deliverables**: `crates/candler-core/tests/oracle/vectors/{gen_live_context.py,live_context_runs.toml}`; `crates/candler-core/tests/guard_run_context.rs` (RC1–RC3); the `run_context_surface` block
**Consistency Checks**: `cargo nextest run -p candler-core -E 'binary(guard_run_context)' && uv run --with rfc8785 python crates/candler-core/tests/oracle/vectors/gen_live_context.py --check crates/candler-core/tests/fixtures/mini-corpus` (expected: PASS)
**Commit**: `test(evaluation): guard the context digest, file rule and isolation`
