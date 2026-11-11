# stray_sweep_tick: leaf report (v0)

## P1

Question: does `b33_setsid_descendant_dies` flake because of the strict `>` on Linux start ticks, and does `>=` remove the flake?

How it ran. Container `m3-linuxrun` (linux/arm64, 12 CPUs, 7.7 GiB), clean tree at a26fe53, one container per batch of 100:

    docker run --rm -v "$PWD":/w -w /w -v m3-linux-target:/target -e CARGO_TARGET_DIR=/target \
      -e CARGO_BUILD_JOBS=2 m3-linuxrun cargo nextest run --no-tests=fail --retries 0 \
      --stress-count 100 --no-fail-fast -p candler-core --test guard_job_runtime \
      -E 'test(=b33_setsid_descendant_dies)'

`CARGO_BUILD_JOBS=2` and `--test guard_job_runtime` are needed: the first attempt without them was killed by the linker (signal 9, out of memory) while linking the other test binaries in parallel.

| Rule | Batch | Red |
|---|---|---|
| strict `>` (a26fe53) | 1 | 9/100 |
| strict `>` | 2, with the sweep's recorder dumped on failure | 16/100 |
| strict `>` | 3 | 21/100 |
| `>=` on the Linux ticks comparison of `started_after` (uncommitted, restored) | 1 | 0/100 |

The probe edits (the `>=`, and the recorder dump in the assertion message of the test) were local and were reverted with `git checkout -- crates`.

Every red run of batch 2 (16 of 16) carries the same recorded step. One of them, for the orphan pid 302, whose start (ticks 48579) equals the supervisor's child's start tick:

    ProcessStopStep { pid: 302, ..., step: Sweep, outcome: Some("refused: the process did not start after the caller") }

Every red run of batches 1 and 3 fails at the same assertion, "the sweep did not find the detached process" (guard_job_runtime.rs:302).

Decision: land. Strict fails in at least one run, every inspected failure names the same-tick refusal, and `>=` passes 100 of 100.
