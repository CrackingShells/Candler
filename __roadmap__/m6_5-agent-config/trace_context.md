# Trace context

**Goal**: Every session's normalised trace begins with a `context_start` event that holds the system text and the first message of the run's first model request that offers tools, verbatim, as the gateway saw them, on both harnesses. It is what lets L3 prove from the trace that the instructions and the session-start hook reached the model. (P0, L3 reads it)
**Estimate**: Sonnet, $8, cap $16. At the cap, stop, commit what is green, and list what is left in the leaf report. Builds use one slot of `/Users/hacker/Documents/tmp/m6_5-target/slot-1..3`, and the leaf removes it when it merges. The leaf deletes the containers it starts. An adversarial review follows the spec-verifier pass.
**Pre-conditions**:
- [ ] `[cmd]` `git -C <worktree> merge-base --is-ancestor feat/m6.5-agent-config HEAD` succeeds
**Success Gates**:
- ✅ G12. In a run on Claude Code and in a run on OpenCode against the fake model, the session's first trace event is `context_start`, and its `system` equals the system text of the run's first request that offers tools, as the gateway logged it. On the base, there is no such event.
- ✅ A trace written before the change still reads, and the schema's version bump is minor.
- ✅ One reader of the gateway log serves both the census and `context_start`; no second reader exists. The trace contract page documents the event, and states the measured size of the gateway's log line on both harnesses.
- ✅ The guards run in a pass that lists them as passed, not skipped. `docker ps -a` shows none of their containers afterwards.
**References**: [Architecture report, v2](../../__reports__/agent_config/02-architecture_agent_config_v2.md) — §Live trace proof, invariant 8 and §Execution model; `crates/candler-schemas/src/trace.rs` (`KnownKind`); `crates/candler-core/src/isolation/gateway.rs` and its njs script; `crates/candler-core/tests/it/guard_census_gateway.rs` (the census reader of the gateway log)

## Step 1: Record the first request and emit the event
**Goal**: The gateway's first-request text becomes the trace's first event.
**Implementation Logic**:
The gateway already logs each request's sequence number and tool names per run and tag. It also logs, for the run's first request that offers tools and carries a context, the request's system text and first message (OpenCode's sequence 0 is a title request with no tools). The core's existing gateway-log reader, the one the census uses, reads them, and the trace normaliser places `context_start {system, first_message}` before the session's first event. The kind is additive to `KnownKind`, so the schema bump is minor and old traces read unchanged. Regenerate only this leaf's own schemas and pages. The codebase is shared: extend the reader, do not copy it, and leave no scaffolding. This leaf touches no file that `declaration` touches.
**Deliverables**: `crates/candler-schemas/src/trace.rs` (`KnownKind::ContextStart`, its payload type); `crates/candler-core/src/isolation/gateway.rs` and its njs script (the first-request record); the gateway-log reader (the `context_start` event); the trace contract page; a guard `guard_trace_context_start` beside `guard_census_gateway.rs`
**Consistency Checks**: `cargo nextest run -p candler-core --run-ignored all -E 'test(trace_context) | test(census_gateway)'` (expected: PASS)
**Commit**: `feat(core): record each run's starting context in the trace`
