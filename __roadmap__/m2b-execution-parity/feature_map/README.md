# Feature map oracle (task featuremap)

What today's sandbox guarantees, so the container runner can be checked against it.

## Provenance
- Reference: `htn-creator-eval@0.4.2`, commit `28be2c158fbc6ee786a789242c2d8fd47bfde9a8`, repo `PCfVW/mcp-python-ingestion` (read-only, via `git show`).
- Read in full or in large part: `SANDBOX_REFERENCE.md`, `_confine_hook.py`, `suite.py`, `_env_build.py`, `_manifest.py`, `_provenance.py`, `_run_config.py`, `_paths.py`, `_run_dirs.py`.
- Grep only: `_run_config_expand.py`, `_git_skill.py`, `_dispatch.py`.
- Not read: any Candler `crates/`, `__design__/`, `__roadmap__/`, nor the plan's design decisions. No container was created.

## Files
- `feature_map.json`: rows `{id, source, feature, reference_mechanism, observable, check}`; null `check` rows carry `why_unchecked`. Ids: `S<n>` per contract `##` header, `S3.R<n>` per invariant I<n> (the contract's only numbered rules), `X<n>` for code-enforced guarantees the contract does not state.
- `checks/`: 31 scripts; env inputs INSPECT_JSON, EXEC, WORKSPACE_DIR, HOME_DIR, STREAM_JSONL, PROVENANCE_JSON, CANARY, plus RUN_DIR (sealed run dir, R01 §Run store), JOB_DIR (job dir: events.jsonl, job.json, children/), STORE_DIR (store root). Details, interface in `checks/README.md`.
- `reference_flags.txt`: the claude command line. `provenance_fields.txt`: the ten provenance fields.
- `ids_from_contract.sh`: ids derived mechanically from the contract. Gate: `LC_ALL=C comm -23 <(sh ids_from_contract.sh) <(jq -r '.[].id' feature_map.json | LC_ALL=C sort)` prints nothing.

## Dispositions
Every row has `disposition`: `check` (script), `smoke_step` (by hand; step in `smoke_step`), `guard:<Bnn>` (Rust guard; ids in `guards`), `intended_deviation` (`deviation` gives old -> new), `not_applicable` (`why_na`). A row may also carry a supplementary `check` next to a guard or deviation owner. Rows without a script carry `why_unchecked`. Design text read for the store layout: R01 §Run store, §Jobs, runs and supervision, §Ends, §Experiment file (no Candler code).

## Reference defects found (not guarantees)
Resume drops provenance (X24); hook covers only Bash/Write/Edit/Read (X13); full host env passes to the agent (X40); contract header says "v3" while the contract is v5.
