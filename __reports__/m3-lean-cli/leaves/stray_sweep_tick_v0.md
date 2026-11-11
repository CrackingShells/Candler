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

Final check after the fix (same command, committed tree): 100/100 passed.

## What changed

| Commit | Change |
|---|---|
| `fix(jobs): read a process's parent from two sources that agree` | `ProcStat`, `ProcStatus`, `ProcFile`, `CROSS_CHECK_READS`, `read_proc_checked`, `read_proc_stat` in `platform.rs`; `parse_proc_stat` is now a wrapper; `state_parent_group` is gone and `read_facts` reads through `read_proc_stat`; the six `s4_` guards in the new `tests/guard_proc_stat.rs` |
| `fix(jobs): sweep orphans that start in the supervisor's tick` | `started_after` becomes `started_no_earlier` (`>=` on Linux ticks of one boot and on darwin `(sec, usec)`); refusal text `the process started before the caller`; S1 and S2 amended |
| `docs(design): let an adopted orphan share the supervisor's tick` | R01, the bullet at line 408, one hunk |

The guards were written before the code (the S4 file did not compile against the old surface; S1 and S2 failed on the reason text and the same-ticks row).

## Bites (mutation applied, seen red, reverted)

| Guard | Mutation | Red |
|---|---|---|
| S1 | `>` restored in the Linux arm of `started_no_earlier` | `signal_adopted_orders_linux_starts_by_boot_and_ticks`, row "the same ticks" |
| S1 | Linux arm returns `true` / boot comparison and ticks loosened to any start | same test, row "fewer ticks" (a refusal was expected) |
| S2 | darwin arm left strict (`>`) | `signal_adopted_refuses_all_but_a_verified_orphan_and_its_verified_group`, the start-100 row |
| S4 | `find(')')` in place of `rfind(')')` | `s4_proc_stat_reads_every_field_for_any_command_name`, and on Linux `s4_a_real_process_with_a_hostile_name_reads_back_its_true_parent` |
| S4 | field index off by one (`nth(16)` to `nth(15)`) | `s4_proc_stat_reads_every_field_for_any_command_name`, `s4_proc_stat_refuses_a_line_that_is_not_a_stat_line` |
| S4 | parent and group swapped | `s4_proc_stat_reads_every_field_for_any_command_name`, `s4_cross_check_accepts_a_disagreement_that_clears_on_a_reread` |
| S4 | the `status` cross-check skipped (`if true`) | `s4_cross_check_refuses_when_stat_and_status_disagree_on_every_read`, `s4_cross_check_accepts_...` |
| S4 | re-read loop cut to one read | `s4_cross_check_accepts_a_disagreement_that_clears_on_a_reread`, `s4_cross_check_refuses_...` (it counts the reads) |
| S4 | re-read loop left unbounded (`loop`) | `s4_cross_check_refuses_when_stat_and_status_disagree_on_every_read` (the reader fails the test past its limit instead of hanging) |
| S4 | the group term of the cross-check deleted (`status.group.is_none_or(..)`) | `s4_cross_check_refuses_a_group_that_stat_and_status_disagree_on`, `s4_cross_check_accepts_a_group_disagreement_that_clears_on_a_reread` |
| S4 | the pid check replaced by `if false` | `s4_cross_check_refuses_files_that_name_another_pid` |
| S4 | `status` unreadable or unparseable answered from `stat` alone | `s4_cross_check_refuses_when_status_cannot_be_read_or_parsed` |

## Notes for the coordinator

- LINUXRUN needs two adjustments on this host: the Docker VM has 7.7 GiB, and linking every test binary of `candler-core` in parallel is killed by the OOM killer (`ld terminated with signal 9`). Use `-e CARGO_BUILD_JOBS=2` and `--test <binary>`. `guard_signal` also needs `docker run --init`: with the test process's parent being the container's pid 1, four of its rows (`parent()` asserts `pid > 1`) fail without it, on any commit.
- The strict-rule failure rate varied between batches (9, 16, 21 of 100), so a 10-run check would not have shown the flake reliably.
