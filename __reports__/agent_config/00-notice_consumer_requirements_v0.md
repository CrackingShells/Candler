# Notice: the HTN eval lane's requirements against the M6.5 plan

**Source.** The Candler milestone coordination session passed on the current positions of the HTN eval lane, Candler's main outside consumer. That lane's plan is not approved yet. No planned HTN experiment needs instruction files or hooks: its treatment is a skill directory at a tag, and the container already confines the agent the way hooks otherwise would. If it ever adds an instruction-file arm, it would need the four things below.

**Checked against.** [`00-architecture_agent_config_v0.md`](00-architecture_agent_config_v0.md) and the roadmap [`__roadmap__/m6_5-agent-config/`](../../__roadmap__/m6_5-agent-config/README.md), as committed in 8c7be6a1.

## Verdict per requirement

| # | Requirement | Plan | Status |
|---|---|---|---|
| a | The file's bytes are a factor level, hashed into the cell digest, so two arms differ only by those bytes. | `instructions.text` is a cell value, so it sits in `comparable.values`, which the digest hashes. Gate G2: two levels give two digests. | **Met** |
| b | It is installed at the workspace root, where each harness reads it. | Claude Code: `<workspace>/.claude/CLAUDE.md`, which Claude Code loads as project memory just as it loads the root `CLAUDE.md`. OpenCode: `$XDG_CONFIG_HOME/opencode/AGENTS.md`, the global file OpenCode always reads. | **Met in intent, not literally.** See below. |
| c | Every run records it in its provenance or manifest. | The text is in the manifest's `comparable.values`. Gate G7 adds the files the harness actually loaded to the census. | **Met**, and also proven loaded. |
| d | The evaluator cannot see it, as `request.treatment` is hidden today. | Not covered. An evaluator that names `instructions.text` or `hooks.*` among its metadata needs gets the value: `assemble` (`crates/candler-core/src/evaluation/context/assemble.rs:92`) serves any `comparable.values` leaf a need names. `treatment` is hidden only because it is its own root, outside `values`. | **Gap** |

## Why (b) does not follow the workspace root

The plan avoids the root deliberately:

- `<workspace>/CLAUDE.md` and `<workspace>/AGENTS.md` are collected as outputs. An installed file there would be graded as the agent's work, or seal the run `input_leak`; G9 guards against exactly this.
- A fixture may ship its own root `CLAUDE.md` or `AGENTS.md`. Writing to the root would overwrite it silently. The plan leaves it in place and the census reports it.

The consumer's need is the harness reading the file, and both placements meet it. No change is proposed.

## The decision for (d)

This is a design choice, not a defect: an evaluator grading "did the agent follow its instructions" may legitimately want the text.

| Option | What changes | Cost |
|---|---|---|
| **Blind, like the treatment (recommended)** | The evaluator registry's need check refuses a metadata need at or below `instructions` or `hooks`. One gate, G10, joins `declaration.md`, the leaf that owns the paths. | Small: one refusal, one unit guard. |
| Visible on request | Nothing changes. The page states that an evaluator naming the path receives the value. | None, but the blinding the HTN lane expects does not hold. |

The roadmap is unchanged until Eliott rules. If he picks the first option, G10 is added to `declaration.md` as an amendment, and every consumer of the gate list (the root's gates, the reminder `candler-M6.5-L1`) is updated in the same amendment.
