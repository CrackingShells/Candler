# CLI surface

**Goal**: Every verb of the final CLI runs from an unrelated directory, every option's help says why it is not derived, the binary runs outside its source tree with its container assets embedded, every quick win names a passing test, and `show`, `diff` and `analyze` accept a bare evaluator version and infer the evaluator when only one exists.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] L2 is merged and the verbs are final: `! git grep -q -- '--plugin' -- crates/candler/src/commands/evaluate.rs && test -f crates/candler/src/commands/analyze.rs && test -f crates/candler/src/commands/ingest.rs && test -f crates/candler/src/commands/inspect.rs && test -f crates/candler/src/commands/resolve.rs && test -f crates/candler/src/commands/ls.rs && test -z "$(git grep -ln 'm3 scaffold' -- crates)"`
- [ ] [cmd] The scaffold's guard and the oracles are in: `test -f crates/candler/tests/guard_cli_surface.rs && git grep -qi 'pending' -- crates/candler/tests/guard_cli_surface.rs && test -f crates/candler/src/commands/preflight.rs && git grep -q 'fn asset_paths' -- crates/candler/src/commands/preflight.rs && test -f __roadmap__/m3-lean-cli/verbs.txt && test -f __roadmap__/m3-lean-cli/quickwins.tsv`
- [ ] [cmd] The evaluator choice of `analyze` is in the core and reusable: `git grep -q 'pub fn choose_evaluator' -- crates/candler-core/src/analysis/report.rs`
- [ ] [cmd] No Dockerfile needs a mode that `include_bytes!` would lose (each `COPY` of a script sets `--chmod`): `test "$(git grep -c 'COPY --chmod' -- assets/docker | awk -F: '{s+=$2}END{print s}')" -ge 2`
- [ ] [nl] Cargo commands run under the campaign README's build-footprint exports
- [ ] [nl] OWNED: `crates/candler/tests/guard_cli_surface.rs` (its pending lists emptied), `docs/contract/codes.toml` (`[cli]` rows of Step 6 only), the `--help` text (the doc comments and `#[arg(help = …)]` of the option fields) in `crates/candler/src/commands/*.rs`, `crates/candler/src/commands/assets.rs` (new) and its `mod` line in `crates/candler/src/commands/mod.rs`, `asset_paths` in `crates/candler/src/commands/preflight.rs`, the evaluator selection in `crates/candler/src/commands/{show,diff,common}.rs`, the root `README.md`. Never the Dockerfiles or `Cargo.toml`
**Success Gates**:
- ⬜ [cmd] OWNED: `test -z "$(git diff --name-only <base>..HEAD | grep -vE '^(crates/candler/tests/guard_cli_surface\.rs|crates/candler/src/commands/[a-z_]+\.rs|crates/candler/src/main\.rs|README\.md|docs/contract/codes\.toml)$')"`, `<base>` being the leaf's branch point
- ⬜ [cmd] U1 every verb from an unrelated directory, rows from clap's subcommand list: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u1_/)'`
- ⬜ [cmd] U2 every option's help ends with a stamp: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u2_/)'`
- ⬜ [cmd] U3 the copied binary runs `run` against the fake world: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u3_/)'`
- ⬜ [cmd] U4 every quick win names a passing test (G-quickwins): `cargo nextest run --workspace --no-tests=fail -E "$(cut -f2 __roadmap__/m3-lean-cli/quickwins.tsv | sed 's/.*/test(=&)/' | paste -sd'|' -)"`, and all ten names exist: `test -z "$(cut -f2 __roadmap__/m3-lean-cli/quickwins.tsv | while read -r n; do cargo nextest list --workspace --message-format oneline -E "test(=$n)" 2>/dev/null | grep -q . || echo "missing: $n"; done)"`
- ⬜ [cmd] U5 a bare version and an inferred evaluator in `show` and `diff`: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u5_/)'`
- ⬜ [cmd] The pending lists are gone: `! git grep -qi 'pending' -- crates/candler/tests/guard_cli_surface.rs`, and no source path of the binary names its build tree: `! git grep -n 'CARGO_MANIFEST_DIR' -- ':(glob)crates/candler/src/**'`
- ⬜ [cmd] G-verbs: `cargo build -p candler && diff <(target/debug/candler --help | awk '/^Commands:/{f=1;next} f&&NF{print $1}' | grep -v '^help$' | sort) <(sort __roadmap__/m3-lean-cli/verbs.txt)` (with `CARGO_TARGET_DIR` as the footprint exports set it: use that directory's `debug/candler`), and `! git grep -n 'm3 scaffold' -- crates`
- ⬜ [cmd] The README lists every verb of `verbs.txt` and no verb that is not there: `test -z "$(while read -r v; do grep -qF "\`$v\`" README.md || echo "missing: $v"; done < __roadmap__/m3-lean-cli/verbs.txt)"` and `! grep -q '`import`' README.md`
- ⬜ [cmd] G-text holds: `git diff --numstat a26fe53 -- 'crates/candler/tests/guard_cli*.rs' | awk '$2!=0{b=1}END{exit b}'` (the file `guard_cli_surface.rs` is new, so its lines are additions; `evaluate`'s expected outputs are the stated exception, R10 §3)
- ⬜ [cmd] G-digest holds: `git diff --numstat a26fe53 -- crates/*/tests/oracle crates/*/tests/vectors ':(exclude)crates/candler-core/tests/vectors/stats' ':(exclude)crates/candler-core/tests/vectors/platform_inventory.toml' | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] The platform guard still holds, the new file needs no unix-only API: `cargo nextest run -p candler-core --no-tests=fail -E 'test(/^q1_/)'`
- ⬜ [cmd] Each guard function has its three doc lines: `f=crates/candler/tests/guard_cli_surface.rs; test "$(grep -c '^/// Bite:' $f)" -ge "$(grep -c '^fn u[1-5]_' $f)" && test "$(grep -c '^/// Oracle:' $f)" -ge "$(grep -c '^fn u[1-5]_' $f)" && test "$(grep -c '^/// Guards:' $f)" -ge "$(grep -c '^fn u[1-5]_' $f)"`
- ⬜ [cmd] STD (the `-p candler` forms: `cargo fmt --check && cargo clippy -p candler --all-targets -- -D warnings && cargo nextest run -p candler && RUSTDOCFLAGS="-D warnings" cargo doc -p candler --no-deps`), LINUX and SUBJ pass
- ⬜ [nl] The leaf report records the bites:
  - U1: a path joined to the project directory or to the binary's directory, in a verb with a relative argument; a new verb with no row;
  - U2: an option with no stamp; a stamp that is not at the end of the help;
  - U3: assets found through `CARGO_MANIFEST_DIR` (the recorded build context sits under the source tree);
  - U5: the `name@` prefix still required.
- ⬜ [nl] Each option's stamp tells the truth (U2 checks only that one exists): the leaf report lists every option with its stamp, for Eliott to read at the level briefing
- ⬜ [nl] No behaviour change of R10 §3 applies beyond those already ruled. If a verb refused an input before this leaf and accepts it now (a bare version, an inferred evaluator, `show --rates` with no `--evaluator`), the leaf report says so as a relaxation
**References**: [R01 §CLI surface over the jobs, §Operation API, §Configuration](../../../../../__design__/architecture.md); [R10 §4 (`cli_surface`), §5 guards U1–U5 and the quick-win map](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md); [R04 CLI defaults audit (the ten quick wins)](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion/__reports__/candler_core/02-findings_cli_defaults_v0.md); [`analyze` leaf](../../analyze.md); campaign README §Gotchas

Checked while drafting:
- `asset_paths` is at `commands/run.rs:114` at a26fe53 and finds `assets/docker` through `concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker")`; the scaffold moves it, unchanged, to `commands/preflight.rs`. It builds `ImageAssets { base_context, env_context, store_root }` and a gateway context. `DockerImageService` stages its own build contexts from the asset sources and tags images by content digest, so the same bytes under another path move no image tag.
- `assets/docker` holds 11 files (44 KB): `base/{Dockerfile,.dockerignore}`, `env/{Dockerfile,.dockerignore}`, `gateway/{Dockerfile,.dockerignore,entrypoint.sh,gw.js,nginx.conf.template}`, `testagent/{Dockerfile,testagent.sh}`. Every Dockerfile sets modes with `COPY --chmod`, so an embedded copy needs no unix permission call and no inventory row.
- The binary has no `sha2` dependency and `Cargo.toml` is not this leaf's: the cache key is made with `candler_core::import::digest_of`.
- `ConfigEnv` knows `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `HOME`, not a cache home, and `resolve/config.rs` is not this leaf's: the cache is under the store root, which `asset_paths(store_root)` already receives (proposal, see the report).
- `show` declares `--rates` with `requires = "evaluator"`; `diff` takes `--from` and `--to` as `id@version` text parsed by `NameVersion`. Both read `common::read_selection`.
- `guard_cli.rs` seeds a store by hand and runs `show` and `diff`; `support/cli_run.rs` builds the fake-docker `World` (a PATH fake, a skills repo, an experiment file) that `U1` and `U3` use.

## Step 1: Embed the assets, run from anywhere
**Goal**: The binary carries its container assets and writes them to a digest-keyed cache, so it runs from a copy outside the source tree.
**Implementation Logic**:
1. Write the `u3_` guards first and see them fail: `u3_a_copied_binary_runs_run`: copy `env!("CARGO_BIN_EXE_candler")` into `<tmp>/elsewhere/candler` (a temp directory outside the source tree), set up the fake-docker `World` as `guard_cli_run.rs` does, run the copy's `run` from an unrelated working directory with the fake `docker` first on `PATH`, and assert: the job id is printed, and the build context recorded in the fake's call log for each `docker build` is a directory that exists, lies under `<store root>/assets/<digest>/docker/`, and is not under the source tree (`env!("CARGO_MANIFEST_DIR")`); `u3_the_embedded_assets_equal_the_tree`: every file under `assets/docker` of the workspace is in the embedded table with equal bytes, and no embedded file is missing from the tree (a unit test in the guard file, reading the tree from `CARGO_MANIFEST_DIR`, the only use of that variable: it is test code). Oracle: the tree and the fake's call log. Seen red against `asset_paths` as it is.
2. `commands/assets.rs`: `const ASSETS: [(&str, &[u8]); 11]`, one `include_bytes!` per file with the path relative to `assets/docker`, written with `concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/…")` (`include_bytes!` reads at compile time, so this is the one place the build tree is named); `pub fn digest() -> Result<Digest, CliError>`, the `digest_of` of the lines `<relative path>\0<digest_of the bytes>\n` in path order; `pub fn materialise(store_root: &Path) -> Result<PathBuf, CliError>`: with `dir = <store_root>/assets/<digest hex>`, return `dir` when `dir/docker` exists; otherwise write every file into a hidden temporary directory beside `dir`, create parent directories as needed, and rename it onto `dir` (a rename that finds `dir` present is a lost race and is not an error); a half-written temporary directory is never `dir`.
3. `preflight.rs`'s `asset_paths(store_root)` returns the contexts below `materialise(store_root)?.join("docker")`, and every use of it takes the `Result`. No `env!("CARGO_MANIFEST_DIR")` remains in the binary's `src` outside `assets.rs`. The unix-only inventory is unaffected: `assets.rs` uses `std::fs` only.
4. The write is idempotent and cheap: a second `run` finds `dir/docker` and writes nothing.
**Deliverables**: `crates/candler/src/commands/assets.rs`: `ASSETS`, `digest`, `materialise`; `crates/candler/src/commands/mod.rs`: `mod assets;`; `crates/candler/src/commands/preflight.rs`: `asset_paths`; `crates/candler/tests/guard_cli_surface.rs`: `u3_a_copied_binary_runs_run`, `u3_the_embedded_assets_equal_the_tree`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u3_/) | binary(guard_cli_run)'` (expected: PASS)
**Commit**: `feat(cli): embed the image assets and cache them by digest`

## Step 2: A bare version and an inferred evaluator
**Goal**: `show` and `diff` take `1.2.0` for `name@1.2.0` when the store holds one evaluator name with that version, and `show --rates` infers the evaluator when the store holds only one.
**Implementation Logic**:
1. Write the `u5_` guards first (the store seeded as `guard_cli.rs` seeds one, two evaluator versions of one name) and see them fail:
   - `u5_diff_takes_bare_versions`: with one evaluator name `ev` holding `1.0.0` and `2.0.0`, `diff --from 1.0.0 --to 2.0.0 --format json` equals the output of `--from ev@1.0.0 --to ev@2.0.0`; a bare version held by two names is refused, naming both;
   - `u5_show_rates_infers_the_only_evaluator`: `show --rates --format toml` with no `--evaluator` on a store with one evaluator equals the output with it named; with two evaluators it is refused, listing them;
   - `u5_show_takes_a_bare_version`: `show --evaluator 1.0.0 --format json`.
2. `commands/common.rs`: `pub fn evaluator_of(selection: &Selection, flag: &'static str, requested: Option<&str>) -> Result<NameVersion, CliError>`, which calls `candler_core::analysis::report::choose_evaluator(selection, requested)` and maps its refusal to `CliError::refused_by` with the flag named in the message. `show` and `diff` call it. `show`'s `--evaluator` stays optional for the outcomes listing (every evaluator when absent: unchanged); `--rates` loses `requires = "evaluator"` and, when no evaluator is given, calls `evaluator_of(.., None)`.
3. The text and JSON of every existing output are unchanged (G-text): only inputs that were refused are now accepted.
4. `analyze` already takes a bare version (its guard A3); this step only checks that it goes through the same function: `git grep -q 'choose_evaluator' -- crates/candler/src/commands/analyze.rs crates/candler-core/src/analysis/report.rs`.
**Deliverables**: `crates/candler/src/commands/common.rs`: `evaluator_of`; `crates/candler/src/commands/show.rs`, `crates/candler/src/commands/diff.rs`; `crates/candler/tests/guard_cli_surface.rs`: `u5_diff_takes_bare_versions`, `u5_show_rates_infers_the_only_evaluator`, `u5_show_takes_a_bare_version`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u5_/) | binary(guard_cli)'` (expected: PASS)
**Commit**: `feat(cli): infer the evaluator and take a bare version`

## Step 3: Stamp every option
**Goal**: The help of every option ends with "cannot be derived: …" or "not an experiment setting: …", so a reader learns why the value is asked for.
**Implementation Logic**:
1. Read `guard_cli_surface.rs`: its option guard (`u2_`) reads each verb's `--help` and joins an option's continuation lines before it checks the ending, and its pending list names the options still without a stamp. Keep its parsing and the names of the guards it already has (the names below are the ones to use where it has none); the verbs `run`, `evaluate`, `show`, `diff`, `kill`, `watch`, `ingest`, `analyze`, `inspect`, `resolve`, `ls` and the global options `--store` and `--error-format` are all in its walk. `--help`, `--version` and positional arguments are outside it.
2. For every option without a stamp, edit its doc comment or `help` so that its last sentence is the stamp and the reason, one line. Two stamps only, written exactly: `cannot be derived: <what makes it impossible to derive>.` and `not an experiment setting: <what it controls instead>.` A stamp is true or it is not written: if an option can be derived, derive it instead of stamping it, and report that as a relaxation (an option that stops being required is a behaviour change to report, not to make silently).
3. Delete each entry of the pending list as its option gets its stamp; delete the list when it is empty. `--error-format` keeps the wording R10 D3 gives: "cannot be derived: what parses the caller's stderr is not visible from the terminal; cannot live in the experiment file: a refusal can come before any file is read".
4. List every option and its stamp in the leaf report.
**Deliverables**: `crates/candler/src/commands/{run,evaluate,show,diff,kill,watch,ingest,analyze,inspect,resolve,ls}.rs`: option doc comments; `crates/candler/src/main.rs`: the global options' doc comments; `crates/candler/tests/guard_cli_surface.rs`: `u2_every_option_help_ends_with_a_stamp` (its pending list for options emptied)
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u2_/)'` (expected: PASS)
**Commit**: `feat(cli): state why each option is not derived`

## Step 4: Every verb from an unrelated directory
**Goal**: The cwd matrix has a row for every verb clap lists, each run from a directory unrelated to the project and the binary, and its pending list is empty.
**Implementation Logic**:
1. Keep the scaffold's matrix: it takes its rows' verbs from the subcommand list in `candler --help` (the `Commands:` block, minus `help`), so a verb without a row fails (`u1_every_verb_has_a_row`). Give each verb in the pending list its row, run by the real binary with the working directory set to a fresh temp directory that holds the fixture files, every path argument relative to it (I14), and `HOME` and the XDG variables pointed into the temp directory. Minimum rows:

   | Verb | Arguments (relative to the working directory) | Asserts |
   |---|---|---|
   | `run` | `run exp.toml`, the fake-docker world | a job id on stdout; the experiment was read from the working directory |
   | `resolve` | `resolve exp.toml` | every value with its source; no job directory appears |
   | `ls` | `ls`, with `experiments/a.toml` below the working directory | `a.toml` is listed |
   | `ps` | `ps` | exit 0, the job just started is listed |
   | `kill` | `kill --all --yes` on a store with no job | exit 0 |
   | `watch` | `watch <id>` of an ended job | the event lines, ending with `job.ended` |
   | `ingest` | `ingest traces/` | runs created below the store; none outside it |
   | `inspect` | `inspect store/runs/<key>` | the run, with no store option |
   | `evaluate` | `evaluate suite.toml`, the fixture plug-in under `plugins.dirs` | a job id on stderr and the report |
   | `show` | `show --format json` | the counts |
   | `diff` | `diff --from 1.0.0 --to 2.0.0 --format json` | the report |
   | `analyze` | `analyze --experiment exp.toml --format csv` | the arms table |

   - The two store rows (quick win 6): with a `candler.toml` in the working directory and no `--store`, the store is `<working directory>/.candler/store`; with no project, it is below `$XDG_STATE_HOME`; neither writes below the repository or the binary's directory.
2. The bite is a path joined to the project root or to the binary's directory in one verb, which makes that verb's row fail; the other bite is a verb added to clap with no row.
3. Delete the pending list for verbs and the word from the file.
**Deliverables**: `crates/candler/tests/guard_cli_surface.rs`: `u1_every_verb_has_a_row`, `u1_each_verb_runs_from_an_unrelated_directory`, `u1_the_store_comes_from_the_project_or_the_state_home`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^u1_/)'` (expected: PASS)
**Commit**: `test(cli): run every verb from an unrelated directory`

## Step 5: The README's verb list
**Goal**: The README names the verbs the binary has.
**Implementation Logic**:
1. In `README.md`, the bullet for `candler` lists the verbs of `__roadmap__/m3-lean-cli/verbs.txt` (excluding the hidden `supervise`), each in backticks, replacing `import` by `ingest`, and nothing else on that line moves. If the bullet for `candler-core` names `import` as a module description ("the legacy importer"), it stays: that is the library, not the verb.
2. No other file changes.
**Deliverables**: `README.md`
**Consistency Checks**: `test -z "$(while read -r v; do grep -qF "\`$v\`" README.md || echo "missing: $v"; done < __roadmap__/m3-lean-cli/verbs.txt)"` (expected: PASS)
**Commit**: `docs(readme): list the verbs of the lean cli`

## Step 6: Every caller mistake carries a code
**Goal**: A refusal that the caller causes in a verb's own checks is printed under `--error-format json` with a code and a side other than `candler` (refusal_late verifier, finding F4, 2026-11-12).
**Implementation Logic**:
1. The verbs that print `candler.internal` for the caller's own mistake:
   - `resolve` and `run` of a missing experiment file (`commands/resolve.rs:182`);
   - `kill` without `-y`;
   - `kill -y` of a missing job, where the first failing job's error must be carried as the source, not only printed;
   - `ps` with an unreadable job, likewise;
   - `inspect` of an unknown or ambiguous selector;
   - `show --evaluator` and `diff --from` with an unknown evaluator.

   Give each a `CliRefusal` row in `commands/refusal.rs` and `codes.toml` `[cli]`, with side `config` unless the cause is the environment, and `file` where a file is at fault.
2. Clap's usage errors (an invalid `--set` path, an unknown option) are printed as one `refusal/1` record under `--error-format json`. Use the code `cli.usage_invalid` and side `config`, and keep clap's text as the message.
3. Write the guards first, under `--error-format json`, one row per site, and see each red. Bite: a site back to `CliError::refused(String)`.
**Deliverables**: the coded refusals above; their rows in `codes.toml` `[cli]`; guard rows in `crates/candler/tests/guard_cli_surface.rs`
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'binary(guard_cli_surface)'` (expected: PASS)
**Commit**: `fix(cli): code the refusals a caller causes in each verb`
