# egress_setting leaf report (findings)

Status: STOPPED before implementing. No code, no codes.toml row, no guard was written. The leaf's
Step 1 rests on a premise the code contradicts (F1), and its wiring needs files outside OWNED (F2).
Base e7aa7bb.

## 1. Commits
| sha | subject |
|---|---|
| (this) | reports(isolation): record the egress_setting leaf |

## 2. Gates
| gate | result |
|---|---|
| Pre-condition: `cargo metadata ... workspace_root` | PASS (the worktree path) |
| Every other gate | BLOCKED by F1 and F2 (nothing built, nothing to run) |

## 3. Guards
None written: a guard on "the step canonicalises the value" would be written against a premise
that cannot hold.

## 4. Claims checked
| claim | what the code shows |
|---|---|
| A resolve step canonicalises `isolation.egress` | FALSE. See F1. |
| The two codes fit `<surface>.<reason>` | TRUE. `isolation.*` is an existing section of `docs/contract/codes.toml` (e.g. `isolation.env_name_invalid`). |
| A refusal gets its code from a `refusal_table!` row plus a `codes.toml` row | TRUE (`resolve/refusal.rs`, `isolation/refusal.rs`; checked by `guard_refusal_codes::r1_*`). But a step cannot return a coded error of its own. See F2. |
| `isolation.egress` is a cell value, not a setting | TRUE. `is_setting` (`resolve/layers.rs:38`) does not list it, and the path is in `LEAVES` (`layers.rs:90`). |
| `experiment/surface.rs` reads the cell value | PARTLY. `Surface` has no caller outside one test (`guard_resolve_surface`); the cell digest is computed from resolved values, not from `Surface`. |

## 5. Findings
- **F1 (spec defect, blocks Step 1.1 and 1.3). A step cannot canonicalise a value the author wrote.**
  `Layered::resolve` (`resolve/layers.rs:366`) picks explicit > experiment file > project > user >
  derived > core default. A step's `derive` lands in the derived layer, so for an experiment that
  writes `isolation.egress` the written list wins and the canonical list is ignored without error.
  So the cell digest would depend on the case, order and duplicates the author wrote, and the
  leaf's goal (canonical in the cell) fails. The same holds for D4: `plugin_environment` cannot
  "append plug-in entries first" by deriving `isolation.egress`, because its derived value loses to
  the experiment's; and two steps deriving one path are refused (`DerivedTwice`, `preflight.rs`).
  A factor level that sets `isolation.egress` per cell is a third path: steps see only the
  experiment-wide values.
- **F2 (outside OWNED). A refusal needs files the leaf does not own.** A step returns
  `Result<StepReport, PreflightError>`; its failed checks reach the user only as
  `resolve.preflight_failed`. `experiment.session_refused` shows the route a coded refusal takes: an
  error enum, a `ResolveError` variant and `session_refusal()` in `resolve/preflight.rs`, a row in
  `resolve/refusal.rs`, and a per-cell second check in `runner/plan.rs` (a factor can set
  `isolation.network` to `bridge` per cell). All four are outside the leaf's OWNED line.
- **F3 (needs a ruling). Where canonical form and the bridge rule live.** Candidate shapes, none
  of which I implemented:
  1. Canonicalise at layering: `Layered::resolve` (or a `Layered` pass) rewrites the winning
     `isolation.egress` into canonical form whatever its layer, and a pure `egress::canonical`
     function is the one parser; the step then only refuses. Moves `layers.rs`.
  2. Canonicalise at planning (as `session::settle` does), per cell, so factor levels work; the
     digest is then taken from the canonical value. Moves `plan.rs`; D4's append has the same home.
  3. Keep the step as pure validation (refuse non-canonical input) and drop "canonicalise": this
     changes the leaf's goal and D3.
  Either 1 or 2 also settles how `plugin_environment` appends (D4), which has the same defect.
- **F4.** The leaf's step-order instruction ("runs after the plug-in step registers; leave a slot
  and a comment in `steps/mod.rs`") is moot until F1 is ruled: the order matters only if both
  steps derive the same path, which the resolver refuses.

## 6. Docker
Not touched. No image, container or network was recorded or created. No target dir was built;
nothing to clean.
