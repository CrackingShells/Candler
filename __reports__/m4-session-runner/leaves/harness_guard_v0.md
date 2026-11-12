# harness_guard leaf report (v0)

Type: findings. Branch `m4/harness_guard`, base 4ce963f.

## Commits
| sha | subject |
|---|---|
| 66e2d34 | test(repo): deny harness names in the core and the schemas |

## Gates
| gate | result |
|---|---|
| L1 `binary(guard_layering_i1)` | PASS, 10 of 10 |
| no existing test removed (`git diff cf3de30` grep for removed `fn`) | PASS, no match |
| SPAWN, STD `-p candler-core` (fmt, clippy, nextest 276 passed, doc) | PASS |
| LINUX (`-p candler-core --tests`) | PASS |
| SUBJ | PASS |
| DIGEST | not run: no core import, resolve or run path touched |

## Guard
`harness_plants_are_found_only_in_the_core_and_the_schemas`. Guards: I1, no harness segment in `candler-core/src` or `candler-schemas/src`; the driver crates and the binary stay free. Oracle: the hand-written plant table (four plants found, three not).

Red first: before the rule it failed with `left: [] right: ["claude"]` for `claude_version` in the core. Green after `HARNESS_SEGMENTS` and the per-root rule `rules_for`.

## Bites on the real tree (each restored from a copy; diff identical after)
| plant | red test and message |
|---|---|
| `pub const claude_version: u8 = 1;` in `candler-core/src/lib.rs` | `workspace_holds_no_domain_segment`: "denied names in .../candler-core/src" |
| `"ANTHROPIC_API_KEY"` in `candler-schemas/src/lib.rs` | `workspace_holds_no_domain_segment`: "denied names in .../candler-schemas/src" |

The table also covers the scope attacks: widened to every crate, the claude-code, opencode and binary plants would report; narrowed to none, the core and schemas plants would not.

## Findings
- No `RENAMES` entry added: the rename test scans only the domain rules, and the mini-corpus holds no harness name. Deviation from the leaf's step text, harmless.
- `planted_tree` gained `candler-harness-opencode` so the opencode plant is scanned.
- The real-tree test keeps the name `workspace_holds_no_domain_segment`; its doc now also states the harness rule.
- Tests of `crates/candler-core/tests` hold harness names (oracle files); they are not scanned roots.

Net: 1 file changed, +86 / -3 lines.

## Verification
One verifier applied the four bites (a constant in the core, a string in the schemas, the rule widened to every root, the roots emptied); each turned the guard red as the leaf requires. It found one real defect: `OpenCode` and `open_code` split into `open` and `code`, so the single-segment rule missed the harness's natural Rust spelling. The coordinator added the sequence `open code` to the harness rule and a plant `enum Harness { OpenCode }` in the core; the plant is unreported with the sequence rule removed (red) and found with it (green).
