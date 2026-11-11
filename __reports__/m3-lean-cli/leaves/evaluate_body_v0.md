# evaluate_body, leaf report v0

An evaluation as a supervised job body: the evaluator slot pool, plug-ins from `plugins.dirs`, an engine that selects runs and reports each committed outcome, and `supervise_evaluate`.

## P4

Question: are an evaluator's descendants reparented to the evaluate job's supervisor, and when do strays appear?

Rig: the Linux image of LINUXRUN, a throwaway test (deleted, not committed) that made itself the subreaper (`become_subreaper`), the `engine_fixture` manifest template with a kind that starts `sh -c 'trap "" TERM; sleep 120'` through `Popen(start_new_session=True)` and returns `pass`, the `guard_engine` environment, and `evaluate` over two runs. A thread listed every process whose parent is the test, with state and command line, every 10 ms (finer than the 50 ms the leaf asks for). Three variants: A, two requests at once, no stray sweeper; B, two requests at once, `ProcMarkerSweeper` installed on the supervisor; C, one request at a time, no sweeper. Each ended with one `sweep_orphans`.

| fact | A (concurrency 2, no sweeper) | B (sweeper) | C (concurrency 1, no sweeper) |
|---|---|---|---|
| stray's parent when first seen | the test (subreaper) | the test | the test |
| delay from the evaluator's exit to that | within one 10 ms poll | within one poll | within one poll |
| a stray with the test as parent while its evaluator is alive | never | never | never |
| stray alive until | the final `sweep_orphans` (~500 ms after the engine returned) | killed 0 to 10 ms after the evaluator's exit, by the request's own ladder sweep; the final sweep found nothing | the final `sweep_orphans`; request 1's stray lived through all of request 2 |
| the evaluator, once exited, a zombie child | no `Z` seen for any `uv run` or `python` process | none | none |
| `sweep_orphans` killed | 2 | 0 | 2 |

Each evaluator is two processes, `uv run` (child of the test) and its `python` (child of `uv run`, which does not exec); the kind's stray is a child of `python`. A stray therefore has the test as its parent from the instant `python` exits.

**Answer: strays are reparented to the supervisor at the end of each request**, when their evaluator exits, never earlier. They outlive their request: the stop ladder signals the evaluator's group, and the stray left it with `setsid`. Without a marker sweeper the stray stays until the supervisor sweeps (C: request 1's stray ran through request 2). With `ProcMarkerSweeper` on the supervisor, the sweep that the ladder runs after every request kills it by its marker in about 10 ms, so the strays never reach the end, provided the marker is the request's own. The evaluator processes themselves are waited for by the supervisor's runtime (no zombie leader).

Rule that follows: the body sweeps once, after the engine returns, as the net (needed by E3, and for a stray that the ladder's marker sweep did not find). No sweep after each run plan: with the per-request marker the ladder's sweep already ends a stray per request, and a stray that escaped it was idle in this probe (state `S`, no CPU, no slot-sized resource) while later requests ran. The probe's stray sleeps; a CPU-bound stray was not tried, because the per-request sweep (B) ends any stray that carries the marker.
