# Round 02 candidates v0 (read-only investigation)

Source: the HTN lane's gaps file (`__reports__/m6b-surface-consolidation/02-input_htn_run2_gaps_v0.md`).
Code paths are under the worktree root. Store paths are under
`.../htn-6-run2-b5b5dd/.claude/skills/htn-creator-workspace/results/phase6-run2-candler/candler-store`
(still there; I read it only). Run ids: `a16b0809eea3e2aef776c80a68db620db5aec0d0ae3f189a25d7cfb871a1c37d-r2-a1`,
`...-r4-a1`; `be1123c55050d8359a07786a17f9ded01837da002654ce5df734c67f3f36f6fc-r4-a1`.

Call cap was passed by a few calls. Items "not read" are listed at the end.

## Q4c start-command gate

**Facts**
- The image is built inside the resolve preflight, not at job start. `ImageIdStep.run` calls
  `images.ensure(...)` and derives `isolation.env_lock`, `isolation.image` and the `inputs` root
  (`crates/candler-core/src/resolve/steps/image.rs:98-159`). `ensure_image` builds the base, the env
  layer and the optional extension layer (`crates/candler-core/src/isolation/image.rs:347-424`).
- `candler resolve` runs the same foreground preflight as `run` (`crates/candler/src/commands/resolve.rs:248-267`,
  `run.rs:174-197`). So a gate "right after the image is built" would run at `resolve` too, and build an image there.
- The server files are experiment or plug-in inputs. They are digested and put in the store at
  `run.rs:169` (`declare_inputs`), before the image step. They are not in the image.
- The read-only mounts are applied per run, later: `mount_inputs` makes an empty mount point in the
  workspace and bind-mounts each input blob read-only (`crates/candler-core/src/runner/exec/mod.rs:146-190`).
  At image-build time the mounts are known only as digests in `inputs`. Nothing applies them yet.
- The server's relative paths (`servers/bio-opentrons/x.py`) resolve against the run's workdir
  (recorded `.mcp.json`, store `raw/resolved.jsonl.gz`; the same inputs mount at `<workdir>/<input.path>`).
- Existing one-off container helper: `DockerCli::control`, used by `CommandCatalogProvider.run` with a
  timeout and an output limit, in the built image, with a record dir
  (`crates/candler-core/src/runner/evidence/catalog.rs:252-300`). I did not read `control` itself,
  so whether it can take mounts, an open stdin or env is not read.
- A declared server's `env` is a free map. `mcp` is a free subtree in the surface
  (`crates/candler-core/src/resolve/layers.rs` `SUBTREES`, `is_known_path`). The Claude Code
  harness writes it to `.mcp.json`; OpenCode requires `env` to be text values (`prepare.rs:293-355`, `text_map`).
  Values are recorded as comparable values in the manifest. Secrets are declared by name only; their
  values are never written (`crates/candler-schemas/schema/experiment.schema.json:288`).
- The two Candler harnesses read the servers through one place (`crates/candler-harness/src/mcp.rs:1-17`, `servers`).

**Reading**: feasible but not where the proposal puts it. Risks:
- The mounts are not available at image build. The gate would need to rebuild the per-run mount list
  (inputs, workspace, workdir) in a throwaway container. That is a second mount path to keep equal to `mount_inputs`.
- A gate in the image step runs in `resolve` and in every job start, even when the image is cached.
- A server that stays up with stdin open is the normal case, so a short wait only catches fast crashes.
  A server that does slow work at import (network, model load) would pass; one that exits at EOF or
  prints help would be a false positive if stdin were not held open.
- Server `env` may hold run-time or secret values. A throwaway container would either miss them
  (false failures) or receive secrets it would not need.
- Network and egress settings must match the run, or imports that reach out behave differently.
- Cost: one container per stdio server (four in the HTN run) per job start.
- A better home is the run, just after `mount_inputs`, so mounts and env are real. That is a design choice, not decided here.

## N1 plug-in metadata (`plan.prompt_domain_name`, `plan.config`)

**Facts**
- Lookup is exact path match over `manifest.comparable().values()`; absent gives `MissingMetadata`
  "is not a value of the run manifest" (`crates/candler-core/src/evaluation/context/assemble.rs:143-158`).
- The needs shape is `Needs { artifacts (globs), inputs (kinds), metadata (paths), snapshots }`
  (`python/candler-evaluator/.../_models/evaluator_describe_result.py:88-110`; core uses `needs.metadata`,
  `needs.inputs`, `needs.artifacts`, `needs.snapshots`, assemble.rs:109-170). Snapshots are refused as unsupported (assemble.rs:110-114).
- What a kind receives: `RunContext { root, artifacts, metadata, scratch }`
  (`python/candler-evaluator/src/candler_evaluator/plugin.py:90-121`). Artifacts are the sealed workspace files
  of the run, filtered by the needs globs (assemble.rs:121-141). Inputs are stored blobs by kind (assemble.rs:160-163).
- The run's `prompt.values` map is one recorded value at `prompt.values` (HTN lane's report; the surface
  treats `prompt.values` as a free subtree, `layers.rs` `is_known_path`). A need for `prompt.values` as a whole path
  would match; a need for `plan.prompt_domain_name` cannot.
- Is `plan.*` a Candler namespace? I found no `plan.` path in `is_known_path` LEAVES or SUBTREES
  (`layers.rs:60-122`: leaves are `model.*`, `tools.*`, `isolation.*`, `budget.per_run.*`, `prompt.*`, `runner.*`,
  `store.root`, `plugins.dirs`, `slots.count`, plus subtrees `mcp`, `prompt.values`, `harness`, `treatment`, `inputs`,
  `snapshots`, `environments`). The word "plan" in Candler code means job/run planning (`runner/plan.rs`), not an experiment part.
  R01's "never called a plan" and the layer text were not re-read by me (R01 §Layer responsibilities, `docs/contract/` pages: not read beyond the search hits).
- Core avoids naming domains: `ImportRules` doc says "the core names no domain"
  (`crates/candler-core/src/import/rules.rs:93-117`).

**Reading**: plug-in design choice, with a real Candler gap behind it only if the lane wants a declared
metadata surface. `plan.*` is the HTN plug-in's own word. The honest fix on the plug-in side: take the
domain name and config from what the run already holds. The prompt template and `prompt.values` are in the
manifest, and the workspace files are in `artifacts`. The kind can read the domain file in the workspace
and the prompt values (declare `metadata = ["prompt.values"]`, `artifacts = [...]`). A Candler-side metadata
surface for arbitrary paths would add a namespace the core does not name today. Confidence: medium (plug-in contract pages not read in full).

**Suggested verdict**: owner:plugin (plug-in reads `prompt.values` and workspace itself). Optional: defer a
"declared metadata" surface to a later round.

## N2 `--set replicates=1` ignored silently

**Facts**
- `replicates` is a top-level file field (`crates/candler-schemas/src/execution.rs:1318-1322`), required (`docs/contract/experiment-file.md:44`).
- It is treated as an operational setting path, so `--set replicates=1` passes the known-path check:
  `is_setting` returns true for `replicates` (`crates/candler-core/src/resolve/layers.rs:27-58`), and
  `check_set_paths` accepts any `is_known_path` (`crates/candler/src/commands/preflight.rs:253-266`, `187-196`).
- `request_settings` passes `--set` values as explicit values into the resolution (preflight.rs:268-315). The plan
  reads the count from the file only: `run_order(per_cell.len(), file.replicates(), ...)` (`crates/candler-core/src/runner/plan.rs:1168`).
  I found no other reader of `replicates` under `commands/` or `runner/`.

**Reading**: Candler defect. A key is accepted as known and then never read. Cause: the path is in the settings list but the plan uses the file.

**Suggested verdict**: accept. Either apply the explicit `replicates` in `plan_runs` or refuse it with a message that says it is a file field. About 2 Sonnet calls (code and test).

## N3 turn cap

**Facts**
- Candler passes the cap as the harness flag and has no meter of its own: `RunCaps::flags` pushes `--max-turns <n>` (`crates/candler-core/src/experiment/budget.rs:35-49`).
  The recorded argv of `a16b0809...-r2-a1` and `be1123c5...-r4-a1` shows `--max-turns 200` (`raw/resolved.jsonl.gz`, frame `argv`, `env: []`). The `a16b0809...-r4-a1` manifest holds `budget.per_run.turns = 200`
  (`comparable.values`); I did not print its argv.
- Stop reason is read from Claude Code's final `result` line: subtype `error_max_turns` becomes a turns cap (`crates/candler-harness-claude-code/src/sessions.rs:16-43`).
- `a16b0809...-r2-a1`: `run.ended_by = {kind: cap, cap: turns}`, `run.num_turns = 201`, result line `subtype error_max_turns`, `terminal_reason max_turns`, `is_error true`.
- `a16b0809...-r4-a1`: `run.ended_by = completed`, `run.num_turns = 250`, result line `subtype success`, `terminal_reason completed`. Claude Code itself reported 250 and no error, with the same `--max-turns 200` argument in the comparable values.
- Assistant message frames in the two runs are 452 and 456, so neither number equals the frame count.

**Reading**: harness behaviour (Claude Code's own counting and enforcement), not a Candler defect. Candler records what the harness reports. The cap of 200 is a flag, and the result line says what counted. Why 250 passed the cap is Claude Code's, unknown from here (possible: sub-agent or skill turns counted differently). Candler could state in the docs that `budget.per_run.turns` is the harness's `--max-turns` and `run.num_turns` is the harness's count.

**Suggested verdict**: owner:consumer (report to the harness side). Optional small doc note: accept, 1 Sonnet call.

## N4 `[treatment]` required even when the treatment is a factor

**Facts**
- `TreatmentIdentityStep` derives `treatment` from the name and the commit a tag resolves to in the checkout its `source` names. "A treatment that names no source is refused" (`crates/candler-core/src/resolve/steps/identity.rs:133-139`, doc comment).
- A treatment factor's levels fill fields `name`, `tag`, `commit`, `source` (`crates/candler-core/src/runner/plan.rs:354-355`, `TREATMENT_FIELDS`). `run.rs:176-197` resolves each declared level separately via `input_for_level`;
  `check_factors` admits `request.treatment` as the one setting factor (preflight.rs:152-169).
- Whether the file itself requires a `[treatment]` table when a factor exists: I did not find the check. `RawExperimentFile` takes `treatment` (execution.rs:1099-1133) and the preflight inserts `file.treatment()` as the base
  (preflight.rs:268-315, `values.insert(REQUEST_TREATMENT, json_of(file.treatment()))`). The refusal text for "no source" is from the step above, not read in full.

**Reading**: partly confirmed. The identity step refuses a treatment with no source. The base `[treatment]` seems to be the default that each level overrides; a factor-only form looks not supported. Not fully read. Likely by design (a level fills fields onto a base).

**Suggested verdict**: defer (confirm in a second pass whether a factor-only treatment can be allowed). If accepted: about 3 Sonnet calls.

## N5 `treatment:` refused in a plug-in's `isolation.build.context`

**Facts**
- The experiment's own `isolation.build.context` supports `treatment:<path>`; `context_dir` strips the prefix and extracts `<path>` of the treatment checkout at the pinned commit
  (`crates/candler-core/src/resolve/steps/image.rs:286-300`).
- For a plug-in fragment, `location_texts` returns a string value whole as a path and skips only list entries that start with `treatment:` (`crates/candler-core/src/resolve/steps/plugin_environment.rs:310-322`: "the strings of a list that are not `treatment:` sources").
  So a `treatment:` string in a plug-in's `context` is handled as a path, which matches the lane's report that only list values accept it.
- I did not find a stated reason in the code I read. I did not read the plug-in environment docs for a reason.

**Reading**: confirmed at the code. Reason: not read, so unknown (deliberate or omission). Likely an omission of the string case, since the same prefix is understood for the experiment's context and for list sources.

**Suggested verdict**: defer until the reason is read in `docs/contract/` and the plug-in environment tests. If accept: about 2 Sonnet calls.

## N6 leading `/` prompt passes resolve, refused at run time

**Facts**
- I did not find the run-time check. Searches for a leading-slash rule in `candler-core`, `candler-harness`, `candler-harness-claude-code` found only unrelated path checks
  (`crates/candler-harness/src/conformance.rs:433` checks `text.starts_with('/')` for paths, not prompts). The refusal may sit in the driver argv builder or the capability checks; not read.
- `candler resolve` stops after the preflight steps; it never calls `plan_job` or the driver's plan for a cell (`resolve.rs:1-14`). A driver-side check at cell planning would therefore not run in `resolve`.

**Reading**: unknown for the exact check. Structurally likely: the check is in the driver's cell planning, which `resolve` skips, so resolve could see the same value but does not call that check.

**Suggested verdict**: defer (find the check first; if it is a pure function of `prompt.template`, accept by calling it in `resolve`, about 2 Sonnet calls).

## N8 `CANDLER_STORE` and `resolve`

**Facts**
- `--store` is a global clap option with `env = "CANDLER_STORE"` (`crates/candler/src/main.rs:37`). The `Resolve` arm passes `cli.store.as_deref()` (main.rs:109).
- `resolve::run` calls `common::store_root("resolve", store)` (`crates/candler/src/commands/resolve.rs:243`), which layers `explicit` into `store.root` through `explicit_path` and reads it back with `resolved_root`
  (`crates/candler/src/commands/common.rs:70-116`). So `resolve` reads the store root and honours `CANDLER_STORE`.
- The printed resolution is made later from `request_settings(...)` (resolve.rs:245, 267), whose input never includes the store root as an explicit value
  (preflight.rs:268-315). My reading: the `store.root` line that `resolve` prints would show the config or core default as its source, not `explicit_argument`. I did not run it.

**Reading**: `CANDLER_STORE` is honoured for where the foreground services point (`foreground(..., &root, ...)`, resolve.rs:248). The printed table very likely does not show it, which would look like it was ignored. Unverified by running.

**Suggested verdict**: accept as a display fix only if confirmed by a run. About 2 Sonnet calls (add the explicit store value to the printed resolution, plus a test). Otherwise reject.

## N9a stored workspace keeps 0-byte placeholders for mounted server files

**Facts**
- At run start, `mount_inputs` creates each input's mount point as an empty file in the workspace (`OpenOptions...create_new(true)`) "so the engine does not make it as root", then bind-mounts the blob read-only on top
  (`crates/candler-core/src/runner/exec/mod.rs:146-190`).
- The sealed workspace is read from the host directory, so it sees the empty placeholder, not the mounted bytes. Checked in the store: four 0-byte files under
  `runs/a16b0809...-r2-a1/workspace/servers/bio-opentrons/` (`gripper_server.py`, `module_server.py`, `liquid_server.py`, `movement_server.py`).
- The bytes are not lost to the store: inputs are content-addressed blobs (`rt.store.layout().blob(&input.digest)`), and the manifest records them under `comparable.inputs` (key seen in the manifest: `comparable` has `values, harness, treatment, snapshots, inputs`).

**Reading**: by design as a side effect (store content-addressing and the mount trick), not an omission. The recorded run is re-runnable from the manifest inputs, not from the workspace tree alone. The lane's workaround (copy from the plug-in) is not needed if the input digests are used. Whether the tree should mark placeholders as mounts: not decided anywhere I read.

**Suggested verdict**: reject as a defect; optional doc note (1 Sonnet call). Could be reopened as a feature: record the mount list in the tree.

## N9b environment variables for the harness process

**Facts**
- The experiment schema has no env table. The only environment entry is the list of secret names, "the names of the environment variables the runs need; their values are never written"
  (`crates/candler-schemas/schema/experiment.schema.json:288`; name pattern at :193).
- The surface has no `env` leaf in `is_known_path` (`layers.rs:60-122`); the free subtrees are `mcp`, `prompt.values`, `harness`, `treatment`, `inputs`, `snapshots`, `environments`. `mcp.servers.<key>.env` is the server's env only, not the harness.
- The recorded resolved frame for `be1123c5...-r4-a1` shows `"env": []` and the argv above with no output-token flag. The run ended `agent_error` / `api_error` with the text "response exceeded the 32000 output token maximum", which names `CLAUDE_CODE_MAX_OUTPUT_TOKENS`. `harness.maxOutputTokens` was 32000 in the result frame.
- The Claude Code driver builds argv and files with `env: []` for this cell (store `raw/resolved.jsonl.gz`). I did not read `crates/candler-harness-claude-code/src/driver.rs` env building line by line beyond its module doc (line 4: it "owns the environment and the secrets of the agent").
- `candler.toml` or the settings table: not read.

**Reading**: no supported way found to set a harness env var from an experiment, plug-in or config. A possible workaround through a declared secret name is unverified (whether secret values reach the harness env verbatim; and it would be recorded as a secret, not a setting). This is a real Candler gap for a tuning knob that changes results.

**Suggested verdict**: accept as a decision for the supervisor (a recorded `harness.env`-like surface touches comparability: it must enter the cell digest). Size: about 6 to 8 Sonnet calls (schema, surface path, driver, manifest record, docs, test). If the supervisor prefers not to grow the surface: defer.

## Not read, left for a second pass
- `DockerCli::control` (what it can mount, hold stdin on, and set as env).
- The run-time check for a leading `/` in the prompt (N6).
- The file-level rule that a `[treatment]` table is required when `request.treatment` is a factor (N4); the stated reason for N5.
- `docs/contract/` plug-in pages and R01 §Layer responsibilities in full (N1); the claim that `plan.*` is absent comes from the surface list, not from those pages.
- Claude Code driver env building and the settings table (N9b).
- `a16b0809...-r4-a1` recorded argv (read only its manifest cap).
