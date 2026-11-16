# Amendment A6: the campaign becomes a dogfoodable 0.x

Source of amendment A6 of `__roadmap__/candler-v1/`. Eliott approved it on 2026-11-16, after he and
the coordinator reviewed the remaining milestones against the three tests of `AGENTS.md`
(sustainable, scalable, efficient).

## Decisions

| # | Decision | Reason |
|:--|:--|:--|
| 1 | No core feature is built for a single case | One eval's need is not evidence of a contract gap |
| 2 | Candler stays pre-1.0 through a long dogfooding period; no freeze in this campaign | What is stable is not yet known |
| 3 | M6 is split | Paired statistics need only M2a and the existing golden runs; the second plug-in is a different kind of work |
| 4 | The order is M5, M6a, M7, M8, strictly sequential | Each leaf sits one level below the previous one |
| 5 | M7 gains agent skills, before M8 | The second plug-in is authored and run through the MCP server |
| 6 | M8 adds no core feature | A gap goes to Future features; a blocking gap gets its own core milestone |

## Effects on the tree

| Before | After |
|:--|:--|
| `isolation/m5_container_isolation.md` and `isolation/m7_mcp_server.md` as siblings, `isolation/freeze/m6_second_plugin_freeze.md` below | M5 in `isolation/`, M6a in `stats/`, M7 in `stats/mcp/`, M8 in `stats/mcp/plugin/` |
| M5: outcome injection, census, egress, plug-in environments, a second isolation kind, A3's environment-kind gate | M5: egress allowlist, thin plug-in environments, the I8 guard, one Docker client and control-container builder (A5, A1, A8) |
| M6: second plug-in, paired statistics, freeze, "Deliberately open" decided, 1.0.0 tags | M6a: paired statistics. M8: second plug-in and gap report |
| "Deliberately open" in the design | "Future features": what, why, trigger |

## A3's assets-path item

A3's gate asked that a `candler` binary run outside the source tree find its assets, with
`commands/run.rs` no longer reading `CARGO_MANIFEST_DIR`. At `origin/main` (3d60cd9) no file of
`crates/candler/src` or its `build.rs` reads `CARGO_MANIFEST_DIR`: `commands/assets.rs` embeds
`assets/docker` with `include_bytes!`. The item is done. The remaining `CARGO_MANIFEST_DIR` read is
`candler-schemas`'s schema generator, which writes the committed schema files and is not a runtime
path. The rest of A3's gate (the environment kind) is superseded by A6; the Python-venv content of
the core stays a constraint on M5, which must not tie the core further to Python, and a Future
features item (environment kinds) with its trigger.
