# Findings: conformance review of what shipped (wire-schemas, M1, M2b) at b2bb782

Four read-only reviewers checked `main` at b2bb782 (`v0.3.0`, `schemas@0.3.0`) on 2026-11-09, each against the specs its milestone was planned under:

| Reviewer | Specs read |
|:--|:--|
| wire-schemas | R01, `candler-v1/wire_schemas.md`, campaign `__roadmap__/wire-schemas/`, its approved plan, KT, CONTRIBUTING |
| M1 Contracts | R01, `contracts/m1_contracts.md`, campaign `__roadmap__/m1-contracts/`, its approved plan, test definition, KT, golden reference |
| M2b Execution parity | R01, `parity/m2b_execution_parity.md`, campaign `__roadmap__/m2b-execution-parity/` with its feature map, its approved plan, seven reports, the HTN reference harness |
| Cross-cutting | R01 invariants, the original architecture v3, the meta-plan, every meta-roadmap leaf (M2c–M7), CONTRIBUTING, the release history |

The reviewers ran no builds. The coordinator spot-checked every finding marked *verified* below in the code, and ran one probe test (W1).

## Verdict

- **The gates hold, mostly.** Every gate a milestone marked met is met, or the shortfall is recorded, with the exceptions in table C. Layering matches R01, and all six release bumps follow the rules.
- **Domain neutrality does not hold.** The core, the binary and `assets/` carry HTN-domain content that the I1 guard cannot see. Most of it came from M2b's parity gate, which asked for the reference's "same sandbox composition, same provenance fields". The I1 guard misses it because it matches literal names in three directories.
- **One real wire breach.** The catalog snapshot reader accepts a struct written as an array (W1).

## A. Domain content in the core: amendment A2, gate in M2c

| id | What | Evidence | Status |
|:--|:--|:--|:--|
| D0 | Planner axis and the binary's `gtpyhop` constant | `isolation/image.rs:513-630`, `supervise.rs:42`, `provenance.rs:78-85` | verified; original A2 |
| D1 | HTN packages built into every image | `assets/docker/env/stable.in` (`gtpyhop-core`, `libcst`, `python-arango`, `python-dotenv`, `mcp`, `jsonschema`), compiled into every lock (`image.rs:52`) | verified |
| D1b | HTN output files pruned in every image | `assets/docker/env/Dockerfile:12` deletes `domain.py` and `problems.py` | verified; planned in M2b's image leaf, never called an I1 breach |
| D3 | Reference provenance vocabulary | `provenance.skill_version` with the reference's `v` normalisation, `eval_infra.*`, and `compare.rs`'s exemption of `skill_version` and the planner from strata | reported; R01 §Run manifest names none of these paths |
| D4 | `planner.runtime.*` handed to M4 | M2b KT, "For M4" item 3 | reported; withdrawn by A2 |
| D5 | `tier` hard-coded in the core importer | `import/legacy.rs:77` `ENTRY_PARAMS = ["scale","text","tier"]`; `grading.rs:363-393` refuses an entry without it | verified; the M1 spec itself listed these params |
| G | I1 guard blind spots | scans only `candler-core/src`, `candler-harness/src`, `candler-schemas/schema`; literal substrings only; renames never touch `tier`; whole-word matching would miss `PlannerAxis` and `planner_package` (30 of 59 planner lines) | verified |

The leaf's A2 gate now covers all of A, with segment matching in place of whole words. One caveat holds: moving `stable.in`'s packages into the HTN experiment can change the lock bytes, and so the live HTN cell digests. The gate keeps the golden-import and M2a-fixture digests strict, and requires any live digest that moves to be listed in the re-validation report.

## B. Harness and environment content in the core: amendment A3, gates in M4 and M5

These are not HTN-domain content, but R01 puts them in the harness driver or a plug-in environment rather than in the core. No existing gate would catch them.

| id | What | Evidence | Owner and gate (A3, approved 2026-11-10; `01-gap_analysis_harness_env_v0.md`) |
|:--|:--|:--|:--|
| H1 | Claude Code names in core | `image.rs:25,35,854,895-902` runs its own `claude --version` rather than `HarnessDriver::version_probe`; `container/run.rs:29-38` `ANTHROPIC_*`; `mounts.rs:18` `CLAUDE_CONFIG_DIR`; `census.rs:33-36` parses Claude Code's `system/init` | M4: "`candler-core/src` names no harness; the image's harness version, the census and the gateway's env names come from the driver", plus a harness deny-list guard |
| H2 | Python venv assumed as the agent environment | `layout.rs:32-35` `/opt/env`, `VIRTUAL_ENV`, `uv pip compile --python-version 3.12`, `tree.rs:31` `pyvenv.cfg`, `python:3.12` base | M5: "the agent environment has a declared kind; the core holds no Python path, variable or marker outside that kind's implementation" |
| H3 | Binary finds its assets in the source tree | `crates/candler/src/commands/run.rs:112` `concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker")` | M5, or M3's packaging |

## C. Shortfalls that a small fix PR can close now

| id | Class | What | Evidence |
|:--|:--|:--|:--|
| W1 | breach | The catalog snapshot reader accepts a struct written as a JSON array, which R01 forbids (§Wire encodings: "never a struct written as an array"). Fix: use `MapOnly` as the other wire modules do, plus array-rejection tests. Schema files unchanged; patch bump on the schemas line. | **probe run**: `from_value::<CatalogSnapshot>(json!([..6 fields..]))` returned `Ok` |
| E1 | evidence gap | The venv and read-only-input exclusion moved from the driver into the core, and no test covers it. B23 tests `.claude/` only. | `runner/evidence/tree.rs:31`; `guard_claude_code_driver.rs:345-395` |
| E2 | gate on weaker evidence (M2b gate 2) | B36 recovers a killed job through a wiring the test builds itself. Neither production path (`ps.rs:88` `recover_job`, `jobs/runtime/session.rs:130` `recover_one`) is tested. | `supervision_gates_e2e.rs:280-446` |
| E3 | gate on weaker evidence (M2b gate 4) | `kill --all -y` never ran against real containers. | `guard_cli_kill.rs:86-94`; smoke v0 has the single-job case only |
| E4 | risk | A build outside a git checkout records no `eval_infra`, so `provenance_refused` refuses every stratum. Moved to M2c's A2 item, because it decides what a provenance path means. | `crates/candler/build.rs`; `compare.rs:335-362` |
| E5 | claim beyond evidence | "Imports losslessly": the oracle compares status, value, weight, cause and role. Evidence notes, treatment, snapshots and inputs are not compared. The literal gate is met. | `tests/oracle/mod.rs:21-38`; `grading.rs:507-529` |
| R | doc drift | R01 says "Nothing is implemented yet" (l.3). R01 is also wrong or silent on: <ul><li>how a legacy cell gets its digest (4 fields missing)</li><li>the frame `source` digest (it is of the `.gz` file)</li><li>the writable `/opt/env` volume</li><li>container child records</li><li>`Isolation.create`</li><li>the "Plan" naming ban</li><li>the `(?![\s\S])` pattern rule</li><li>which `provenance.*` paths gate strata</li><li>the catalog's `RelPath` base</li></ul>The README says the crates are empty. CONTRIBUTING names two schema roots (there are 17), pins release-plz 0.4.2 (CI uses 0.4.3), and has no rule for breaking changes while versions are 0.y.z. `supervision_gates_report_v0` still says the Linux half has not run. | reviewers' tables |
| K | records | The wire-schemas campaign leaves still show their gates as ⬜, with empty Progress tables, while the meta leaf marks all five ✅. Subjects over 72 characters keep recurring (049a9ab is 73), and nothing enforces the limit. | `__roadmap__/wire-schemas/` |

## D. Owned by an open milestone, no action now

| id | What | Owner |
|:--|:--|:--|
| F8 | The smoke scored 29 of 31; `provenance_fields` failed and was never re-run after the `candler-container@1` rename | M2c re-validation (already in its Step 1) |
| F9 | Gate 6 is decided only on synthetic arms; no comparability CLI exists | M4 (recorded in the KT) |
| O5 | R01's `candler run <experiment> [--<field> <value>]` against `--set PATH=VALUE` | M3 |
| O8 | Invariants with no labelled guard: I2, I4, I6 and I21 (M2a/M3); I8 ground truth (M2c/M5); I9 (M4); I13 (M7); the second clause of I14 | as listed |
| O9 | release-plz bumps by commit type, so a narrowing schema change typed `fix(schemas)` would ship as a patch | a CONTRIBUTING clause now; a schema-diff classifier later |
| O10 | The M5 leaf's goal ("runs can execute in a disposable container") is already done by M2b | M5 planning |

## Recorded deviations, checked and refuted as breaches

- **M1:**
  - the integrity source moved to `contamination_catalog.jsonl`;
  - a missing catalog counts as clean;
  - `api_error` maps to `agent_error`;
  - `StraySweeper` moved to M2b;
  - PDEATHSIG was dropped;
  - I17 and I11 were amended;
  - the store root falls back to XDG;
  - `.DS_Store` files are kept in the mini-corpus.
- **wire-schemas:**
  - the release job moved into `ci.yml`, with GitHub releases;
  - the known subject-length exceptions.
- **M2b:**
  - the RiVault cache;
  - `planner.runtime` absent (A59);
  - the macOS gap;
  - ZombieTick unguarded;
  - the Claude Code naming in core, planned in the architecture report, though it now goes to M4 as H1.

## Checked and met (summary)

- **wire-schemas:**
  - the outcome and catalog schema files are byte-identical to 0.1.0;
  - the outcome reader is map-only;
  - drift check, fixtures, cog and CI are in place;
  - the published schemas hold no domain words.
- **M1:**
  - all five gates: 240 runs and 10,320 rows checked by an independent oracle;
  - traces, integrity, supervision record and drift;
  - R01 was updated in the same merge as M2b's changes.
- **M2b:**
  - the Linux container e2e passed on the PR head, and the merged tree is that tree;
  - gate 3 (resume and forced deviations, B27/B28) and gate 5 (canary secret, `assert_clean` cannot pass on an empty scan);
  - the plan's decisions: docker CLI as the child, StopHook and DestroyDriver split, labels and sweeper, gateway design B, pinned base digest.
- **Cross-cutting:**
  - `cargo metadata` layering matches R01;
  - all six tags follow the bump rules.

## Needs execution (not run)

- **M1:** the golden import guard and the schema drift bless, both on CI.
- **M2b:** the e2e on macOS, which needs Docker; the bite test for E1 (remove `ENV_MARKER`, expect a green suite); the no-git build for E4.
- **Cross-cutting:** the planted `PlannerAxis` bite against whole-word matching; the `("tier","band")` rename bite; the lock diff for D1's digest caveat.

The fix PR or milestone that owns each item runs it, rather than this review.
