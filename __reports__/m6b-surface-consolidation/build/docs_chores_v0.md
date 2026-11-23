# Docs chores: build report (v0)

Leaf `docs_chores` of the M6b build. Docs only: no code, no test, no verifier.

## Changes

| File | Change | R16 source |
|:--|:--|:--|
| `__design__/architecture.md` | New `### Documents and surfaces` after the layer table: audiences table, offline rule, four consequences, box border | §Responsibility record (draft), Q4, Q4c, Q9 |
| `__design__/architecture.md` §Future features | Ten rows added: declared-versus-census check, bind-mount probe, Docker lock and `candler gc`, reported model and errata, check a plug-in and a suite without a job, glossary page, token sums, treatment layer in a plug-in, dependency-code twins; "Two counts" widened to "Two counts, a regrade gap and double grading". "schemas for CLI outputs" dropped from the C5 row | §Drafted R01 §Future features rows, X8 seam 6 |
| `__design__/architecture.md` §CLI surface | `resolve <experiment>` (was `<experiment \| suite>`); `analyze` reads the file `--experiment` names (was: finds it by digest) | §Seams 6 |
| `__design__/architecture.md` intro of §Future features | "M6a builds the paired tests" clause dropped; the Paired designs row already exists | X8, §Seams 6 |
| `__design__/architecture.md` §Analysis statistics | "stratum verdict" became "comparability verdict" (two places); §Readings names the field `interval_reading` | Q7, Q8 |
| `docs/statistics.md` | "comparable stratum" became "comparability class"; "stratum check" became "comparability check"; "Paired tests are not offered yet" became "the runs are not paired, so a paired test does not apply" | Q7, X8 |
| `README.md` | Rewritten as the store window, 74 lines; crate and verb inventory gone | X5, D2 |
| `python/candler-evaluator/README.md` | Author page: module, kind, `RunContext`, `KINDS`, `needs`, `params_schema`, one excerpt of `examples/synthetic_plugin/synthetic_plugin.py` (the file `engine_second.py:3` imports); "M2a fills it in" gone | X11, G16 |
| `docs/contract/evaluator-helper.md` §Surface | One sentence pointing to the docstrings and README | X11 |

Gates run: store window length 74; drift greps; record and ten rows; `contract_pages::` 7 of 7 pass; ownership check prints nothing.

## Quick start run

Built with `cargo build -p candler` (RUSTC_WRAPPER=sccache), then from the worktree root:

```
./target/debug/candler ls --format text crates/candler/tests/fixtures/observe/ls/experiments
```

It printed `a.toml` (2, 0), `notes.toml` as skipped with "experiment file holds a field it does not know", and `sub/b.toml` (1, 0), exit 0. The README says only that experiment files are listed and a non-experiment is skipped with its reason.

### Defect: no repository fixture resolves

R16 X5 and the leaf ask the quick start to run `candler resolve` on an experiment fixture of the repository. None resolves on the tip, with Docker up:
- `golden_design/experiment.toml`, `observe/ls/experiments/a.toml` and `sub/b.toml` fail `treatment_identity.error` ("the treatment names no source").
- An experiment with no `[treatment]` is refused `manifest.field_unset`.
- `--set treatment.source=...` is not accepted for that path.
- With a local git repo holding `.claude/skills/<name>` and `treatment.source` written into a copy of `b.toml`, the next refusal is `gateway.gateway_token` ("`model.endpoint` is not set", then "`model.token_secret` is not set").

So a resolve needs a treatment repository, an endpoint and a token secret: it cannot be "a command that spends nothing" on a bare fixture. I did not widen scope (no fixture is owned). The README quick start therefore builds and runs `ls` on the fixture directory (no Docker), and says in prose that `candler resolve` needs Docker and a real experiment naming a treatment repository and a model endpoint.

## Follow-ups

- Decide the quick start's `resolve` step: either add a self-contained fixture that resolves offline (treatment repository, endpoint, secret) or accept the `ls` quick start as written.
- R01 still says "stratum" in the comparability sense outside §Analysis statistics (about lines 918, 1160 to 1199, 1580, 1770). Q7 renames the sense; I limited the edit to the leaf's section. A sweep is a small docs change.
- Q8: neither R01 nor `docs/statistics.md` named the field `reading` anywhere, so there was no word to change. R01 §Readings now names `interval_reading`; the page does not name the field.
- The `analyze` row in §CLI surface says nothing of `analysis-report/1` or `--format`; that line is `output_contract`'s and `analysis_record`'s.
- Quick start check of `analyze` claim: `candler analyze --help` lists `--experiment`, consistent with the new row.
