# egress_setting leaf report (findings)

Base e7aa7bb. The leaf first stopped on a spec defect (F1, F2), the coordinator ruled option 2
(F3), and the leaf was then implemented as ruled.

## 1. Commits
| sha | subject |
|---|---|
| 31aee37 | feat(isolation): resolve an egress allowlist of hostnames and ports |
| (this) | reports(isolation): record the egress_setting leaf |

(The first report commit, bb38a1a, recorded the stop; this report replaces its content on top.)

## 2. Gates
| gate | result | last line |
|---|---|---|
| Pre-condition: workspace_root | PASS | the worktree path |
| `guard_egress_setting::` (4 guards) | PASS | 4 passed |
| `egress_enters_the_cell` and DIGEST | PASS | 45 passed (core digest set); numstat of the oracle vectors zero |
| `guard_refusal_codes::` and `contract_pages::` | PASS | 8 passed; 7 passed |
| STD, `-p` forms | PASS | fmt ok; clippy -D warnings; doc -D warnings; core 273 passed; candler 134 passed |
| LINUX (`-p candler-core --tests`) | PASS | Finished |
| SUBJ (base e7aa7bb) | PASS | No errored commits; no subject over 72 |
| SPAWN | PASS | 9 passed |
| GOLDEN (touched plan.rs) | PASS | 2 passed; 1 passed |
| OWNED | PASS with additions | see F5 |
| E2E, DRIFT | not run | no container path, no schemas change |
| Grit reviewer agent | NOT RUN | clippy -D warnings passes; the review is left to the coordinator |

## 3. Guards
All in `crates/candler-core/tests/it/guard_egress_setting.rs`. Each carries `Guards:` and `Oracle:`.
| guard | Guards | Oracle | bite and the failing message |
|---|---|---|---|
| `egress_parse_table` (34 rows: 10 accepted, 24 refused) | D3: canonical form; each refused class has its code, naming the entry; bridge | RFC 1123 §2.1 by hand (label rule, 63/253 lengths worked by arithmetic), sort order of the text | IP literal accepted: `IPv4 literal: got Ok(Some([HostPort { host: "93.184.216.34", port: 443 }])), wanted Err(("isolation.egress_invalid", ...))`. Missing port defaulted to 443: `missing port: got Ok(Some([HostPort { host: "example.com", port: 443 }])), wanted Err(...)` |
| `egress_refused_at_resolve` | D3: refused before a job, code and entry named, step derives nothing | the codes of D3; written value read back | covered by the IP-literal bite: `an IP literal: Ok(())` |
| `egress_enters_the_cell` | D3: absent moves no digest; set enters as the canonical list; other order/case/repeats is the same cell; a factor level is canonicalised | the base digest of the plain cell, recorded before the change (ea0e539a...); the list by hand | written list kept: `the cell shows the canonical list, left: [pypi.org:443, files.pythonhosted.org:443], right: [files.pythonhosted.org:443, pypi.org:443]`. Path classed as a setting: `a set value is part of the cell`, both digests equal |
| `egress_refused_per_cell` | D3: a list set by a factor level with `bridge` fixed is refused per cell; a bad factor entry too; an empty list with bridge plans | the codes of D3 | per-cell call dropped: `a list with bridge: Ok(1)` |

Bites were injected from copies and restored from the copies. `contract_pages` and
`guard_refusal_codes` gained rows for the two codes (ResolveError and PlanError `Egress`).

## 4. Claims checked
| claim | what the code shows |
|---|---|
| A resolve step canonicalises the value | FALSE (F1). The derived layer is below file, project and user. |
| The codes fit `<surface>.<reason>` | TRUE; `isolation.*` is an existing section; the generated index lists them under isolation. |
| The cell digest of an absent value stays | TRUE; no code runs for it; DIGEST 45 passed, and the plain-cell digest is pinned. |
| `isolation.egress` is a cell value | TRUE; not in `is_setting`; in `LEAVES`. |

## 5. Findings
- **F1 (spec defect, resolved by ruling).** A step's derived value cannot override what an author
  wrote (`Layered::resolve`, layers.rs). The same holds for D4's append in `plugin_environment`.
- **F2.** A coded refusal from a step needs `preflight.rs`, `resolve/refusal.rs`, `runner/plan.rs`
  and `runner/refusal.rs`, as `experiment.session_refused` does.
- **F3 (ruling).** Option 2: one pure `egress::settle`; a refusing step on the experiment-wide
  values; the same call per cell in `plan.rs`, which writes the canonical list into the cell before
  `Comparable::from_resolved`. Nothing is prepared for `plugin_environment`'s append; the step-order
  comment is dropped. The step is registered last in `register_m2b_steps` (its doc says so).
- **F5 (OWNED additions).** Beyond the ruled three files: `runner/refusal.rs` (the `PlanError::Egress`
  row, same rule), `tests/it/guard_refusal_codes.rs` and `tests/it/main.rs` (append), and
  `docs/contract/index.md`, regenerated with `CANDLER_BLESS_CONTRACT=1`: two rows, nothing else.
  `codes.toml` is appended only (two `[resolve."isolation.egress_*"]` tables at the end of the file).
- **F6 (decisions the leaf did not state).** The canonical order is the sort of the `host:port` text
  (so `:443` sorts before `:8080`, and `:1000` before `:443`). A port is digits only, and leading
  zeros are dropped (`:0443` is `:443`). An empty list is no allowlist: `settle` returns `None` for it (after the bridge check, so `[]` with
  bridge is not refused), and planning drops the written `[]` from the cell, so it digests as absent. A last label of digits is refused, so `a.123:443` fails
  (RFC 1123 §2.1: a TLD is not all-numeric). The refusal's `detail` names the entry; the entry is
  not a separate field, because the report/check route carries text only.
- **F7.** The step's refusal reaches the user as `ResolveError::Egress` only when the egress step is
  the sole failing step, as for session; with several failing steps it is `resolve.preflight_failed`
  listing them. The per-cell refusal is always coded.
- **F8.** `Surface` (surface.rs) was not changed: it has no caller outside one test and the cell
  digest does not read it; the leaf's "egress reading in surface.rs" had nothing to add.

- **Fix round 1.** An empty list now settles to absent and digests as `sha256:ea0e539a...`; that digest was measured on e7aa7bb with a throwaway test in a scratch worktree (removed), and the guard's `Oracle:` names e7aa7bb. Bite: the empty list kept as `Some([])` gives a different digest and `isolation.egress` in the cell values.

## 6. Docker
Not touched; nothing recorded or created. The target dir and scratch are removed.
