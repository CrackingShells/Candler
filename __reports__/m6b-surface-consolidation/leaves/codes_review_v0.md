# Refusal codes review (v0)

Leaf `codes_review` of M6b. Tip `a0df4d48`. Facts first (this section), advice after (`## Registry verdict`). `F=docs/contract/codes.toml` in every command; `python3` has no `tomllib`, so every count is `awk`/`grep` over the file. Code citations come from colgrep reads of the coordinator worktree, which holds the same code.

## Measurements

### M1. The 377 rows by table, and the 85-versus-83 question
The file is append-only, so a row added later sits under whatever header came last: seven rows sit after the `[helper]` header (`F:1898-1930`). Counting by the header above a row and counting by the table in the row's own key give different numbers.

| Count | `internal` | `helper` | `resolve` | `schemas` |
|:--|--:|--:|--:|--:|
| by header above the row | 83 | 34 | 12 | 83 |
| by table in the row's key | **85** | **27** | **17** | 83 |

- by header: `awk '/^\[[a-z_]+\]$/{s=$0} /^\[[a-z_]+\./{n[s]++} END{for(k in n)print k,n[k]}' $F`
- by key: `awk '/^\[[a-z_]+\."/{t=$0;sub(/^\[/,"",t);sub(/\..*/,"",t);n[t]++} END{for(k in n)print k,n[k]}' $F`
- the strays: `grep -n '^\[internal\.' $F | awk -F: '$1>1761'` prints `1918` and `1922` (two internal rows); `grep -n '^\[resolve\.' $F | awk -F: '$1>1761'` gives five resolve rows (`1898`..`1926`).

**Settled: `internal` is 85.** R04's 85 counts by key and is right; the planning recount of 83 counted by header and is wrong for rows 1918 and 1922 (`candler_core::tree::WalkError`, `...::plugin_environment::FragmentError`). Total check: 85 + 27 + 265 = 377 (`grep -c '^\[[a-z_]*\."' $F` after removing the 18 bare section lines gives 377).

All tables by key: schemas 83, internal 85, evaluation 40, isolation 32, helper 27, jobs 20, import 18, resolve 17, cli 17, runner 10, harness 9, analysis 7, store 6, supervise 3, experiment 2, manifest 1. Bare empty sections: `[frames]`, `[trace]` (`F:786`, `F:1325`).

### M2. Rows that are not caller codes
| Kind | Rows | Command |
|:--|--:|:--|
| `[internal]` allowlist (print `candler.internal`, no `side`, key is `crate::path::Type::Variant`) | 85 | `grep -c '^\[internal\.' $F` |
| `internal` rows carrying a `side` | 0 | `awk '/^\[internal\./{a=1;next} /^\[/{a=0} a&&/^side/{c++} END{print c+0}' $F` |
| `[helper]` rows (Python side: `side` plugin 22, candler 4, suite 1; key `helper.*`) | 27 | `awk '/^\[helper\./{h=1;next} /^\[/{h=0} h&&/^side/{c++} END{print c+0}' $F` |
| retired (`F:675`, replaced by `isolation.harness_version_unreadable`) | 1 | `grep -n '^retired' $F` |
| lifted (`F:1930`, `resolve."isolation.egress_unsupported"`) | 1 | `grep -n '^lifted' $F` |
| caller codes, live | 263 | 377 - 85 - 27 - 2 |

So 112 of the 377 rows (30 %) are not caller codes; 263 are. R04's "375 live" is right but hides that only 263 are codes a caller can parse as `refusal/1`.

### M3. Caller codes by side (265 rows with a side outside `internal` and `helper`)
Command: `awk '/^\[[a-z_]+\."/{t=$0;sub(/^\[/,"",t);sub(/\..*/,"",t)} /^side *=/{c[$3]++} END{for(k in c)print k,c[k]}' $F` (includes the 27 helper rows' sides; helper-free counts below subtract them).

| side | all rows with a side | of which helper |
|:--|--:|--:|
| environment | 91 | 0 |
| plugin | 58 | 22 |
| config | 56 | 0 |
| experiment | 39 | 0 |
| candler | 28 | 4 |
| suite | 20 | 1 |

Caller rows with side `candler` (a defect in Candler, never an author's error): 24, all in `schemas` (`manifest.*`, `catalog.*`, `outcome.*` builders). By table: environment is 100 % of `isolation` (23), `jobs` (17), `runner` (7), `store` (6), `harness` (6), `supervise` (3), `import` (15 of 18); `evaluation` mixes four sides (plugin 17, suite 12, environment 7, config 4).

### M4. The table is not the code prefix
Caller codes by the first segment of the code (265 rows): isolation 37, experiment 26, config 23, jobs 20, format 20, evaluation 18, plugin 16, outcome 16, import 16, suite 15, runner 8, manifest 8, analysis 7, store 6, harness 6, catalog 6, canonical 5, resolve 4, supervise 3, protocol 3, values 1, cli 1 = 22 prefixes over 14 caller tables (`awk '{print $1}' caller | sed 's/\..*//' | sort | uniq -c`, where `caller` is the 265 keys). `experiment.*` codes live in five tables (schemas, experiment, runner, resolve, harness); `config.*` in cli, resolve, import. The registry's header says "one section per surface" (`F:7`); in practice a section is a Rust module's catalog, and a surface (the code prefix) spans sections. A caller reads the prefix; the table is Candler's own bookkeeping.

### M5. Emit sites
Method: for each of the 265 caller rows, `git grep -hE "(Enum|Self)::Variant([^A-Za-z0-9_]|$)" -- crates ':!*refusal.rs' ':!*refusal_impls.rs' ':!*/tests/*' ':!*tests.rs'`, comment lines dropped, lines counted. The catalog reads (`candler_core::refusal::catalog`, `crates/candler-core/src/refusal.rs:13-42`, which chains the per-module `catalog()` and `document_value_rows`, `refusal.rs:252`) and the code-to-variant tables (`refusal_table!` blocks, e.g. `crates/candler-schemas/src/refusal_impls.rs:61-198`) are the only places that name every row, and are excluded.

| Emit sites outside tables and tests | Codes |
|:--|--:|
| 0 | 8 |
| 1 | 129 |
| 2 or more | 128 |

The eight at zero: `format.schema_tag_mismatch`, `protocol.version_mismatch` (rows with no `Enum::Variant` shape), `experiment.value_invalid`, `store.value_invalid`, `import.value_invalid`, `resolve.value_invalid` (wrappers built by `#[from]`/delegate over `format.*`), `cli.usage_invalid`, `evaluation.evaluator_not_started` (`ClientError::Spawn`). Zero hits is a static signal, not proof of a dead code: a `#[from]` conversion or a macro builds a variant without naming it. **No caller code is proven unemitted by this count; none of the eight is proven emitted either.** One site is not safe: it can be a single constructor behind a guard.

Emit versus page: `config.no_store` has one site, `CliError::refused_by(CliRefusal::NoStore)` in `resolved_root` (`crates/candler/src/commands/common.rs:103`), and its row text says "Not raised today: a verb that is given no store root takes the default store" (`crates/candler/src/commands/refusal.rs:235-240`, `docs/contract/index.md:70`), while the variant's own message says "no store: pass --store, set CANDLER_STORE or configure store.root" (`refusal.rs:24` region). The row and the code disagree; whether the site can fire depends on whether the settings always carry a default store root (R06 G9 says a missing root takes the default).

### M6. Codes behind `*.document_invalid` (G8)
`ExperimentFile::from_toml_str` flattens every deserialization error of the document into `ExecutionError::Document { detail, path }` unless the error is an unknown field (`crates/candler-schemas/src/execution.rs:1285-1301`, via `read_value`, `crates/candler-schemas/src/refusal.rs:747`, which keeps the pointer from `serde_path_to_error` but turns the inner error into a string). The suite file's own wrapper is the same shape (`suite.document_invalid`, `EvaluationError::Toml`). So the inner reason of a failed document is a sentence in `detail`, never a code, on the file-read path.

Candidate rows whose code the file-read path cannot print (static evidence from the page Notes and from the flattening; programmatic builders and `ExperimentFile::new` callers are not traced here):

| Group | Rows | Count |
|:--|:--|--:|
| experiment inner | `count_zero`, `cost_not_positive`, `factor_without_levels`, `table_empty`, `path_overlap`, `input_invalid`, `entry_repeated`, `reserved_root_misused`, `reserved_value_malformed`, `absolute_path` | 10 |
| suite inner | `suite.list_empty`, `id_repeated`, `dependency_unknown`, `dependency_cycle` | 4 |
| `format.*` (the three author documents never print them, per `docs/contract/experiment-file.md:108`) | 19 of 20 (`schema_tag_mismatch` excepted) | 19 |
| `canonical.*`, `values.null` | | 6 |

Command for the experiment/suite rows: `grep -E 'count_zero|cost_not_positive|factor_without_levels|table_empty|path_overlap|input_invalid|entry_repeated|reserved_root_misused|reserved_value_malformed|absolute_path' docs/contract/codes.toml`. Total shadowed candidates: 39 of 263 live caller codes (15 %).

### M7. Codes that share one cause or one fix
Counted on the 265 keys with `awk '{print $1}' caller | grep -c '<pattern>'`:

| Family | Rows | Pattern |
|:--|--:|:--|
| `*.value_invalid` wrapper of `format.*` | 6 | experiment, resolve, store, import, outcome, cell value |
| `*.io_failed` | 6 | store, supervise, jobs (2), evaluation (2) |
| `*.field_unset` / `*_field_unset` (builder left a required field unset; side `candler`) | 5 | manifest, protocol, outcome, catalog, config |
| `*.unknown_field` | 3 (+ `config.unknown_key`) | experiment, suite, plugin |
| `isolation.gateway_*` | 8 | unavailable, start_failed, misconfigured, no_answer, unauthorized, capped, upstream_error, not_serving |
| `*docker_*` | 7 | one cause (Docker not answering) in isolation and jobs |
| `isolation.mount_*` | 5 | |
| `isolation.env_name_*` | 3 | |
| `*dependency_*` | 4 | `suite.*` and `evaluation.*` twins (`suite.dependency_cycle` vs `evaluation.dependency_cycle`; `dependency_unknown` likewise) |
| `*toml_invalid`/`json_invalid` | 4 | |
| `*unreadable` | 17 | different files, different fixes |

Granularity range: `isolation` carries 37 codes under 9 prefixes of mounting, env, gateway, image, sweep and docker; `manifest` 8 and `catalog` 6 are all `candler`-side builders; `experiment` (the table) holds 2 and `manifest` 1.

### M8. Contract pages
- Caller codes on no page Refusals table: 24 (`format.*` 10, `manifest.*` 7, `catalog.*` 3, `outcome.*` 2, `protocol.field_unset` 1; the rest `format.*`), `comm` of the page rows against the registry keys (page rows: `grep -hoE '^\| \`[a-z_]+\.[a-z_0-9]+\`' docs/contract/*.md`, which also matches field rows, so the page-minus-registry side is noise and is not used).
- Version stamps `[checked: v@x]` (`grep -n 'checked: v@' docs/contract/*.md`): 21 stamps. `v@0.6.0` on 10 (`suite-file.md:66-69`, `plugin-manifest.md:62-66`, `evaluator-protocol.md:511`), `v@0.7.0` on 11 (`experiment-file.md:106-110`, `driver-traits.md:72-75`, `harness-capabilities.md:41-43`). The workspace is `0.13.0` (`Cargo.toml:6`), `candler-schemas` is `0.11.3` (`crates/candler-schemas/Cargo.toml:4`).
- `docs/contract/index.md:70` says `config.no_store` is "Not raised today" (see M5).
