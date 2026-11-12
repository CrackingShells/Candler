# contract_wire, leaf report v0

The refusal identity on the `schemas@` line: the `Refusal` trait, the `refusal/1` record, codes for every author-facing variant of `candler-schemas`, JSON pointers for parse errors, the `x-candler-class` keyword, and the P2 and P6 schema changes. A Sonnet implementer did the work in four rounds. Branch `m3/contract_wire` has 4 commits, head a14d80c, merged as `merge(schemas): land contract_wire`.

```
a14d80c build(evaluator-helper): regenerate the models for the new schema files
5678aeb feat(schemas)!: add the refusal record and refuse unknown keys
c63854c docs(schemas): register the codes of the schemas' refusals
b73bbfd test(schemas): pin the refusal, pointer and class oracles by hand
```

`5678aeb` is the only release-triggering commit and touches only `crates/candler-schemas/`. Its `BREAKING CHANGE:` footer names the newly refused input.

## P3's answers
- **`default` after `OmitNull`:** `#[serde(default)]` emits `"default"` and drops the field from `required`. A `present`-read `Option` and a plain `Option` emit neither. So `optional` is "not in `required`", derived from the committed schema.
- **`x-candler-class`:** `#[schemars(extend(…))]` puts it on the property. Beside a `$ref` it sits as a sibling of the `$ref`. It works on `remote = "Self"` types, on a `BTreeMap` and on a `Value`.
- **datamodel-codegen** ignores `x-` keywords. The regenerated helper models differ only in their sha256 headers.
- **Pointers:** with `deny_unknown_fields`, `serde_path_to_error` names the unknown field itself, in TOML and JSON, nested and in array items (`/list/1/bogus`). A missing field names its parent. An internally tagged enum (`environment`) loses the nested path, so the manifest's `environment` has a strict reader of its own (below).

## Codes
82 rows in `codes.toml` `[schemas]`, none deleted. 6 variants delegate and own no row: `ExecutionError::{Format, Values, Canonical}`, `OutcomeError::Format`, `EvaluationError::Canonical` and `ValuesError::Canonical`. Each returns the inner code and path with the wrapper's side.

| code | side | enum | variant |
|---|---|---|---|
| `experiment.path_overlap` | experiment | ExecutionError | PathOverlap |
| `experiment.unknown_field` | experiment | ExecutionError | UnknownField |
| `suite.dependency_cycle` | suite | EvaluationError | Cycle |
| `suite.document_invalid` | suite | EvaluationError | Toml |
| `suite.unknown_field` | suite | EvaluationError | SuiteUnknownField |
| `plugin.manifest_invalid` | plugin | EvaluationError | ManifestToml |
| `plugin.command_empty` | plugin | EvaluationError | ManifestCommandEmpty |
| `plugin.unknown_field` | plugin | EvaluationError | ManifestUnknownField |
| `outcome.cause_missing` | plugin | OutcomeError | CauseMissing |
| `catalog.schema_tag_mismatch` | environment | CatalogError | SchemaTag |
| `format.name_invalid` | config | FormatError | Name |

No `#[error(` line was edited. The old and new texts of 24 pointer cases and 6 syntax errors are byte-identical. No digest or vector moved.

## Rulings taken during the leaf (coordination session, 2026-11-12, for Eliott's review)
1. **Unknown keys are refused** in `experiment/1`, `suite/1` and `plugin-manifest/1`, with the codes `<surface>.unknown_field`. Records Candler writes stay lenient. The open tables are `[fixed]`, a factor's levels, a suite entry's `params` and the suite's `source`.
   - The closed tables: every table of the three documents except the open ones. The guard `c3_an_unknown_key_in_every_closed_table_is_refused_at_its_pointer` plants a key in each.
   - The manifest's `protocol` is read through a strict private twin, so `describe` results stay lenient.
   - All 18 author files in the repository and both HTN plug-in files were parsed read-only with the new readers. The one stray key that matters is the golden suite's `[source]` (ruling 2). `vectors/experiment/legacy_cells.toml` has a `case` key, but it is a vector read as raw TOML, not as an experiment.
2. **`suite/1` gains an optional free `source` table.** It records where a translated suite came from. Candler stores it and never reads it. The golden suite vector is untouched.
3. **The plug-in manifest's errors blame the plug-in.** `EvaluationError::Toml` was split into `Toml` (suite) and `ManifestToml` (plugin). An empty `command` got its own variant, `ManifestCommandEmpty`. `CatalogError` is coded, with side `environment` or `candler`.
4. **`suite.toml_invalid` was renamed `suite.document_invalid` before any release,** since it covers shape errors as well as syntax.
5. **`JobSpec` embeds the now-strict `ExperimentFile` and `Budget`, and that is accepted.** The job spec is version-locked: only the supervisor that the same executable spawns reads it, and `ps` shows `-` for a spec it cannot read.
6. **An unknown dotted path under `[fixed]` is accepted by the schemas reader.** Only core's path classes can refuse it. Planned as `refusal_core` Step 6 (`experiment.unknown_path`, `fix(cli)!`).

## Spec defects found
- R10's sample class `schema` → `candler` is wrong: `schema` is `required` in the three author documents.
- No `free` class on an outcome's `value` text: `OutcomeRecord`, `Evidence`, `Value` and `RunKey` are shared with `event-envelope` and `run-manifest`, so the keyword would leak into those files. The classes declared are `candler` on `EvaluateParams.{context, items, prior, scratch}`, and `free` on a suite entry's `group`, an RPC error's `data` and the suite's `source`.
- A refusal with neither `path` nor `file` is valid: `Refusal::path` is `None` for a variant that names no place. Both together is invalid.
- `FormatError` has 19 variants, not 20. The 20th code in the `format` surface is `SchemaTagError`.
- Variants that carry a resolved field path (`PathOverlap`, `EmptyTable`, `Duplicate`) return no pointer: that path is not a place in the file.
- The brief asked for a leaf report on the branch, which the OWNED gate forbids. The coordinator commits this one instead, and the brief was amended.
- The G-codes gate against a26fe53 was tautological, since `codes.toml` did not exist there. The gate now runs against each leaf's base, and against 8aad2c9 for the milestone.

## Verifier
A Sonnet spec-verifier attacked e09438e and returned FAIL with findings F1–F6; every gate passed.
- F1: `environment = { kind = "none", bogus = 1 }` was accepted, because `deny_unknown_fields` does not reach a unit variant of an internally tagged enum.
- F2: `command = []` was coded `suite.list_empty`.
- F4: wrong pointer for a key named like its tagged parent.
- F5: a stale doc line.
- F6: the rename in ruling 4.
- F3: accepted, as ruling 5.

Round 4 fixed F1, F2, F4, F5 and F6. The verifier re-checked a14d80c and returned PASS. It probed the strict environment reader with duplicate keys, a missing or mistyped `kind`, and escaped key names. Two bites of its own against the new guard went red: the `none` key check removed, and `deny_unknown_fields` removed from `PerRunCaps`. Three Low notes:
- **N1, accepted:** `located()` matches serde's message text, so a key literally named ``x` of this table`` gets its name appended twice. Only pathological names are affected.
- **N2, follow-up:** a wrong type or value inside `environment` now points at `/environment`, not at the field (`project = 3`, a bad `kind`). The pointer is valid, and it names the parent. It can be fixed by reading the second stage through `serde_path_to_error`, before the codes' pages are written (`docs_refusals`).
- **N3, accepted like `count_zero`:** `experiment.entry_repeated` (`ExecutionError::Duplicate`) is also raised from Candler-internal checks (`check_ascending`, `check_sources`, `Comparable::from_resolved`). A variant has one side, so an internal duplicate blames the experiment. Splitting the variant is possible only before the codes are released.

## Bites (each seen red, then reverted)
- **C1:** `code` serialised as an index; the `Environment` side renamed `env`.
- **C2:** two variants sharing a code; a code renamed in the table; a `codes.toml` row deleted; a side changed in the table only; `ManifestCommandEmpty` folded back into `EmptyList`.
- **C3:** the pointer dropped; `deny_unknown_fields` removed from `Budget`, `RawSuiteEntry` or `RawPluginManifest`; the `none` key check removed from the environment reader; the tagged-enum key not appended.
- **C4:** a declared class contradicting the hand table (`scratch` as `free`); a property with no row.
- **C5:** `base_commit` left required; `null` accepted.
- **C6:** `evaluation.planned` missing from `KNOWN_EVENT_KINDS`.

## Gates
All PASS on a14d80c, with `--no-tests=fail` where it applies:
- OWNED;
- one release commit, touching only the schemas crate (`^feat(schemas)!\?: `);
- the four-commit order;
- `trace.rs` untouched; no `#[error(` line edited;
- C1–C6;
- G-codes against d7c896b: 410 added, 0 deleted;
- the C4 and C5 jq checks; the `refusal/1` root;
- the digest guards, with no vector edited; the golden suite guard;
- the domain-name grep;
- DRIFT; the helper's models with pytest and ruff;
- STD (`-p` forms and `cargo check --workspace --all-targets`), LINUX, SUBJ;
- the core digest guards; the workspace nextest.

One load flake was seen in round 1 and is not from this leaf: `guard_supervise::handles_carry_bytes_requests_and_captures`, which passed on rerun.

## Production
| Round | Time | Tokens | Cause |
|---|---|---|---|
| 1 | 29 min | 356k | the leaf |
| 2 | 45 min | 417k | the unknown-fields ruling, the `Toml` split, `CatalogError` |
| 3 | short | — | `source` declared |
| verify ×2 | 8 min + 4 min | 190k + 215k | FAIL F1–F6, then PASS |
| 4 | 8 min | 458k (the harness's count for the resumed agent) | F1–F6 |

Peak target dir 4.0 GB; 255–259 GiB free throughout.
