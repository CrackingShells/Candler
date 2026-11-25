# agent_config

Instruction files and hooks as per-arm experiment data (M6.5).

## Documents

### Round 00
- [00-architecture_agent_config_v0.md](00-architecture_agent_config_v0.md) — superseded by v1. The approved plan: cause, shape, harness check, proof, risks, roadmap.
- [00-notice_consumer_requirements_v0.md](00-notice_consumer_requirements_v0.md) — superseded by v1. The HTN eval lane's four requirements checked against the plan: three met, evaluator blinding (d) open for Eliott.

### Round 01
- [01-architecture_agent_config_v1.md](01-architecture_agent_config_v1.md) — superseded by v2. The plan with Eliott's two rulings on the notice: a global instruction file per harness plus a read-only workspace-root file per arm, and evaluator access deferred. Gates G1–G10.
- [01-notice_consumer_requirements_v1.md](01-notice_consumer_requirements_v1.md) — **latest**. The HTN lane's four requirements against v1: three met, evaluator access (d) deferred.

### Round 02
- [02-architecture_agent_config_v2.md](02-architecture_agent_config_v2.md) — latest architecture. v1 plus Eliott's ruling that the proof is live and read from the trace: a new `context_start` trace event from the gateway's first request, a new L1 leaf `trace_context`, a session-start hook marker, and a live RiVault confirmation run. Gates G1–G13, cap $85.

### Round 03
- [03-findings_probe_harness_checks_v0.md](03-findings_probe_harness_checks_v0.md) — **latest**. The level-1 probe, measured in the real images: the check command for each harness, where Claude Code's global file loads, the order and place (system or first message) in which instructions and session-start hooks reach the model. Two points amend v2's proof wording.

### Round 04
- [04-observation_level1_out_of_scope_v0.md](04-observation_level1_out_of_scope_v0.md) — **latest**. Eight findings from level 1 that lie outside its leaves: the 4 MiB gateway log read, an unlocked image build, a commit that does not build alone, OpenCode's silence on unknown keys, `mcp.servers` names, a timing guard, an unused image, a child not identified for signalling.

## Status

The plan is approved, and the roadmap [`__roadmap__/m6_5-agent-config/`](../../__roadmap__/m6_5-agent-config/README.md) has been authored. The plan and the roadmap were amended on 2026-11-25 for two instruction scopes (v1), then for a live, trace-read proof (v2). Level 1 is dispatched, and the probe is done.

On 2026-11-25 Eliott ruled on the two points that round 03 raised. The roadmap's leaves carry the amended gates:
- The proof reads the whole starting context. `context_start` comes from the run's first request that offers tools. G4 and G11 pass when each text is found in its `system` or its `first_message`, since Claude Code puts CLAUDE.md texts and SessionStart output in the first message.
- Consumers write `hooks` and `instructions.global` in the harness's own shape and keys (`claude-code`, `SessionStart`, `start.sh`). The experiment surface takes them as opaque subtrees.

Level 1 is done and merged locally. On 2026-11-25 Eliott raised the cap from $85 to $125, after level 1 measured $55.40: planning $14, build $41. Level 2 runs on closed-scope briefs and a fresh agent for each fix round.

No decision is open.
