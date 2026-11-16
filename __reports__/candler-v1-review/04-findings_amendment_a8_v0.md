# Amendment A8: M5 is split into M5a and M5b

Source of amendment A8 of `__roadmap__/candler-v1/`. Eliott ruled it on 2026-11-16, in chat (option B
of three), after the coordinator reported that the M5 campaign had reached its budget cap.

## Why

| Figure | Value |
|:--|:--|
| Cap of the approved plan | $60 |
| Spent when the coordinator stopped | about $56 (Opus coordinator $28.8, 12 Sonnet subagents $27.5; `docker_client` alone about $11) |
| Forecast of the remaining levels L2 to L4, as planned | $57 to $67 more (total about $115 to $125) |
| Forecast, lean | about $45 more (total about $100) |
| Earlier estimate | 4.5 times too low: it counted context size, which is not what is billed |

The options were a lean cap (A), a split (B) and a stop after the two running leaves (C). Eliott chose B.

## Decisions

| # | Decision |
|:--|:--|
| 1 | M5a is wire_shapes, docker_client, egress_setting, container_create and plugin_environment, plus a design record update. It lands as its own PR |
| 2 | M5a refuses a set `isolation.egress` (and a plug-in environment's egress) with the new code `isolation.egress_unsupported`, until the proxy lands. The cell value and its parse stay behind that refusal |
| 3 | M5a's plugin_environment is built without the catalog assertion; catalog capture of input-mounted servers is a Future features item of R01 |
| 4 | M5b has a fresh plan and budget with step-sized leaves: guard reuse (A1), the egress proxy (D1, D2, the `egress/` directory), ground-truth refusal merged with its e2e (D9, I8), the rest of the design record, the live smoke |
| 5 | The order is strictly M5a, M5b, M6a, M7, M8 |

## Effects on the tree

| Before | After |
|:--|:--|
| `isolation/m5_container_isolation.md`, with `isolation/stats/` (M6a) below it | `isolation/m5_container_isolation.md` is M5a; `isolation/m5b/` is new and wraps `stats/`, so M6a, M7 and M8 each sit one level deeper |
| One campaign `m5-container-isolation/` holding all of M5 | The same campaign holds M5a; its `client/seams/guard_reuse.md` and `client/seams/egress/` (egress proxy, ground-truth refusal, proof) are marked as moved to M5b and kept, with their statuses |

No node is deleted and no status changes. The relative links of the M6a, M7 and M8 leaves are
corrected for the extra level.
