# Notice: the HTN eval lane's requirements against the M6.5 plan (v1)

**Source.** The Candler milestone coordination session passed on the HTN eval lane's positions. No planned HTN experiment needs instruction files or hooks. If one ever adds an instruction-file arm, it would need the four things below. v0 of this notice put two of them to Eliott; he ruled on both on 2026-11-25.

**Checked against.** [`01-architecture_agent_config_v1.md`](01-architecture_agent_config_v1.md) and the amended roadmap [`__roadmap__/m6_5-agent-config/`](../../__roadmap__/m6_5-agent-config/README.md).

## Verdict per requirement

| # | Requirement | Plan (v1) | Status |
|---|---|---|---|
| a | The file's bytes are a factor level, hashed into the cell digest, so two arms differ only by those bytes. | `instructions.workspace` is a cell value, so it sits in `comparable.values`, which the digest hashes. Gate G2: two levels give two digests. | **Met** |
| b | It is installed at the workspace root, where each harness reads it. | `instructions.workspace` is mounted read-only at `<workspace>/CLAUDE.md` on Claude Code and `<workspace>/AGENTS.md` on OpenCode. A global file per harness, `instructions.global.<harness>`, is set once for the experiment, and the workspace file overrides or complements it. The mount keeps the file out of the collected outputs and out of the agent's reach (G9), and an input at the same path is refused (G10). | **Met** (was "met in intent" in v0) |
| c | Every run records it in its provenance or manifest. | Both texts are in the manifest's `comparable.values`. Gate G7 adds the files the harness actually loaded, with their scope, to the census. | **Met**, and also proven loaded. |
| d | The evaluator cannot see it, as `request.treatment` is hidden today. | Deferred. The consumer writes the evaluator and knows which instructions the box holds, so no evaluator needs to read them back. Today an evaluator that names one of these paths among its metadata needs receives it, like any cell value. | **Deferred**, until a consumer brings a reason |

## What changed from v0

- (b): v0 argued against the workspace root because a root file is collected as an output and may overwrite a fixture's file. A read-only mount through the existing `mount_inputs` answers both: the tree builder skips it, and a collision is refused rather than shadowed.
- (d): v0 recommended blinding with a gate G10. There is no blinding work. G10 now names the collision refusal.
