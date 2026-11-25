# Seam review after level 2

The review read `git diff origin/main...feat/run-context` at `512a4edb`, after all four level-2 leaves had merged. It was read-only and was briefed without the leaves' conclusions. It covered:

- the seams between producers and consumers;
- the wording on what a plug-in is served;
- the codebase's idioms and waste;
- whether the docs are true.

On that tip:

- `cargo fmt --check`, clippy and `cargo doc` pass;
- `cargo nextest run --workspace` ran 737 tests: all passed, 32 skipped;
- the helper's `pytest` passes (18 tests).

## Seams: nothing mismatched

Every field, key, code and path agrees on both sides of each seam:

- **`exit` (L1).** The schema refuses `null`. Neither reader writes it, and both write `exit` only on `tool_result`.
- **Served files.** The `{path, abs, sha256}` shape `assemble` writes is the one the helper's `ServedFile.from_wire` reads.
- **Context digest.** The keys `context_digest` adds are the ones `gen_live_context.py` and `gen_vectors.py` project.
- **Refusal code.** `evaluation.run_part_missing` is the same in `refusal.rs`, `codes.toml` and `index.md`.

## What a plug-in is served

The plan and several docs say a kind is served what it declares. The engine serves differently:

- It builds one context per request.
- That context is the union of the needs of the kinds of the entries the request sends (`crates/candler-core/src/evaluation/engine/run.rs:620`, over `sendable`).
- Entries that are blocked, not applicable or already finished contribute nothing.

So:

- a plug-in is served the union of what the kinds of its sent entries declare;
- every kind in the request can read any part another one declared.

The plan v0 (`00-plan_run_context_v0.md`, §Settled and §What a kind can see) and the done leaf `serve_context.md` are left as written, because they record what was approved and built. This note supersedes their sentences on what "a kind sees". The current docs, doc comments, contract pages, READMEs and the ruling note are corrected in the fix leaf below.

## Rulings

| # | Finding | Ruling |
|---|---|---|
| 1 | The `context_digest` doc says every path, relative included, is outside the digest, but the artifact `path` is in the projection (`evaluation.rs:1721`, `architecture.md:1054`) | Fix the wording |
| 2 | The helper's generated models read `null` on an optional field as absent, where the schema and the Rust reader refuse it. AGENTS.md calls a more lenient second reading a defect | Fix at the generator if it can make the models refuse `null` where the schema does with `pytest` green, guarded by a row seen red; otherwise stop and report. Keep the unbounded `int` for `code` against Rust's `i32` as a follow-up |
| 3 | Batching and outcome reuse: a kind that needs only artifacts is refused when a sibling entry in the same request needs a part the run lacks. Its context digest, and so its I4 key, also changes with which sibling entries were sent. Artifact globs already behaved this way; the served hashes make it matter more | Docs now: a Note on `evaluator-protocol.md` states that the context and its digest cover the union of the request, and that a missing part refuses the whole request. The design question is a follow-up for the maintainer |
| 4 | "Never served empty": every run stores `trace.jsonl`. A failed reader or a silent session leaves a header-only trace (`runner/exec/mod.rs:762`), which is served. `PartMissing` covers only runs that do not store the part | Fix the wording to "a part the run does not store"; the `Needs.trace` doc says a trace can hold a header alone |
| 5 | The `ContextArtifact.path` doc says "relative to the run". For an artifact it is relative to the outputs; for the trace and manifest, to the workspace root | Fix the wording, and the `ServedFile` docstring |
| 6 | `ContextRaw.path` is absolute while `ServedFile.path` is relative, and no doc says the raw streams are gzip-compressed | State both in the docs; no change to the shape |
| 7 | `Needs` ignores an unknown key in the Rust reader (schemars `deny_unknown_fields` without serde's) | Already open with the maintainer; unchanged here |
| 8 | Stale text: `architecture.md:1036` says "hard links or clones"; the `refusal_scope.tsv` note omits the missing-part refusal; the code's text omits the interrupted-run case | Fix the wording. `CorruptBlob` keeps its name, because the code is released |
| 9 | `crates/candler-harness-claude-code/tests/fixtures/digests.toml` is read by no code and names reader 0.2.0, and READMEs still describe it as a ratchet | Delete it, and the sentences |
| 10 | The OpenCode reader spells `"shell"` three times, where the Claude Code reader has `SHELL_TOOL` | One constant |
| 11 | In `guard_context_digest.rs`, the "trace path" and "manifest path" rows vary only `abs`, so the relative path is never varied | Vary the relative path, seen red under a bite that projects it |
| 12 | `evaluator-helper.md`'s Surface section is a paragraph, where CONTRIBUTING §Contract pages asks for one sentence | Back to one sentence; the README and the docstrings carry the API, as the page already says |
| 13 | The helper `CHANGELOG.md` says `read_trace` refuses "a line the schema refuses" | Make it true |
| 14 | The served rows' hashes come from the store, so the generator is independent only in the projection | Accepted by design: the hash of a stored file is the one thing the generator reads from the store |

## Follow-ups

- **The batching design for outcome reuse (ruling 3).** A kind's I4 key and its refusal depend on its sibling entries in a request. This is for the maintainer.
- **`Needs` unknown keys (ruling 7).** Open with the maintainer.
- **Python `ExitStatus.code` is unbounded, while Rust's is `i32`.**
- Carried from the leaves:
  - the OpenCode signal name mapping;
  - Claude Code hook denials, and OpenCode `permission.rejected`, read as denials;
  - a guard against a stray file in `raw/`;
  - writable modes on workspace directories.
