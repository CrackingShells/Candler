# M5a fix leaf A: stop path, lock split, egress refused

Type: findings. Branch `m5/leafA`, cut from `feat/m5-container-isolation` at d6389815.

## 1. Commits
| sha | subject |
|:--|:--|
| e956ffe3 | fix(isolation): stop the agent container through the daemon |
| 18019f53 | fix(isolation): refuse a lock split between plug-in and experiment |
| fdbc086f | feat(isolation): refuse egress until the proxy lands |

## 2. Gates
| gate | result | last line |
|:--|:--|:--|
| SPAWN | PASS | 9 passed |
| STD (`-p candler-core -p candler`: fmt, clippy `-D warnings`, nextest, doc) | PASS | 422 passed, 25 skipped |
| LINUX | PASS | clippy `--target x86_64-unknown-linux-gnu --workspace --tests` clean |
| DIGEST | PASS | 45 passed; vectors numstat 0 changes |
| GOLDEN | PASS | both runs green |
| SUBJ | PASS | `cog check d6389815..HEAD`, subjects at most 72 characters |
| E2E | PASS | 15 run, 15 passed |
| OWNED | PASS | see finding 3 |

## 3. Guards, oracles, bites
| guard | Guards | oracle | bite and failing message |
|:--|:--|:--|:--|
| `guard_container_stop::stop_asks_the_daemon_and_falls_back_to_the_client` | a stop asks the daemon and leaves the client alone while it answers; the ladder ends the client only if the daemon fails or the client outlives the container | groups the recording signaller saw and how the run ended; three daemon answers (stops, unreachable, client stays) | fallback removed: `the daemon is unreachable: the run did not end, left: None`. Stop skips the daemon: `the daemon stops the container: the daemon is asked once, left: 0` |
| `guard_container_e2e::create_e2e::agent_create_then_attach` (stop row extended, `term-echo` now exits 143) | after a stop the agent's post-TERM line is recorded and the exit is the container's | coordinator's probe table: trap TERM, exit 143, line relayed | stop signals the client: `stop: ended Stopped, Code { code: 143 }, output ... got-term` (red on `Ended`: the daemon's stop in the ladder still delivered the line) |
| `guard_plugin_environment::layer_pair_from_two_layers` (5 rows added) | the lock and the requirements list come from one layer | D4's pair rule: plug-in lock beside experiment list, the reverse, plug-in both, experiment both, lock alone | lock check dropped: `plug-in lock, experiment list: resolved` |
| `guard_egress_setting::{egress_refused_at_resolve, egress_refused_per_cell}` | a well-formed list is refused with `isolation.egress_unsupported` at resolve and per cell | the new code; parse table untouched | refusal removed: `a well-formed list: Ok(())`, `a factor level: Ok(1)` |
| `guard_egress_setting::egress_unset_leaves_the_cell` (was the unset half of `egress_enters_the_cell`) | no egress, or an empty list, keeps the cell | digest `ea0e539a...` recorded for egress_setting | unchanged oracle; passes |
| `guard_plugin_environment::{layer_lists_append, layer_egress_merge_reads_the_plugin_report_only}` | a plug-in's egress, alone or beside the experiment's, and from a stored resolution, is refused per cell | the code; the pure `plugin_egress`/`unite_egress` rows stay | refusal removed: both plan instead of refusing |

## 4. Findings
1. **Deadline stop (fixed in fix round 1).** The ladder in `supervise/stop.rs` (OWNED for this change only) no longer signals right after a successful hook `pre_stop`: it lets the child drain for the stop grace and signals only if `pre_stop` failed or the child is still alive. Rows added to the stop guard: deadline with a working daemon is never signalled; with an unreachable daemon, or a client that stays, the ladder signals and the run ends. Bite (signal right after `pre_stop`): `deadline, the daemon stops: the client was signalled, left: [(pid, 15)], right: []`.
2. **Behaviour change.** A cancel that was asked for is now the run's stop even when the client exits by itself (`stopped = cause` unless the deadline came first, `runner/exec/mod.rs`); before, only `Ended::Stopped` counted. A natural exit racing the cancel now reads as stopped.
3. **OWNED additions, all needed.** `supervise/stop.rs` (round 1), `runner/exec/{mod,session,spawn}.rs` (the stopper is built in `spawn.rs`), `isolation/container/stop.rs` (`stop_agent`), `isolation/mod.rs` (`EndFacts::stop_agent`, default refuses so a backend without a container ends the client at once), `assets/docker/testagent/testagent.sh` (`term-echo` exits 143), `tests/it/guard_container_stop.rs` (new, one `mod` line).
4. **Egress rows removed, restorable.** `egress_enters_the_cell`'s set-value rows (digest differs, canonical list in the cell, spelling twin, factor twin) are gone with the refusal; the canonical form stays in the parse table. M5b removes the one `refuse_until_proxy` call in `plan.rs` and the egress step and brings the rows back from history. The cell-writing code in `plan.rs` is unchanged.
5. **Details.** An empty list stays an absent value, not refused. An invalid entry or bridge keeps its own code ahead of `egress_unsupported`. The newly refused input is the coordination session's ruling.
6. **Not done.** The Grit reviewer pass was not run (turn cap). `lock_split` reads the plug-in's requirements from the fragment and the experiment's from the resolved value.

## 5. Docker
Before: 16 images, 3 containers, 5 networks. After: containers and networks unchanged; the image list differs by one id each way (`98aaaa150a71` gone, `1fcbe9158428` present; an `01e5b1f42031` also appeared and went), which I did not remove or keep: the testagent build of `agent_create_then_attach` and a sibling's builds both run here. Pulled: none. E2E test: `guard_container_e2e::create_e2e::agent_create_then_attach` (extended, no new e2e).
