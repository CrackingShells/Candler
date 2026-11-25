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
- [02-architecture_agent_config_v2.md](02-architecture_agent_config_v2.md) — **latest**. v1 plus Eliott's ruling that the proof is live and read from the trace: a new `context_start` trace event from the gateway's first request, a new L1 leaf `trace_context`, a session-start hook marker, and a live RiVault confirmation run. Gates G1–G13, cap $85.

## Status

The plan is approved, and the roadmap [`__roadmap__/m6_5-agent-config/`](../../__roadmap__/m6_5-agent-config/README.md) has been authored. The plan and the roadmap were amended on 2026-11-25 for two instruction scopes (v1), then for a live, trace-read proof (v2). No leaf has been dispatched, and no decision is open.
