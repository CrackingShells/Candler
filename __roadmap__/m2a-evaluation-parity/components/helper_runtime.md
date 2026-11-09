# Helper runtime: Python evaluator loop

**Goal**: `candler_evaluator` serves the evaluator protocol on stdio for any plug-in. It forks one child per item, guards the fds so only valid frames reach stdout, and reports its own identity from PEP 610 metadata, so that one item's hang, exit or stray write costs that item alone (HR1) and the instrument digest is truthful (HR2).
**Pre-conditions**:
- [ ] [cmd] ENV, DEP(skeleton) and DEP(protocol_wire)
- [ ] [cmd] `cd python/candler-evaluator && uv sync --frozen && uv run --frozen python -c 'import candler_evaluator'`
**Success Gates**:
- ✅ [cmd] PYSTD, SUBJ and SCAFFOLD pass, and OWNED passes with `^python/candler-evaluator/(src/candler_evaluator/|tests/|examples/)` and `test -z "$(git diff --name-only <base>..HEAD | grep -F _models)"` (POSIX ERE has no look-ahead)
- ✅ [cmd] `cd python/candler-evaluator && uv run --frozen pytest -q tests/test_guard_isolation.py tests/test_guard_identity.py` passes (HR1, HR2)
- ✅ [cmd] Fork probe, logged in the report: `for i in $(seq 20); do uv run --frozen pytest -q tests/test_guard_isolation.py || exit 1; done` from `python/candler-evaluator`
- ✅ [cmd] `test "$(find python/candler-evaluator/examples -type f -exec cat {} + | wc -c)" -le 10240` (bytes, not disk blocks)
- ✅ [nl] The plain verifier holds no open BLOCKER or MAJOR: lock state inherited across the fork, pre-fork imports, the digest across items
**References**: [R01 §Evaluator protocol, §Plug-ins](../../../__design__/architecture.md) — methods, notifications, deadlines, identity; [R03 §Python helper, §Protocol, §Truthful instrument](../../../__reports__/m2a-evaluation-parity/00-architecture_v0.md); [PEP 610](https://peps.python.org/pep-0610/) — `direct_url.json`; [R04 HR1, HR2](../../../__reports__/m2a-evaluation-parity/01-test_definition_v0.md); `crates/candler-schemas/tests/fixtures/rpc/` — the shared vectors

## Step 1: Serve the protocol loop with an fd guard
**Goal**: The helper reads requests line by line from stdin and writes only protocol frames to the real stdout, whatever a plug-in prints.
**Implementation Logic**:
1. At start, `dup` fd 1 to a private frame fd, then point fd 1 at fd 2, so plain `print`, `os.write(1, …)` and inherited child stdout all land on stderr. Frames go only through a `FrameWriter` on the private fd, one JSON object per line, flushed per frame.
2. `serve(plugin)` handles `describe` (returns the plug-in's `DescribeResult` with the identity of Step 3) and `evaluate` (Step 2), answers `cancel` with the `-32800` error, and exits 0 on stdin EOF. Messages are plain dicts checked against the envelope shape; `helper_release` replaces them with generated models.
3. Plug-in API (R03 §Kind signature): a plug-in module exposes `KINDS: Mapping[str, Callable[[params, context, prior], Outcome | Mapping]]`, which `serve` dispatches by the item's kind. `Outcome` is a small dataclass of the outcome fields, and `serve` accepts a plain mapping with the same keys, so a plug-in's kinds need not import `candler_evaluator`. `context` and `prior` are the duck-typed objects R03 pins.
**Deliverables**: `src/candler_evaluator/{__init__,loop,frames,plugin}.py` (`serve`, `FrameWriter`, `Plugin`, `Outcome`)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen ruff check && uv run --frozen pytest -q -m scaffold` (expected: PASS)
**Commit**: `feat(evaluator-helper): serve the protocol loop behind an fd guard`

## Step 2: Fork one child per item with a deadline
**Goal**: Each item runs in its own forked child with the item deadline, so a hang, an exit or a crash yields `not_measured` for that item only.
**Implementation Logic**:
1. Per item: `os.fork()`; the child first arms its own item deadline (`signal.setitimer(ITIMER_REAL, …)` with `SIGALRM` at its default action), runs the kind, writes one outcome as JSON to a pipe and `os._exit(0)`. The parent never signals anything (I22: the only stop is the core's request deadline, sent through `Supervisor`); it waits for the child and reports `not_measured {reason: instrument_error}`, with evidence naming the cause, on an alarm, a non-zero exit, another signal or an empty pipe. A child that survives its alarm is ended with the whole helper by the core's request deadline, and its item, never reported, becomes `not_measured` in the client.
2. Fallback: where `fork` is unavailable or `CANDLER_EVALUATOR_NO_FORK=1`, run the kind in-process under the same fd guard, with no item deadline; the describe result says which mode is active.
3. Pre-fork: import the plug-in and its kinds before the first fork; no thread is started before forking.
4. Each outcome is sent as an `outcome` notification as soon as it is known; the `evaluate` result carries the count.
**Deliverables**: `src/candler_evaluator/isolate.py` (`run_item`, `ForkRunner`, `InlineRunner`)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q -m scaffold` (expected: PASS)
**Commit**: `feat(evaluator-helper): isolate every item in a forked child`

## Step 3: Report a truthful identity
**Goal**: The helper reports the plug-in distribution's name, version and instrument digest from its installed metadata, and refuses to guess.
**Implementation Logic**:
`identity(dist_name) -> Identity` reads the distribution's `direct_url.json` (PEP 610): a `vcs_info` with `vcs == "git"` and no `dir_info.editable` gives digest `git:<commit_id>`; an editable install gives the commit if known plus `dirty = true`; a missing `direct_url.json` (an index install) raises `IdentityError`, which `describe` turns into an error response. No guessing from `git` on disk.
**Deliverables**: `src/candler_evaluator/identity.py` (`identity`, `Identity`, `IdentityError`)
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q -m scaffold` (expected: PASS)
**Commit**: `feat(evaluator-helper): report identity from pep 610 metadata`

## Step 4: Guard isolation and identity with a synthetic plug-in
**Goal**: The two cornerstone guards use committed synthetic and echo plug-ins and fail under their bites.
**Implementation Logic**:
1. `examples/synthetic_plugin/` (≤ 10 KB with the echo plug-in, hand-written): kinds `ok`, `hang`, `exit`, `print`, `write1`, `subprocess_echo`. `examples/echo_plugin/`: one kind that returns the context and prior it received as evidence (`engine` ENG2 reuses it).
2. `tests/test_guard_isolation.py`, HR1 (R6): one request mixing every synthetic kind with `ok` items; only the faulty items are `not_measured`, every `ok` item is right, and the helper's real stdout parses line by line as envelope frames against the shared WIRE1 vectors' rules. Oracle: hand literal. Bite: drop the fd redirect.
3. `tests/test_guard_identity.py`, HR2 (R7): dist-info trees generated in `tmp_path` (clean `git+file`, editable, none); expect `git:<sha>`, `dirty = true`, refusal. Oracle: PEP 610 text. Bite: ignore `editable`.
4. Delete the scaffold tests, record bites, run the fork probe, dispatch the plain verifier.
**Deliverables**: `examples/{synthetic_plugin,echo_plugin}/`; `tests/test_guard_isolation.py` (HR1); `tests/test_guard_identity.py` (HR2); `tests/conftest.py`
**Consistency Checks**: `cd python/candler-evaluator && uv run --frozen pytest -q && test -z "$(git grep -n 'mark.scaffold' -- tests src)"` (expected: PASS)
**Commit**: `test(evaluator-helper): guard item isolation and identity`
