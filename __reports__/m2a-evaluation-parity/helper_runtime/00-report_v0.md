# helper_runtime: leaf report (v0)

The implementer's first report follows as it came back. The rulings on its findings 1–7 are in [l1/00-rulings_v0.md](../l1/00-rulings_v0.md) §helper_runtime, and the verifier's in §helper_runtime verifier.

## First report
### 1. Commits (rebased onto feat/m2a-evaluation-parity 40138a6, fixups autosquashed)
- 6ba1190 feat(evaluator-helper): serve the protocol loop behind an fd guard
- 62740fa feat(evaluator-helper): isolate every item in a forked child
- 2a0268b feat(evaluator-helper): report identity from pep 610 metadata
- 631dded test(evaluator-helper): guard item isolation and identity

Only `python/candler-evaluator/{src,tests,examples}` changed. No Rust was touched. Each commit imports and passes `ruff check`. I did not run `cargo check` per commit because no Rust changed.

### 2. Gates
- PYSTD: PASS. pytest 5 passed, ruff check clean, ruff format --check "13 files already formatted".
- HR1+HR2 pytest: PASS.
- Fork probe: 20/20 PASS. Log at `$TMPDIR/m2a-helper_runtime-forkprobe.log`; runs took 0.43-0.98 s each, 33 s wall. I ran it again after the fixups and got 20/20.
- Examples size: PASS, 1760 bytes (limit 10240).
- SUBJ: PASS. `cog check` printed "No errored commits" and all subjects are 72 characters or fewer.
- SCAFFOLD: PASS. No `mark.scaffold`; 2 `def test_` and 2 `Guards:` lines.
- OWNED: PASS, checked with a Python `re` version of the leaf regex (12 files, 0 violations). The gate as written does not run; see finding 5.
- STD: PASS. SPAWN `guard_signal` 4/4; fmt, clippy, nextest (268 passed, 3 skipped), doc and linux-target clippy all passed (CHAIN-EXIT=0). Log at `$TMPDIR/m2a-helper_runtime-std.log`.
- Plain verifier (spec-verifier agent): no BLOCKER or MAJOR. Its MINOR M1 (HR2 had no non-editable `dir_info` case) and M2 (malformed item types crashed the helper) are fixed in the fixups. M3 is not fixed: after a cancel the abandoned child is not reaped until the helper exits. Two NITs are left (`_early_cancels` grows without bound; no-fork mode lets a kind read stdin or raise KeyboardInterrupt).

### 3. Guards
- **HR1** `test_one_fault_costs_one_item`. Guards line: "Guards: R6 — a hang or an exit costs its own item and no other, and a stray write costs nothing and never reaches the protocol pipe. Oracle: hand literals and the rules of the shared WIRE1 vectors."
  - The frame checker is itself validated against every valid and invalid vector file inside the test.
  - Bite 1: replace `os.dup2(2, 1)` in `frames.py` with `pass`. The guard fails (stray print lines on stdout).
  - Bite 2: replace the `ITIMER_REAL` value passed to `setitimer` in `isolate.py` with 0. The guard fails by timeout, since the hang never ends.
  - Both restored with `git checkout -- <file>`.
- **HR2** `test_identity_is_what_the_install_records` (4 parametrized cases: git, editable, index, plain dir). Guards line: "Guards: R7 — a clean git install reports git:<sha>, an editable install reports dirty, and an install that records nothing is refused. Oracle: the direct_url.json examples of PEP 610."
  - Bite: drop `and local.get("editable") is True` in `identity.py`. The `gh-dir` case fails (the bitten code accepts the plain dir install the test expects to be refused).
  - Restored with `git checkout`.

### 4. One-off records
- Fork probe as above.
- One-off schema check, not committed: every frame from a describe plus evaluate run validates against `evaluator-message.schema.json`.
- I22: no code in `src` signals anything. The child arms its own `ITIMER_REAL` and the parent only waits. The leaf did not ask for a child kill on timeout, so R03/I22 conflict with nothing in it.
- The `setitimer` bite left one orphaned `hang` child (pid 37201, a 3600 s sleep that exits by itself). I did not signal it.

### 5. Findings and deviations
1. **Spec gap, outcome record.** The wire `OutcomeRecord` requires `role`, `suite_entry_digest` and `context_digest`, which the helper cannot know (Item has no role, `depends_on` or `applies_to`). The helper emits role `descriptive` and an all-zero sha256 for both digests. Engine step 2 must overwrite all three; its text lists the digests but not the role. Please confirm, or define a lighter wire record.
2. **Spec gap, mode in describe.** `DescribeResult` has `additionalProperties: false`, so "the describe result says which mode is active" cannot be done. Deviation: a `log` notification `{level, message, isolation: fork|inline}` is sent just before the describe response.
3. **Spec defect, editable digest.** The schema forces `dirty=false` for a `git:` digest, and PEP 610 never puts `vcs_info` beside `dir_info`. So "editable gives the commit if known" cannot occur. An editable install's digest is the sha256 of its `direct_url.json` text with `dirty=true`. The helper refuses non-editable directory installs, archives, non-git VCS, index installs, and versions that are not SemVer.
4. **Spec gap, prior.** Item has no `depends_on`, so "one Prior per `depends_on` entry" cannot be built. Kinds receive every prior record of the request plus the records of earlier items of the same request, in order, and pick by expectation id.
5. **OWNED gate as written does not run.** `grep -vE` with `(?!_models)` is invalid ERE (ugrep errors) and `test -z` of the empty output passes. It needs `grep -P` or a Python check.
6. **API additions.**
   - `Plugin` dataclass (name, version, dist, instruments, specs, deterministic) and `Plugin.from_module`.
   - A `python -m candler_evaluator <module>` entry that installs the fd guard before importing the plug-in.
   - `RunContext` also exposes run, raw, trace, `artifact_files`, snapshots and inputs, which the echo plug-in needs for ENG2.
7. **Cancel.** While an item runs, the parent selects on stdin. On cancel it abandons the child (left to its own alarm) and answers -32800. A child that blocks SIGALRM survives until the core's request deadline, as the leaf says.
8. No `_models` created, no dependency added, nothing pushed.

## Plain verifier: findings and their fate
The verifier ran every runnable gate, the 20-run fork probe and both bites, and attacked state inherited across the fork, pre-fork imports, the identity digest, signals, the frame stream and one-fault-one-item. It found no BLOCKER or MAJOR.

| Finding | Fate |
|---|---|
| M1: HR2 has no `dir_info` case with `editable` false or absent, so the literal bite "ignore `editable`" leaves it green | Fixed: rows `gh-dir` and `gh-dir-off`. The literal bite now gives `2 failed, 3 passed` |
| M2: a wrongly typed `deadline_ms` or `kind` crashes the whole helper | Fixed: `_parse_evaluate` checks `kind`, `params`, `deadline_ms` (int, not bool, 0 to 2**53-1) and `expectation_id`, and answers -32602. New rows in HR1's table |
| M3: a child abandoned after a cancel is never reaped while the helper lives | Fixed: `ForkRunner.reap()` calls `waitpid(pid, WNOHANG)` on kept pids before each fork and at exit. No signal is sent |
| N1: `_early_cancels` grows for cancels of requests that never arrive | Accepted as a note |
| N2: the no-fork fallback lets a kind read stdin, raise `KeyboardInterrupt` or write to the frame fd | Accepted: the fallback is documented as having no isolation |

## Fix round (autosquashed, rebased onto 9683a54)
- Commits: `55232dd` (M2), `f72cc47` (M3), `45ff840`, `36be0d5` (M1 and the M2 rows).
- Bites, on a `git archive` copy:
  - HR1, `os.dup2(2, 1)` replaced with `pass`: fails.
  - HR2, `editable is True` dropped: fails, `2 failed, 3 passed`.
  - M2, `deadline_ms` validation removed: HR1 fails. The `kind` check removed: HR1 fails.
- Not bitten alone: the `isinstance` test on `deadline_ms`. The range check after it raises `TypeError` on a str or None, which is also answered -32602, so only the pair is pinned.
- M3 was checked by hand: an abandoned child in state `Z` before `reap()` is gone after it. A child that ignores its alarm and outlives the helper stays orphaned, as I22 requires.
- Harness note: `tests/conftest.py` sets `PYTHONPATH` for the helper subprocess, so a scratch bite must prepend its own `src`.

## Landing (coordinator, on 9683a54)

| Gate | Result |
|---|---|
| OWNED | PASS: nothing outside `^python/candler-evaluator/(src/candler_evaluator/\|tests/\|examples/)`, and no `_models` path |
| SUBJ | `cog verify` clean on each commit, the subjects equal the spec's four `Commit:` lines, all 72 characters or fewer |
| PYSTD | `6 passed`, `ruff check` clean, `13 files already formatted` |
| SCAFFOLD | PASS: 2 tests, 2 `Guards:` lines, no scaffold marker |
| Fork probe | 20 of 20 (implementer, after the fix round) |
| NOPUSH | no remote `feat/m2a-evaluation-parity` |
| STD | not re-run: no Rust changed. The implementer's first run gave 268 passed, 3 skipped |

The leaf landed as `b013f67` `merge(evaluator-helper): land helper_runtime`.
