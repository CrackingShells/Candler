# checks/

POSIX sh. Exit 0 = property holds; one-line message on stdout either way. Mechanism-neutral: they see a run only through these env vars.

| var | meaning |
|---|---|
| INSPECT_JSON | path to `docker inspect` JSON of the agent container (or equivalent config dump) |
| EXEC | command prefix that runs a command inside the agent environment, cwd = the agent's start directory. Used unquoted: `$EXEC pwd -P` |
| WORKSPACE_DIR | host path of the agent work dir (post-run state, or pre-run for provisioning checks) |
| HOME_DIR | host path of the agent's HOME |
| STREAM_JSONL | the agent's stream-json output |
| PROVENANCE_JSON | the run's provenance block |
| CANARY | random token value that was given to the agent as its auth token |
| RUN_DIR | the sealed run directory of one run, laid out per R01 §Run store: `manifest.json`, `run.done`, `raw/<stream>.jsonl.gz`, `trace.jsonl`, `tree.json` |
| JOB_DIR | the job directory: `job.json`, `events.jsonl`, `children/` |
| STORE_DIR | the store root: `store.json`, `runs/<run key>/`, `jobs/`, for multi-run and resume rows |

Store-side checks (RUN_DIR / JOB_DIR / STORE_DIR): run_sealed, stderr_kept, run_dir_per_key, resume_runs, manifest_fields, manifest_comparable, run_measured, no_abs_paths, tree_inventory, integrity_verdict, job_events; canary_absent also scans them. They use only names R01 states. Assumptions: `manifest.run` is an object with `cell`, `replicate`, `attempt`; `tree.json` is a path -> sha256 map (optionally under `files`); with WORKSPACE_DIR set, tree_inventory also rejects files under any dir carrying pyvenv.cfg. Extra tools: gzip, mktemp.

Dependencies: sh, jq, find, grep, awk.

Timing: provisioning checks (venv_*, layout, planner_pin, no_reference_answers, manifest_outside, env_scrubbed, host_isolation, non_root, benign_devices) need EXEC to reflect the pre-run state. Post-run checks (workspace_no_pyvenv, no_bytecode, canary_absent, stream_*, skill_discovery, settings_sources_isolated, outside_path_denied) use WORKSPACE_DIR, HOME_DIR, STREAM_JSONL.

Assumptions: the Claude Code init event carries cwd, permissionMode, model, mcp_servers, plugins, slash_commands, skills. outside_path_denied passes vacuously when the agent never tried an outside path. venv_contents fails on an environment without the five deps installed (expected).
