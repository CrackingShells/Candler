# one_reader (Step 1, Commit B): leaf report v0

Commit B types the MCP servers an experiment declares, in `candler-schemas`, as its own generated
schema. It touches only `crates/candler-schemas/`. Steps 2-4 follow below.

## First reads

- `candler_harness::mcp::servers` (`crates/candler-harness/src/mcp.rs:77`) reads `mcp.servers` as a
  map (`cell.table_at`), strips the optional `name` leaf and re-keys by rendered name
  (`by_name`, `mcp.rs:39`).
- opencode's `server()` (`crates/candler-harness-opencode/src/prepare.rs:295`) translates one entry
  into OpenCode v2's shape. The fields it reads: an optional `codemode` (default true), an optional
  `type` marker, and the two disjoint families — a launched command (`command`, `args`, `env`) or a
  remote URL (`url`, `headers`). `LOCAL_KEYS` and `REMOTE_KEYS` (`prepare.rs:276-277`) name the
  allowed keys; any other key is refused.
- The experiment schema (`crates/candler-schemas/src/execution.rs`, `RawExperimentFile`) types
  `fixed` as `BTreeMap<FieldPath, JsonValue>` (rendered through `patternProperties`) and each factor
  `level` as the generic `Level`. Every concrete value under either is the same "any JSON value
  except null".

## B: how the type is published

**Choice: publish the server type as its own generated schema** (`mcp-server.schema.json`), not in
the experiment schema.

**Why:** the experiment schema cannot type the one path `mcp.servers` in place. Under `fixed`, keys
are dotted field paths but *every* key maps to the same generic `JsonValue` schema via
`patternProperties`; there is no mechanism to give `mcp.servers` a schema different from any other
`fixed` path. A factor level is the same generic `Level`. So a server declared under `fixed` or in a
factor level can only sit under a schema that is permissive about the server object's own fields.
The typed server therefore lives at its own root, and A1's single reader applies it to both places,
as §D5's plan allows.

`candler_schemas::servers` defines `McpServer` (the full key set the drivers read, `deny_unknown_fields`)
and `McpServerKind` (the `type` markers: `stdio`, `local`, `http`, `sse`, `remote`). `command` is an
`Option<String>` (`servers.rs:66-67`): a server whose `command` is not text fails this schema, but
the schema does not require a `command` or a `url`; that is the reader's job (A1). It is
registered as the root `mcp-server` in `generate.rs` and committed under
`crates/candler-schemas/schema/mcp-server.schema.json`.

**Base vs tip for the observation** (`guard_mcp_server` tests): the committed experiment schema is
unchanged by B and still validates a full experiment whose `fixed."mcp.servers"` holds a server with
a non-text `command` (a number). The published `mcp-server` schema refuses that same entry. On the
base, `mcp-server.schema.json` does not exist, so the malformed entry had no typed reader at all.
Independent review re-ran the committed guard on a fresh compile: on the base state `the_mcp_server_schema_refuses_a_non_text_command` and `the_mcp_server_schema_accepts_what_a_driver_writes` fail (schema file missing, guard_mcp_server.rs:29); on the tip 209/209 pass; emptying `properties.command` turns the refuse test red.

## Gates

| Gate | Verdict | Evidence |
|---|---|---|
| Leaf pre-condition: `feat/integrity` is an ancestor of HEAD | PASS | `git merge-base --is-ancestor feat/integrity HEAD` succeeds |
| Base observation: experiment schema accepts a non-string `command`; no typed schema exists | PASS (observed on base) | scratch `one_reader-1` at `origin/feat/integrity`: `BASE ACCEPTS: malformed mcp.servers entry (non-string command) validates against experiment.schema.json`; `mcp-server.schema.json exists on base: False` |
| Tip observation: the typed schema refuses the non-string `command` | PASS | `guard_mcp_server::the_mcp_server_schema_refuses_a_non_text_command` (and `the_experiment_schema_cannot_type_the_mcp_servers_path`, `the_mcp_server_schema_accepts_what_a_driver_writes`) |
| Schema drift check (blessed, then without the variable) | PASS | `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift` → 6 passed; then `cargo nextest run -p candler-schemas -E 'test(schema_drift)'` → `committed_schemas_have_no_drift` PASS, `every_committed_schema_file_has_a_root` PASS |
| Workspace clippy | PASS | `cargo clippy --workspace --all-targets -- -D warnings` → `Finished` with no warnings |
| Workspace nextest | PASS | `cargo nextest run --workspace` → `743 tests run: 743 passed (6 slow), 32 skipped` |
| Workspace doc | PASS | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` → `Finished` |
| `cargo fmt --check` | PASS | clean after `cargo fmt` |
| Commit touches only `crates/candler-schemas/` | PASS (pending `git diff --name-only HEAD~1 \| grep -v '^crates/candler-schemas/'` prints nothing) | all changed paths are under `crates/candler-schemas/` |
| Schemas-release gate (CI) | PASS | the commit is `feat(schemas)!` and touches no other crate under `crates/` (CONTRIBUTING.md:147-182) |

## Notes

- One test, `guard_protocol_client::every_failure_path_of_an_evaluator_is_accounted_for`
  (`candler-core`), failed once during a full parallel workspace run and passed immediately on
  re-run, both on the base (scratch `one_reader-2`, three runs) and on this tip's full suite
  (743/743). It is parallel-execution contention in an orthogonal path; no other crate uses the new
  type, so B cannot regress it.
- The typed entry is deliberately a permissive-but-typed struct (every field the drivers read,
  `deny_unknown_fields`); the local/remote dichotomy and the `command`-or-`url` requirement are the
  reader's job in A1, which is the next step's work, not B's.

## A1: one reader of MCP servers (commit `feat(resolve)!: read MCP servers in one place and refuse a malformed one`; the leaf names the scope `harness`, which `cog.toml` does not allow)

Preceded by `fix(schemas): refuse an unknown key in an MCP server entry` (serde `deny_unknown_fields` on
`McpServer`; guard `guard_mcp_server::serde_refuses_an_unknown_key_in_a_server_entry`; schema drift
unchanged, 210/210 in `candler-schemas`).

`candler_harness::mcp::servers` returns `Option<BTreeMap<String, McpServer>>` and is the only reader of
`mcp.servers`. It refuses, with `experiment.cell_value_invalid` and the path `mcp.servers.<key>` in the
message: a field of the wrong type, an unknown key (the type refuses it), an entry with neither
`command` nor `url`, and both families at once. OpenCode's `server()` only translates a typed server;
Claude Code's `render_mcp` serialises it; `stdio_servers` and `Stdio` are gone (the start check
filters the typed servers by `command`). `candler resolve` calls the new
`runner::plan::check_cells(file, resolution, driver)`, which builds every cell, factor levels
included, and has the driver check each one.

### Absolute path in args (audit-2)
`run` refuses it at cell build: `Comparable::from_resolved` calls `holds_absolute_path`
(`crates/candler-schemas/src/execution.rs:2214`, predicate `is_absolute_path` at `:2224`), code
`experiment.absolute_path`. `resolve` did not build cells. `check_cells` makes it build them, so the
refusal comes from the same predicate; the reader has no absolute-path check of its own.

### Gates
- PASS `guard_silent_acceptance::silent_cases_are_refused_with_a_code_and_a_pointer`: rows for command
  not text, args not text, neither command nor url, env value not text.
- PASS `guard_cli_observe::o1_resolve_and_run_refuse_a_malformed_mcp_server_with_one_record`: same
  code and message from `resolve` and `run`.
- PASS `guard_cli_observe::o1_resolve_refuses_what_a_cell_holds`: absolute path in args
  (`experiment.absolute_path`) and a malformed server in a factor level
  (`experiment.cell_value_invalid`), both from `resolve`.
- Base red: scratch worktree `integrity-scratch/one_reader-a1` at `b9310a95` (B and the schema fix) with
  these test files copied in, target dir `integrity-target/one_reader-a1`, line
  `Compiling candler-harness v0.15.0 (.../integrity-scratch/one_reader-a1/crates/candler-harness)`:
  `3 tests run: 0 passed, 3 failed`. Worktree, branch and target dir removed.
- fmt, clippy `-D warnings` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: PASS.
- `cargo nextest run --workspace`: `746 tests run: 746 passed (8 slow), 32 skipped`.

## Breaking changes (approved)
- Claude Code now refuses an unknown key, non-text `args`/`env` values, and a server with neither
  `command` nor `url`.
- OpenCode now accepts a `url` server without `type`, read as remote.
- The library lost `stdio_servers` and `Stdio`; `mcp::servers` returns typed servers.

## Multi-level resolve guard (commit `fix(resolve): check each cell against its own level`)

Risk A1 left untested: `check_cells` received one resolution, so `candler resolve` on an experiment
whose `request.treatment` factor has several levels could fail with "a treatment level has no
resolution".
- Guard: `guard_cli_observe::o1_resolve_accepts_an_experiment_with_two_treatment_levels` (two table
  levels, tags `v1` and `v2` of two commits of the skills repository, `resolve` must exit 0).
- RED on the A1 tip (`51a4dc9d`): exit 2, `experiment.document_invalid` at `/factors`, "the treatment
  level {...\"tag\":\"v2\"} has no resolution". A1 had broken valid experiments.
- Fix: `resolve` resolves each declared level with `input_for_level` (moved from `run.rs` to
  `preflight.rs`, shared by both verbs) and hands `check_cells` one resolution per level, as `run` does.
- GREEN after the fix: the guard, and the 24 tests of `guard_cli_observe` and `guard_cli_run`.

## A2: one reader of replicates, and the silent inputs left (commit `feat(resolve)!: read replicates once and refuse the silent inputs left`; the leaf's subject is 78 characters and `cog verify` allows 72, so `in one place` became `once`)

- `Levels::replicates` (`runner/plan.rs`) returns `Result<u32, PlanError>`. A `replicates` setting that is
  no count, zero, or beyond 32 bits is `PlanError::Replicates` (the code and pointer of the schema
  validator that the CLI's `check_replicate_count` raised: `experiment.count_zero` at `/replicates`). A
  `[fixed]` count that differs from the file's own `replicates` is `PlanError::ReplicatesConflict`,
  code `experiment.replicates_conflict`, pointer `/replicates`. A setting that came from `--set` still
  wins over the file's count; with no setting the file's own count is the count.
- `check_cells` calls it, so `resolve` refuses what `plan_runs` refuses, with the same record.
- `check_replicate_count` and its two calls in the CLI preflight are gone; its unit test moved to the
  guard rows below.
- `model.token_secret` is read from every layer (file, explicit, project and user configuration, core
  defaults) before the `env` table is checked (`resolve/layers.rs`).
- The leaf's other silent cases (non-text server `command`, `args`, `env`) were closed by A1.
- The three per-case codes of PR #57 stay: their refusals sit in other readers (`ResolveError` for the
  path and not-integer cases, `EnvRefusal` for the env names), whose messages name what to fix.
- Registry: `experiment.replicates_conflict` added to `docs/contract/codes.toml`; the contract page
  regenerated (`CANDLER_BLESS_CONTRACT=1`).
- Un-ignored: `deferred_silent_cases_are_refused_with_a_code_and_a_pointer` and `DEFERRED` are gone; the
  `replicates_conflict` row sits in `ROWS` (now 11 rows). No `#[ignore]` with an OWNED reason is left.

### Gates (A2)
- PASS `guard_silent_acceptance::silent_cases_are_refused_with_a_code_and_a_pointer` (rows: token_secret
  from a core default, `replicates_conflict`, count zero). Base red: scratch `one_reader-a2` at
  `1a1d7433` with the test files copied in, target dir `one_reader-a2`, line `Compiling candler-core v0.15.0
  (.../integrity-scratch/one_reader-a2/crates/candler-core)`: `2 tests run: 0 passed, 2 failed`; the rows read
  `got None` for all three (the library fell back to the file's count).
- PASS `guard_cli_observe::o1_resolve_refuses_a_replicate_count_the_plan_refuses` (same base run, red).
- PASS `guard_refusal_codes` (a case for `PlanError::Replicates`; the registry row).
- fmt, clippy `-D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`: PASS.
- `cargo nextest run --workspace`: `747 tests run: 747 passed (6 slow), 31 skipped`.

## Who produces the state (lead review, for seam review 1)

Three places in this leaf where a test or a reader does not take its state from the one producer:

1. **The two-level resolve guard failed on CI** (`guard_cli_observe::o1_resolve_accepts_an_experiment_with_two_treatment_levels`, panic at `guard_cli_observe.rs:254`, "Author identity unknown"). The cause is the git identity, not the fixture: the guard ran `git commit` itself and inherited the machine's identity, while every other commit in the world goes through `support::cli_run`'s `git`, which pins the author and blanks the global and system config. Locally it passed because macOS git guesses an identity from the host. Red reproduced with `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1` and `user.useConfigOnly=true`: `1 test run: 0 passed, 1 failed`, same panic line. Fix: `World::tag_new_commit` makes the commit and tag through the fixture's `git`; under the same environment `guard_cli_observe` and `guard_cli_run` give `25 tests run: 25 passed`. The experiment shape is one a user writes: two tagged versions of one skill from one source, as two `request.treatment` levels; the test writes that experiment file itself (a consumer input) and the skills repository comes from the world's fixture.
2. **`check_replicates_input`** (`crates/candler-core/src/resolve/layers.rs:389-409`) still reads `replicates` itself, with its own codes (`ReplicatesPath` and a not-an-integer variant), next to `Levels::replicates`. The leaf names only the CLI's `check_replicate_count` for deletion, so this is not required by the leaf, but "one reader" is not strictly met for `replicates`, and a non-integer and a zero count get codes from two readers.
3. **`at_plan_fixed`** in `guard_silent_acceptance.rs` builds the resolution from `Facts::base()` and edits `resolved["settings"]` by hand; the settings come from the real `Layered`, the resolution around them does not.

## HTN migration note

For the HTN lane. This repository has no external users; this is a note, not a shim.

Experiment files now refused, each by `resolve` and `run` with one record:
| input | refusal |
|---|---|
| an `mcp.servers` entry with a `command`, `args` or `env` value that is not text, an unknown key, neither `command` nor `url`, or both families | `experiment.cell_value_invalid`, naming `mcp.servers.<key>` |
| an absolute path in the `args` of a server | `experiment.absolute_path` (`run` refused it before; `resolve` now does) |
| `[fixed] replicates` that differs from the file's own `replicates` | `experiment.replicates_conflict` at `/replicates` |
| `replicates` of 0 or beyond 32 bits, from `--set` or `[fixed]` | `experiment.count_zero` or `experiment.cost_not_positive` at `/replicates`, now also from the library |
| an `env` name equal to a `model.token_secret` that a configuration layer or a default sets | `experiment.env_token_secret` |

Library break: `candler_harness::mcp::stdio_servers` and `Stdio` are gone, and
`candler_harness::mcp::servers` returns `Option<BTreeMap<String, McpServer>>` (`candler_schemas::servers`).
`PlanError` gained `Replicates` and `ReplicatesConflict` (the enum is `non_exhaustive`).
`candler resolve` on a file with several `request.treatment` levels now resolves each level.
The HTN golden design (`guard_golden_design`) passes unchanged.

## Bites
- Lenient reader restored (scratch `one_reader-b1`, `mcp::servers` drops an entry that fails to read; target
  dir `one_reader-b1`, line `Compiling candler-harness v0.15.0 (.../integrity-scratch/one_reader-b1/crates/candler-harness)`):
  `3 tests run: 0 passed, 3 failed` (`silent_cases_are_refused_with_a_code_and_a_pointer`,
  `o1_resolve_and_run_refuse_a_malformed_mcp_server_with_one_record`, `o1_resolve_refuses_what_a_cell_holds`).
- Replicate fallback restored (scratch `one_reader-b2`; `check_replicates(..).unwrap_or(file count)` and the
  conflict test disabled; line `Compiling candler-core v0.15.0 (.../integrity-scratch/one_reader-b2/crates/candler-core)`):
  `2 tests run: 0 passed, 2 failed` (`silent_cases_are_refused_with_a_code_and_a_pointer`,
  `o1_resolve_refuses_a_replicate_count_the_plan_refuses`).
- Scratch worktrees, branches and target dirs removed.

## Breaking changes (approved)
- Claude Code now refuses an unknown key, non-text `args`/`env` values, and a server with neither
  `command` nor `url`.
- OpenCode now accepts a `url` server without `type`, read as remote.
- The library lost `stdio_servers` and `Stdio`; `mcp::servers` returns typed servers.

## Breaking changes (from A2, not yet decided)
- `[fixed] replicates` that differs from the file's own `replicates` is refused
  (`experiment.replicates_conflict`); it was silently read as the `[fixed]` value.
- `Levels::replicates` and `plan_runs` refuse an invalid count where the library fell back to the file's.
- `resolve` now refuses an invalid count and, with a `request.treatment` factor, resolves every level.
- An `env` name equal to a `model.token_secret` that a core default or a configuration layer sets is refused.

## Verifier
A fresh Sonnet verifier read the leaf, this report and the diff of `crates/` (no build).
- New types and modules: only `McpServer`, `McpServerKind` (with their module `candler_schemas::servers`)
  and the two `PlanError` variants. No trait, no wrapper. `Stdio` and `stdio_servers` are gone;
  `check_replicate_count` is gone.
- Readers: `candler_harness::mcp::servers` is the one reader of `mcp.servers`
  (`cell.table_at(&field_path!("mcp.servers"))`, `mcp.rs:118`); `Levels::replicates` is the one count reader.
- New function with a call hop: `check_cells` (two steps: the count, then the cells), the one entry `resolve` uses.
  `mcp_servers(cell)` in each driver's `prepare.rs` forwards to `mcp::servers`; they pre-date this leaf.
- Residuals, not changed here: `check_replicates_input` (`resolve/layers.rs`) is a strict second read of
  `replicates` with its own codes (`ReplicatesPath` and a not-an-integer variant); a non-text `model.token_secret`
  yields no name for the env check, so the check is skipped (the gateway step and the session refuse it later).

## Left

Nothing of A2 or step 4. `container-e2e` runs on the pull request.
