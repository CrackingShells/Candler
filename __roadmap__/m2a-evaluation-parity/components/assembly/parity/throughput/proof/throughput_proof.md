# Prove parity and throughput on the corpus

**Goal**: On the full corpus with `htn-creator-eval` 0.5.1, GP1–GP3 pass, the regrade grades at least 100 items/s, and the import takes 30 s or less.
**Pre-conditions**:
- [ ] [cmd] `git log --first-parent --format=%s feat/m2a-evaluation-parity | grep -qx 'merge(evaluation): land regrade_throughput'`
- [ ] [cmd] `git -C "$R" rev-parse -q --verify 'refs/tags/htn-creator-eval@0.5.1^{commit}'`, with `$RW` at the tagged plug-in tree
**Success Gates**:
- ⬜ [cmd] STD, SUBJ and SCAFFOLD pass, and OWNED passes with `^(crates/candler/tests/guard_golden_parity\.rs$|crates/candler-core/src/evaluation/engine/(mod|run)\.rs$|crates/candler-core/tests/public_surface\.rs$|crates/candler-core/tests/guard_engine\.rs$|crates/candler-core/tests/fixtures/engine/|__reports__/m2a-evaluation-parity/throughput/throughput_proof/)`
- ⬜ [cmd] SPAWN, then the golden_parity guard command passes. Its `timings.txt` shows an import of 30 s or less and each regrade at 89 s or less, which is 8,880 items at 100/s or more (before: 135.7, 642.5 and 680.8 s). The whole run takes 5 min or less.
- ⬜ [cmd] The README's `candler diff … | jq -e …` gate and the rates diff against `rates_by_cell.toml` pass on the kept store, with `--to htn-creator-eval@0.5.1`
- ⬜ [nl] The adversarial verifier holds no open BLOCKER or MAJOR. It was deferred here from golden_parity, because here a corpus pass costs minutes.
**References**: [golden_parity](../../golden_parity.md) — GP1–GP3 and their bites; [golden_parity report](../../../../../../../__reports__/m2a-evaluation-parity/golden_parity/00-report_v0.md) — the 0.5.0 run and its finding 3; [profile §4](../../../../../../../__reports__/m2a-evaluation-parity/throughput/00-regrade_profile_v0.md) — the projection

## Step 1: Prove parity against 0.5.1
**Goal**: The guard diffs `0.4.2` against `0.5.1`, the identity that now grades.
**Implementation Logic**:
1. `TO` and the guard's docs name `htn-creator-eval@0.5.1`. Everything else is read from the oracle files as before.
2. Run the guard once, keeping its store. Record its `timings.txt`, the nextest summary and the free disk before and after. Then run the two README CLI gates on the kept store, and delete the store.
3. Do not repeat a bite that golden_parity recorded, unless the verifier asks for it.
**Deliverables**: the 0.5.1 identity in `crates/candler/tests/guard_golden_parity.rs`
**Consistency Checks**: the guard command from the success gates (expected: PASS)
**Commit**: `test(evaluation): prove golden parity against htn-creator-eval 0.5.1`

## Step 2: Record the proof and hand it to the verifier
**Goal**: The report puts the before and after numbers side by side, and the deferred adversarial verifier attacks the guard and the throughput changes.
**Implementation Logic**:
1. `__reports__/m2a-evaluation-parity/throughput/throughput_proof/00-report_v0.md` holds:
   - the timing table, before and after;
   - the guard log;
   - the CLI gates;
   - the items/s figure and what bounds it now, compared with the profile's projection.
2. The coordinator dispatches the adversarial verifier, which runs at most one corpus pass. Its targets are golden_parity's (hide a mismatch behind `equivalence.json`, and attack the generator), plus three more:
   - the merged batches changed no outcome, including `prior[]` and `blocked` across a merged batch;
   - the sync module keeps the order write, sync, rename, sync directory;
   - the preload breaks no fresh-module rule.
**Deliverables**: `__reports__/m2a-evaluation-parity/throughput/throughput_proof/00-report_v0.md`
**Consistency Checks**: `test -s __reports__/m2a-evaluation-parity/throughput/throughput_proof/00-report_v0.md` (expected: PASS)
**Commit**: `reports(evaluation): record the throughput proof`

## Step 3: Report the deadline each request was given
**Goal**: A caller can read the deadline the engine gave each request, so a test can pin it without waiting for it to expire.
**Implementation Logic**:
1. `EvaluateReport` gains `request_deadlines: Vec<(RequestId, Duration)>`, with one entry per sent request, in the order of `request_ids`. The deadline is the one passed to the client: the flat override when one is set, and otherwise the sum of the items' deadlines plus `REQUEST_DEADLINE_MARGIN`.
2. The field is added only. `request_ids` and every serialised CLI output stay as they are. Update the `public_surface` anchor if it constructs the report.
**Deliverables**: `EvaluateReport::request_deadlines`
**Consistency Checks**: `cargo check --all-targets` (expected: PASS)
**Commit**: `feat(evaluation): report the deadline each request was given`

## Step 4: Close the verifier's guard gaps
**Goal**: Each A1 engine behaviour the verifier broke without failing a guard now fails ENG1. The corpus fixture is rebuilt after a plug-in edit.
**Implementation Logic**:
1. ENG1 gains three rows in `crates/candler-core/tests/guard_engine.rs`. Each row kills one of the verifier's surviving bites:
   - **Dependency order.** A fixture declares a dependency after its dependent in suite order. The request sends the dependency first. The bite to kill: `order()` follows suite order only (`engine/plan.rs`).
   - **Request deadline.** With no `request_deadline` override, a request of two entries with item deadlines of 40 s and 5 s reports a deadline of exactly 75 s in `request_deadlines`. A second request, with the flat override set, reports that override. Both expected values are hand literals from the fixture. No kind sleeps. The bites to kill: the deadline is the margin alone, the sum alone, or the largest item deadline plus the margin (`engine/run.rs`).
   - **NA blocks its dependents.** An entry that depends on the `not_applicable` entry N1 is written `blocked` and is not sent. The bite to kill: NotApplicable no longer stops dependents (`engine/run.rs`).
2. `guard_golden_parity.rs`:
   - its fixture marker `complete.json` also hashes the plug-in tree (`$RW/$PLUGIN`, every tracked file) and the helper source, so that a kept store is rebuilt after an edit to either;
   - the "about half an hour" strings in the module doc and the `#[ignore]` reasons give the measured time instead.
3. Record each bite in the report. Run the guard command once more, since the fixture marker changed.
**Deliverables**: the three ENG1 rows; the wider fixture marker
**Consistency Checks**: SPAWN, then `cargo nextest run -p candler-core -E 'binary(guard_engine)'` (expected: PASS), and each of the three bites fails it
**Commit**: `test(evaluation): guard dependency order, deadline sum and na blocking`

## Step 5: Record the verifier round
**Goal**: The report shows each verifier finding with what was done about it.
**Implementation Logic**:
1. `00-report_v0.md` gains a section "Verifier round". It has one row per finding: its severity, the fix or the ruling, and the bite evidence for Steps 3 and 4. It also has the new guard run's `timings.txt`.
**Deliverables**: the section in `__reports__/m2a-evaluation-parity/throughput/throughput_proof/00-report_v0.md`
**Consistency Checks**: `grep -q '^## Verifier round' __reports__/m2a-evaluation-parity/throughput/throughput_proof/00-report_v0.md` (expected: PASS)
**Commit**: `reports(evaluation): record the throughput verifier round`
