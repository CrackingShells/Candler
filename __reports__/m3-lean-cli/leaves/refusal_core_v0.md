# refusal_core, leaf report v0

Codes for the refusals of `candler-core` and the harness crates, the `refusal/1` rendering under `--error-format json`, and three refusals of author input. A Sonnet implementer did the work in one round: 38 minutes and 365k tokens. Branch `m3/refusal_core` has 6 commits, head ca12ef5, merged as `merge(cli): land refusal_core`. Its verifier is `refusal_late`'s, which also covers this leaf.

```
ca12ef5 fix(cli)!: refuse a field path that no path class knows
fcfd313 fix(cli): refuse a factor over a setting before any image work
7d817c4 fix(cli)!: refuse a --set key under request that no setting names
df9ce2e feat(cli): print a refusal as refusal/1 under --error-format json
188e28e feat(cli): code the refusals of the core and the harnesses
57b9f75 docs(cli): register the codes of the core's refusals
```

## What landed
- **Codes:** 29 enums and 132 variants, in `<module>/refusal.rs` beside each enum for `resolve`, `experiment`, `runner`, `isolation`, `store`, `supervise` and `frames`, plus the two harness crates. `codes.toml` gains 592 lines with none deleted, including 51 `[internal]` rows.
  - `[internal]` rows are keyed by the qualified enum path plus `::Variant` and carry a `reason`. An enum with no author variant has one row with no `variant`.
  - `GatewayError::NotReady` delegates to `NotReadyCause`, so a 401 and a 429 get distinct codes. `ExecError::Manifest` delegates with side `candler`.
- **Rendering:** `--error-format json` prints one `refusal/1` line on stderr.
  - `CliError::Refused` and `Failed` are `[internal]` envelopes. `Failed` reports the code of the first coded refusal in its chain.
  - A new `CliError::Coded` carries a coded refusal and prints `refused: …`.
  - `CliError::with_pointer`, `with_file` and `led_by` keep the text form byte-identical.
- **Pointer narrowing:** the core's `PlanError` points at `/factors`. The binary narrows it to `/factors/<i>/path`, so `runner/plan.rs` stays unedited.

## Behaviour changes (approved in R10 §3 and by the coordination session's Step 6 ruling)
1. **`7d817c4`, `fix(cli)!`:** `--set request.<unknown>` is refused as `experiment.unknown_request_key`, exit 2.
   - These stay accepted: `request.harness`, `request.treatment`, `request.inputs` and paths below them.
   - `request.treatment_source` keeps its retired-setting message.
2. **`fcfd313`, `fix(cli)`:** a factor over a setting is refused right after the experiment file is read, before any docker call.
   - Its text changes from `candler: \`run\` failed: …` to `candler: refused: …`.
   - No `guard_cli*` row held the old text, and no digest moves.
3. **`ca12ef5`, `fix(cli)!`:** an unknown field path under `[fixed]`, in a factor or in `--set` is refused as `experiment.unknown_path`.
   - **Overlap with M4:** M4's `runner.mode` and `runner.idle_s` must be added to `experiment/paths.rs` and to the `LEAVES` array inside `is_known_path`. Whichever milestone merges second checks both, and checks that M4's S5 guard still passes.

## Step 6 probe
- **Refused at the base?** No. An unknown path was accepted with exit 0, and a job started. The path became a cell value that nothing read.
- **Where the path classes live:**
  - `experiment/paths.rs` holds string constants and no enumerating function.
  - `resolve/layers.rs::is_setting` matches by pattern.
  - `field_paths.toml` is a test oracle only.
- **Open subtrees:** `mcp` and `prompt.values`. `isolation.limits` is closed: `pids`, `memory` and `cpus` are its named members.
- **Classifier:** `experiment::paths::is_known_path(&FieldPath) -> bool`, one add-only hunk at the end of `paths.rs`. It accepts the listed leaves, `is_setting`, the typed roots and everything below them, the children of the open subtrees, and the ancestors of all these.
- **`[fixed]` pointers:** a key under `[fixed]` is one pointer segment (`/fixed/zzz.unknown_thing`), the quoted dotted-key form.

## Spec defects and open classification calls
- Step 1's "isolation 13 enums" is 10 in the code and in the tsv. The total, 29 enums and 132 variants, matches.
- Step 5's gate names a `guard_cli_run` row that does not exist. The refusal is covered by `guard_settings_out_of_cell` and by R3.
- Step 3's `[config]` section does not exist in the registry. The binary's rows are in `[cli]`, with codes `config.no_store`, `config.retired_setting`, `experiment.unknown_request_key` and `experiment.unknown_path`.
- `refusal_table!` cannot take a unit variant, so unit variants are written `X { .. }`. Fixing it would take a `candler-schemas` change.
- `SchemaTagError` is `pub(crate)`, so the chain walk cannot downcast to it. The other nine schemas types are walked.
- R2 covers five rows with two exceptions:
  - The missing-plug-in-directory row is pending (it asserts `candler.internal`).
  - The suite-params row needs a live uv plug-in environment, so it goes to `refusal_late`.
- R1's binary-crate test is a unit test, `commands::refusal::tests::r1_…`, so the gate's `-E 'test(/^r1_/)'` does not match it in `-p candler`. It runs in the full `-p candler` run.
- **For `refusal_late`'s verifier:**
  - `PlanError::StoredCellUnknown` is coded `runner.stored_cell_unknown`, side experiment, although the tsv does not list it.
  - `MountError::BadPath` is internal per the tsv, yet a project directory whose name holds a comma could cause it.
  - `StoreError::IndexCorrupt` and `Parse` are internal.
- `resolve.path_not_utf8` broke the code pattern with its digit, so it was renamed `resolve.path_not_unicode` before any release.

## Bites (each seen red, then reverted)
- **R1:** a new `thiserror` enum in neither list; `SecretError::Missing` given the fallback code and side `candler`; `ConfigError::Read` given side `candler`.
- **R2:** the path dropped; the side wrong; the record on stdout; two lines for one error; the text format printing the record; the experiment `file` dropped.
- **R3:**
  - the unknown request key accepted (red before the fix);
  - every key under `request` refused;
  - the refusal moved after `ensure_sidecar`;
  - the unknown-path check skipped for factors, and again for `--set`;
  - every path under `isolation` refused.

## Gates
All PASS on ca12ef5, with `--no-tests=fail`:
- OWNED; R1 (2 tests), R2 (5), R3 (3);
- G-codes (0 deletions against 8defdec); G-text (0 deletions against a26fe53);
- no wildcard arm in any `refusal.rs`; `candler-schemas` untouched;
- STD in its `-p` forms (269 passed, 19 skipped); LINUX; SUBJ; no `confirm_*`.

On the merged tip the coordinator ran G-codes against 8aad2c9 (1002 added, 0 deleted), G-text, G-digest, and STD plus LINUX in their workspace forms.

## Production
- 38 minutes (01:17–01:55) and 365k tokens, against an estimate of 90 minutes and 500–750k tokens.
- The implementer's own reckoning of "3 to 4 hours" was wrong. Its commit timestamps give the real figure.
- Peak target dir 3.6 GiB.
- One early `cargo` call ran without the environment and built a 4.4 GiB `target/` inside the worktree. It was deleted at once.
- The `rust-conventions` reviewer that AGENTS.md names was not run.
