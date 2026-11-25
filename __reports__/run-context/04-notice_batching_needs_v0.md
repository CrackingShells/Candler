# A kind's reuse key and refusal follow its batch — Notice (v0)

Date: 2026-11-25

---
type: notice
title: Make a kind's reuse key and refusal follow its own needs
tag: gap
effort: M
reversibility: engine-internal; no wire change in either idea
evidence: verified
spotted-during: the seam review after level 2 of the run context campaign (ruling 3 of `02-seam_review_v0.md`)
date: 2026-11-25
status: open
---

## TL;DR
- A kind's outcome reuse key and its refusal depend on the other kinds sent with it.
- Serving trace, raw streams and manifest makes the coupling costly; artifact globs already had it.
- Either idea changes how the engine batches or keys requests, and its guards.

## Context
The engine builds one context per request, and a request holds every applicable entry of one evaluator at one stage (`Batch`, `crates/candler-core/src/evaluation/engine/plan.rs`; `send`, `engine/run.rs`). The context is the union of those entries' needs, and its digest is part of each outcome's `I4Key` (`evaluation/selection/select.rs`). `docs/contract/evaluator-protocol.md` states this behaviour in a note; nothing changes it.

Two effects, with a plug-in that has `domain_valid` (needs one artifact) and `shell_discipline` (needs the trace):

| Effect | Example |
|---|---|
| Reuse misses | Suite v1 grades `domain_valid` alone, context digest D1. Suite v2 adds `shell_discipline` at the same stage, so the context is artifacts plus trace, digest D2. `domain_valid`'s key changes and it is graded again, though nothing it reads changed. An LLM judge pays again and may give a second, different grade. If `shell_discipline` is not sent because a dependency failed, the digest goes back to D1. |
| Over-refusal | On an imported run with no trace, `shell_discipline` cannot be served, so the whole request is refused with `evaluation.run_part_missing`, `domain_valid` included. |

## Scope Delta
Two ideas; the choice between them, or neither, is left to whoever implements it.
- **Idea A: split requests by needs.** One request per distinct set of needs within an evaluator's stage. Each kind is served exactly what it declared; its digest covers only that; a missing part refuses only the kinds that need it. Cost: one more plug-in process per extra needs set; kinds with equal needs still share a request.
- **Idea B: digest and refusal per kind, one request.** Keep the union context, but compute each entry's `I4Key` digest over its own kind's declared parts, and refuse only the entries whose kind needs a missing part, sending the rest a reduced context. Cost: per-entry digest and refusal tracking in the engine; each kind still sees parts it did not declare.
- Either way: the note in `evaluator-protocol.md`, the "what a plug-in is served" wording in the docs, and guards that see reuse and refusal from one kind's side.

## Accept / Decline

| | Accept | Decline |
|---|---|---|
| **Benefit** | A kind's stored outcome is reused whenever what it reads is unchanged, and an artifacts-only kind is graded on a run that lacks a trace | One request per evaluator and stage keeps process count minimal, and the current behaviour is already documented and guarded |
| **Cost** | An engine change to batching (A) or keying (B), new guards, and digests of contexts that serve several needs sets change | Re-grading cost and possible grade drift when a suite adds a kind with other needs; plug-in authors must split such kinds into separate plug-ins or stages themselves |

## If Accepted — Next Step
The coordinator assigns it to the milestone that owns evaluation; its planner picks A or B in an architecture report before any leaf.

## If Declined — Next Step
Keep the note in `evaluator-protocol.md` as the rule, and revisit when a consumer reports a reuse miss or an over-refusal.
