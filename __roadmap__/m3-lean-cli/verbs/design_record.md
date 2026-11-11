# Design record

**Goal**: R01 (`__design__/architecture.md`) records decisions D1 to D10 of M3, the evaluator slot pool, the final CLI surface, the refusal identity, the analysis statistics and the workspace targets, so that the design says what the code does; it writes no code.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] R01 still holds the provisional text this leaf replaces, and none of the new sections: `git grep -q 'Provisional:\*.*experiments/' -- __design__/architecture.md && ! git grep -q 'refusal/1' -- __design__/architecture.md && ! git grep -q '^### Analysis statistics' -- __design__/architecture.md && ! git grep -q '^### Workspace targets' -- __design__/architecture.md`
- [ ] [cmd] The platform inventory it cites is in: `test -f crates/candler-core/tests/vectors/platform_inventory.toml`
- [ ] [nl] OWNED: `__design__/architecture.md`, except the part of §Process supervision that `stray_sweep_tick` owns (the line on the orphan's start, line 408 at a26fe53) and the rest of that section, which this leaf does not touch. No other file. The milestone that merges to `main` second rebases its own R01 edits
**Success Gates**:
- ⬜ [cmd] OWNED: `test "$(git diff --name-only <base>..HEAD)" = "__design__/architecture.md"`, `<base>` being the leaf's branch point
- ⬜ [cmd] §Process supervision is untouched: `diff <(git show <base>:__design__/architecture.md | sed -n '/^### Process supervision/,/^### Agent environment and secrets/p') <(sed -n '/^### Process supervision/,/^### Agent environment and secrets/p' __design__/architecture.md)` prints nothing
- ⬜ [cmd] The four removals hold: `! git grep -q 'Provisional:\*.*experiments/' -- __design__/architecture.md && ! git grep -q 'Provisional:\*.*plug-in' -- __design__/architecture.md && ! git grep -q 'where .ls. looks' -- __design__/architecture.md && ! git grep -qF 'candler import <corpus>' -- __design__/architecture.md`
- ⬜ [cmd] Each decision's key term is in its section (the style of M2c's G6d): `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^[1-5]|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"`: it reads the 56 rows of the `terms` block below, each `step|heading|term`, and prints each term that its section lacks
- ⬜ [nl] After L2 merges, the coordinator greps each name the new text gives (`candler_schemas::refusal::Refusal`, `x-candler-class`, `slots.evaluators`, `CANDLER_BLESS_CONTRACT`, `--error-format`, `candler_core::refusal::catalog()`) in `crates/`, `docs/` and the leaves, and every one is found where the text says; a name the code does not hold yet at this leaf's merge is listed in the leaf report
- ⬜ [cmd] No code, no schema and no roadmap file moved: `test "$(git diff --name-only <base>..HEAD -- crates python docs __roadmap__ | wc -l)" -eq 0`
- ⬜ [cmd] The document keeps its shape: `test "$(grep -c '^## ' __design__/architecture.md)" -eq "$(git show <base>:__design__/architecture.md | grep -c '^## ')"` (no level-2 section is added or removed)
- ⬜ [cmd] SUBJ passes
- ⬜ [nl] Each paragraph this leaf adds states the decision as the code will do it and gives its reason in a sentence; none refers to a plan, a milestone number or a roadmap (AGENTS.md: the design says what is), except §Workspace targets, which says "after M7" because R01's milestone list already uses it
**References**: [R01 the sections named in Steps 1 to 5](../../../__design__/architecture.md); [R10 §1 (the I22 text, owned by `stray_sweep_tick`), §2 D1–D10 in full, §5 guards C1–C4 and D1–D4](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md); [M2c `README.md` gate G6d](../../m2c-wire-alignment/README.md); campaign README §Gotchas (Owned files)

Checked while drafting: R01's headings at a26fe53 are `### Configuration` (line 176), `### Budget` (192), `### CLI surface over the jobs` (263), `### Job` (288), `### Process supervision` (339), `### Error model` (1019), `## Deliberately open` (1159), `### Workspace` (117, under `## Proposed State`) and `## Implementation` (1171, which ends with the Tooling paragraph); `### Analysis statistics`, `### Refusals`, `### Contract pages`, `### Workspace targets` and `### Landing order` do not exist. The Operation API table already lists `ingest`, `analyze` and `show`/`inspect`; the `ingest` row says "yes (job) for big corpora" and the CLI table gives `candler import`; `watch` already reads "`--json` (default)". The section sampler `sec()` stops at the next heading of level 2 or 3, so a `####` line stays inside its section.

The key terms, one row per `step|heading|term`; the gates read this block, so a term is added here first:

```terms
1|### Refusals|refusal/1
1|### Refusals|<surface>.<reason>
1|### Refusals|docs/contract/codes.toml
1|### Refusals|append-only
1|### Refusals|candler.internal
1|### Refusals|JSON pointer
1|### Refusals|candler_schemas::refusal::Refusal
1|### Refusals|serde_path_to_error
1|### Refusals|`plugin`
1|### Refusals|`suite`
1|### Refusals|`experiment`
1|### Refusals|`config`
1|### Refusals|`environment`
1|### Contract pages|x-candler-class
1|### Contract pages|CANDLER_BLESS_CONTRACT
1|### Contract pages|contract_pages
1|### Contract pages|catalog()
1|### Contract pages|driver-traits.md
1|### Contract pages|[helper]
1|### CLI surface over the jobs|--error-format
2|### Job|JobSpecFile
2|### Job|evaluation.outcome
2|### Budget|slots.evaluators
2|### Budget|CPU count
2|### Configuration|slots.evaluators
2|### Configuration|plugins.dirs
2|### Plug-in manifest (static; on the `schemas@` line)|plugins.dirs
3|### CLI surface over the jobs|candler ingest
3|### CLI surface over the jobs|--rules
3|### CLI surface over the jobs|alias
3|### CLI surface over the jobs|interrupt
3|### CLI surface over the jobs|candler resolve
3|### CLI surface over the jobs|candler inspect
3|### CLI surface over the jobs|candler analyze
3|### CLI surface over the jobs|candler evaluate <suite>
3|### CLI surface over the jobs|experiments/
3|### CLI surface over the jobs|skipped
3|### CLI surface over the jobs|--live
3|### CLI surface over the jobs|NO_COLOR
3|### CLI surface over the jobs|include_bytes!
3|## Deliberately open|`ls`
4|### Analysis statistics|Wilson
4|### Analysis statistics|Fisher
4|### Analysis statistics|Newcombe
4|### Analysis statistics|Holm
4|### Analysis statistics|weighted
4|### Analysis statistics|core_default
4|### Analysis statistics|docs/statistics.md
5|### Workspace targets|WSL2
5|### Workspace targets|platform_inventory.toml
5|### Workspace targets|compile_error!
5|### Workspace targets|after M7
5|### Workspace targets|Linux
5|### Workspace targets|macOS
5|### Landing order|scaffold
5|### Landing order|orphan
```

## Step 1: Refusals and contract pages
**Goal**: R01 records D2 to D5: the refusal identity, the machine-readable form, the field classes and the generated pages.
**Implementation Logic**:
1. New `### Refusals`, right after `### Error model`, before `## Golden corpus (M2a exit gate)`, so it stays inside `## Contracts & Invariants`. It holds: a refusal is an input the caller can fix; each has a stable **code** `<surface>.<reason>` (for example `manifest.unknown_field`, `suite.params_invalid`), a **side** among `plugin`, `suite`, `experiment`, `config`, `environment` and `candler` (whose input is at fault; `candler` never for an author's error), and a **path**, an RFC 6901 JSON pointer into the parsed document or a file. The record is `refusal/1` `{schema, code, side, path?, file?, message, causes[]}` and the trait `candler_schemas::refusal::Refusal` (`code`, `side`, `path`). The registry is `docs/contract/codes.toml`, hand-written and append-only: a released code is never renamed or removed, so it is the oracle of the tests that read the code. Every other error that reaches the CLI carries the fallback `candler.internal`, side `candler`, and is listed in the registry's `[internal]` allowlist. Parse errors get their pointer through `serde_path_to_error`. Exit codes stay 0, 1 and 2.
2. New `### Contract pages`, right after it: the pages of `docs/contract/` tell a coding agent what each surface expects. Their field classes come from the committed schema files: a property in `required` is `required`; one with a default or optional is `optional`; only `candler` and `free` are declared, as the annotation keyword `x-candler-class`. The pages are generated by the drift test `contract_pages` (regenerated with `CANDLER_BLESS_CONTRACT=1`), which reads the committed schema files, never the Rust types; the code-side catalogue is `candler_core::refusal::catalog()`; the helper's codes are the `[helper]` section of `codes.toml`. The page of the driver traits (`driver-traits.md`) is written by hand by the session-runner milestone and folded into the generator by whichever milestone merges to `main` second; until then the generator lists it in the index and leaves its file alone.
3. In `### CLI surface over the jobs` (Step 3 owns the table): the global option `--error-format text|json`, default `text`; `json` writes one `refusal/1` record per line on stderr. Its help text gives the two reasons it is not derived and not a file setting (what parses the caller's stderr is not visible from the terminal; a refusal can come before any file is read). Write the sentence here in Step 1, the table row in Step 3.
**Deliverables**: `__design__/architecture.md`: `### Refusals`, `### Contract pages`, the `--error-format` sentence of `### CLI surface over the jobs`
**Consistency Checks**: `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^1|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"` (expected: PASS)
**Commit**: `docs(design): record the refusal identity and contract pages`

## Step 2: Evaluate as a job, and the slot pool
**Goal**: R01 records D6: `evaluate` is a supervised job the command waits for, evaluator processes have their own machine-wide pool, and `plugins.dirs` is read.
**Implementation Logic**:
1. `### Job`: an evaluate job is a job like a run job: its record, lock, children, subreaper and sweep are the same, and its events add `evaluation.outcome` per committed record. `JobSpec` stays the run spec; the job directory holds a `JobSpecFile` (`Run` or `Evaluate`), read and written at the job-directory boundary only. The evaluate body has its own serve path with no container.
2. `### Budget`: the machine-wide slot paragraph splits in two pools, because an agent run holds its slot for minutes or hours while an evaluator is a short, CPU-bound process: agent processes take slot files of the agent pool (`slots.count`); evaluator processes take slot files of their own pool (`slots.evaluators`), so an evaluation never waits behind agent runs; both are `flock` files released by the kernel when the holder dies. The evaluator pool's default is derived from the CPU count. `max_parallel` row: "agent runs or evaluator processes in this job" stays.
3. `### Plug-in manifest (static; on the `schemas@` line)`: the sentence marked *Provisional:* about the project config listing plug-in directories becomes plain: plug-ins come from the directories of `plugins.dirs[]`, which `evaluate` reads, so `--plugin` goes. `## Deliberately open` then no longer lists "how plug-ins are registered": delete that item (one other item, `ls`, is Step 3's). If the coordinator rules the registration still open, restore the item and keep the sentence.
4. `### Configuration`: the user-config row gains `slots.evaluators` beside `slots.count`; the project-config row says `plugins.dirs[]` is where `evaluate` finds plug-ins; `slots.evaluators` is a machine-wide setting, refused in a project config like `slots.count`.
5. The `evaluate` row of the CLI table is Step 3's.
**Deliverables**: `__design__/architecture.md`: `### Job`, `### Budget`, `### Configuration`, `### Plug-in manifest (static; on the `schemas@` line)`, `## Deliberately open`
**Consistency Checks**: `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^2|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"` (expected: PASS)
**Commit**: `docs(design): record evaluate as a job and the evaluator slot pool`

## Step 3: The lean CLI surface
**Goal**: R01's CLI table and its notes say the final surface: `ingest` in place of `import`, `inspect`, `analyze`, `resolve`, `ls` decided, `evaluate`'s shape, output on terminals.
**Implementation Logic**:
1. Table rows: replace the `import` row by `candler ingest <path>…`, which turns raw Claude Code trace directories (one run per stream) into runs and, with `--rules <file>`, a legacy results folder under the import rules the file states; it runs to completion in the caller, repeats to the same store and is safe to interrupt; `candler import` stays an alias for a results folder. Add `candler resolve <experiment|suite>` (prints every resolved value with its source and runs the preflight; starts no job), `candler inspect <run dir | key prefix | job id>`, `candler analyze [--evaluator <name@version | version>] [--experiment <file>] [--format json|csv]` and `candler evaluate <suite> [<job-id> | --runs <glob>]` (starts the evaluate job, prints its id on stderr, follows it and prints the report; Ctrl-C cancels it; with no selection it grades the runs with no complete outcome set for the suite's evaluator versions). Add the `--error-format` row of Step 1. The last row's list shrinks to `show`, `diff`.
2. `candler ls`: remove "*Provisional:*". It lists experiment files under `<project root>/experiments/`, recursively, plus any path given; with no project it looks under `./experiments/`; a TOML file that does not parse as `experiment/1` is listed as skipped, with the reason. Scanning the whole project is rejected: slow, and it turns up suites and manifests.
3. After the table, a paragraph on terminals: `watch` prints event records (`--json`) by default and `--live` draws progress; colour is on only when stdout is a terminal and `NO_COLOR` is unset, and there is no `--no-color`; the binary embeds the container assets with `include_bytes!` and writes them to a cache keyed by their digest, so it runs from any directory.
4. `## Deliberately open`: delete "where `ls` looks;" and "how plug-ins are registered;" (D6 settles it: `plugins.dirs`) from the list of what development decides. In §Operation API's table, the `ingest` row's job column becomes "no: runs in the caller, repeats to the same store, safe to interrupt" (coordinator ruling, 2026-11-11: synchronous in M3; I4 guards the interrupt). Nothing else of those sections changes.
**Deliverables**: `__design__/architecture.md`: `### CLI surface over the jobs`, `## Deliberately open`
**Consistency Checks**: `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^3|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"` (expected: PASS)
**Commit**: `docs(design): record the lean cli surface`

## Step 4: The analysis statistics
**Goal**: R01 records D8 in a short section that points to the page for human readers.
**Implementation Logic**:
1. New `### Analysis statistics`, right after `### Wire schema: outcome record`. It states, in this order: the unit of replication is the run and an arm is a cell (outcomes of one run share the agent's behaviour, so they are not independent); the rate per role is Σnum/Σden with its counts and with no interval; the Wilson 95 % interval per (arm, expectation) applies only where every weight is 0/1, else it is omitted with the reason `weighted`; a comparison is made for every pair of cells in one comparable stratum that differ in exactly one declared factor, per expectation, by Fisher's exact test (two-sided) with the Newcombe difference interval; Holm's adjustment runs over every comparison and expectation of one role in one analysis; an invalid run is in neither k nor n and is reported per arm with its reason, and an end the experimenter caused is never charged to an arm; a pair the stratum verdict refuses is reported as refused, with its cause and its rates; α is 0.05, recorded with the source `core_default`, with no option. One sentence says these are Candler's defaults, not the analysis: every count is carried, so an author can analyse the same results another way.
2. It ends with a pointer to `docs/statistics.md` for what each method answers, where it stops being valid and what Candler does not claim.
**Deliverables**: `__design__/architecture.md`: `### Analysis statistics`
**Consistency Checks**: `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^4|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"` (expected: PASS)
**Commit**: `docs(design): record the analysis statistics`

## Step 5: Workspace targets and the landing order
**Goal**: R01 records D7 (what Candler builds for, and how a Windows port stays ready) and D1 (how the first changes land).
**Implementation Logic**:
1. New `### Workspace targets`, right after `### Workspace` (under `## Proposed State`, before `## Operation API`): Candler builds and runs on Linux and macOS; on Windows it runs inside WSL2, where it is Linux; the binary refuses to compile on a non-unix target with a message that names WSL2 (one `compile_error!`); `candler-schemas` and the Python helper stay portable. A native Windows port is a milestone of its own after M7. It starts from the committed inventory `crates/candler-core/tests/vectors/platform_inventory.toml`, which lists every file that calls a unix-only API (the count at the time of writing, from the inventory) with what it uses and its Windows counterpart, and from a guard that fails when a file outside the list starts to use one. A short table of the counterparts: process-group stop with escalation → Job Objects; descendants that escape → Job Objects (no sweep); never signalling a reused pid → a held process handle; `flock` → `LockFileEx`; a detached supervisor → `DETACHED_PROCESS`; Docker → the same CLI with host path forms for mounts; the helper's `os.fork` isolation → a spawn mode.
2. New `### Landing order` at the end of `## Implementation`, after the Tooling paragraph: the supervisor's orphan fix lands on `main` first, on its own, because the flake it cures fails every pull request's CI; the CLI scaffold (dependencies, the verbs as stubs, the registry skeleton) lands beside it; the rest of the lean CLI follows as one change. Two sentences, no dates, no plan references.
**Deliverables**: `__design__/architecture.md`: `### Workspace targets`, `### Landing order`
**Consistency Checks**: `test -z "$(sed -n '/^```terms$/,/^```$/p' __roadmap__/m3-lean-cli/verbs/design_record.md | grep '^5|' | while IFS='|' read -r n h t; do awk -v h="$h" '$0==h{f=1;next} /^##+ /{if(f)exit} f' __design__/architecture.md | grep -qF -- "$t" || echo "$h: $t"; done)"` (expected: PASS)
**Commit**: `docs(design): record the workspace targets and the landing order`
