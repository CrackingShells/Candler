# design_record: leaf report (v0)

R01 (`__design__/architecture.md`) now records what M5a builds. Only R01 and this report changed.

## R01 sections

| Section | Now says |
|---|---|
| §Process supervision, Containers | One docker client builds every docker process: absolute binary, allowlist `DOCKER_HOST`, `DOCKER_CONTEXT`, `HOME`; no `PATH`, because nothing resolves a program through it and the gateway never pulls, builds or pushes. |
| §Process supervision, agent container and stop | `docker create`, record, `docker start -a`; `-i` on both when stdin is fed; no cidfile. A stop is `docker stop --time <grace> <id>` and the client is never signalled while the daemon is reachable; the supervisor ladder is the lost-daemon fallback; full grace only when TERM is ignored. |
| §Agent environment and secrets | Client allowlist corrected (`HOME`, no config dir); table rows `isolation.egress` (cell value) and `isolation.build.plugin` (setting); the egress value is parsed, canonicalised and refused with `isolation.egress_unsupported` at resolve and per cell. |
| §Plug-ins | `agent_environments` row; D4 rules: derived layer below the experiment file, lists append, pairs from one layer (F3), duplicate requirement names only with a plug-in (F4), CLI inputs, unknown plug-in `resolve.preflight_failed` (F2), unknown environment `isolation.plugin_environment_missing`. |
| §Future features | "Decided and scheduled" now names M5b (egress proxy, guard reuse, ground truth refusal with e2e, live smoke); new row: catalog capture of input-delivered servers. |

## Claims checked at d6389815

- `CLIENT_VARIABLES = [DOCKER_HOST, DOCKER_CONTEXT, HOME]` in `isolation/docker.rs`; `gateway.rs` has no `PATH`.
- `create_argv` and `start_argv` in `isolation/container/run.rs` carry `-i` on both when interactive; `start` uses `-a`; no cidfile in `container/` or `isolation/`.
- `isolation.egress_invalid`, `isolation.egress_with_bridge`, `isolation.plugin_environment_missing`, `resolve.preflight_failed` exist; `plugin_egress`, `unite_egress` and `egress::settle` exist.
- Dockerfile/context split refusal and the duplicate requirement-name refusal are in `resolve/steps/image.rs`.

## Findings

- Not on this branch, written as the brief states: the `docker stop` stop path, `isolation.egress_unsupported`, and the lock/requirements split refusal (`lock_bytes` still lets a plug-in lock win silently; the coordinator's F6 fix is pending).
