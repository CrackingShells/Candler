# Amendment A7: M6a compares treatments as paired arms

Source of amendment A7 of `__roadmap__/candler-v1/`. Eliott approved it on 2026-11-16. The gap
turned up while M5 was being planned and built: HTN asked how to compare two skill versions, and
the M5 coordinator checked the answer in the code.

## The gap

Candler cannot express two treatments as compared arms. Measured at `origin/main` 18142ec:

| Fact | Where |
|:--|:--|
| `request.treatment` is an operational setting | `resolve/layers.rs` `is_setting` |
| A factor whose levels name a setting is refused | `runner/plan.rs` `fresh_cells`, `PlanError::SettingAsFactor` |
| One resolve has one treatment, one env lock and one image, copied into every cell | `resolve/steps/image.rs` `ImageIdStep::run`; `runner/plan.rs` `fresh_cells` |
| A `treatment:<path>` requirement source is read at the one treatment's pinned commit | `resolve/steps/image.rs` `source_of` |
| `analyze` tests only cell pairs inside one experiment that differ in one declared factor | `report.rs` `factors_between`; R01 §Analysis statistics |

So two skill versions take two experiment files. These are two strata, and `analyze` prints each
one's rates and Wilson intervals side by side, with no test between them. Per-arm treatment
dependencies (the `treatment:` source) vary the lock only between experiments.

## Why M6a, and why it is not a single-case feature

Comparing agent systems under controlled conditions is Candler's purpose (README), and a treatment
is the thing an author most often compares. M6a decides how arms are paired and tests them. If an
arm cannot be a treatment, M6a's tests cannot compare two versions of a skill, prompt tree or
plug-in. The need comes from HTN run 2 and Phase 8, but the capability holds for every treatment.

## Decision

| # | Decision | Reason |
|:--|:--|:--|
| 1 | M6a gains treatment as a declared factor | Paired tests between treatments are the comparison M6a exists for |
| 2 | Each cell resolves its own treatment, env lock and image; equal inputs share one lock and one image | `treatment:` sources and the treatment tree differ per arm, so the lock and image can too, and I7 keeps them declared |
| 3 | The plan decides the wire shape before any code: which path varies, how the setting rule treats it, how per-cell resolution is ordered and cached | Settings stay out of cells (I7); one-treatment experiments must not move a digest |
| 4 | Rejected: a cross-experiment comparison in `analyze` | It compares strata that may differ in more than the treatment, and it gives nothing else, such as per-arm dependencies |

## Effects on the tree

| Node | Change |
|:--|:--|
| `stats/m6a_paired_stats.md` | Goal, gates, step 1 and step 2 deliverables gain the treatment factor; the title is unchanged |
| `stats/README.md` | Context and goal name treatment arms; A7 logged |
| `README.md` | A7 logged |

No node is added, moved or removed. M6a's budget grows. Its plan gives the figure.
