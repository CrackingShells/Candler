# wire_shapes leaf report (findings)

## 1. Commits (base 000f8f1)
| sha | subject |
|---|---|
| d5dd2f7 | feat(schemas): declare agent environments and an egress allowlist |
| 154c171 | feat(isolation): admit the egress and plug-in environment paths |
| (this) | reports(isolation): record the wire_shapes leaf |

The sha of the report commit is that of branch tip `m5/wire_shapes`.

## 2. Gates
| gate | result | last line |
|---|---|---|
| `cargo check --workspace --all-targets` on each of the two commits | PASS | checked d5dd2f7, checked 154c171 |
| schemas guards (`agent_environments_`, `container_role_egress_`) | PASS | 4 tests run: 4 passed |
| `guard_settings_out_of_cell::` | PASS | 2 tests run: 2 passed |
| DRIFT | PASS | tree clean after bless |
| DIGEST (diff form, `guard_cell_digest`, core digest set) | PASS | 9 passed; 45 passed |
| SUBJ / G6 | PASS | No errored commits; exactly one release commit on schemas, `feat(schemas)` |
| OWNED | see finding F4 | |
| SPAWN | PASS | 9 passed |
| STD, `-p` forms | PASS | fmt ok; clippy -D warnings; doc; schemas 214, core 269 passed; candler `contract_pages::` and `r3_` pass |
| LINUX (`-p candler-schemas -p candler-core --tests`) | PASS | Finished |
| E2E, GOLDEN, Docker | not run | no container path or import/run path was changed beyond two match arms |

## 3. Claims checked
| claim | what the code showed |
|---|---|
| Step 1: can `Derived {step, from}` record a plug-in fragment value? | **Yes. No new `ValueSource` variant.** `from` is a free `String` ("the location the step read, as the experiment file wrote it"); `layers.rs` already attaches a per-path `origins` text to derived entries, and `render_source` prints "derived <step> from <from>". `"<plugin>/<env>"` is exactly the text the experiment wrote in `isolation.build.plugin`; `plugin_environment@<token>` is a valid `NameToken`. |
| `isolation.build.plugin` is a setting | True already by the `isolation.build.` prefix rule in `is_setting`. The `LEAVES` entry is redundant for it but added as the leaf says. |
| `is_location` | Not changed: the path names a plug-in environment, not a file. |
| `isolation.egress` is a cell value | True: not matched by `is_setting`. |
| `PluginManifest::new` callers | Signature kept (core callers unchanged); `with_agent_environments` added, and the reader calls it. |
| The map keeps `PluginManifest: Eq` | `JsonValue` was not `Eq`; added `impl Eq for JsonValue` (finite numbers only, so reflexive). |
| Contract page and codes.toml | `docs/contract/plugin-manifest.md` is generated and a test (`d1_regenerating...`) fails without regeneration; regenerated. Neither it nor `codes.toml` is under `crates/`, so the CI schemas release check (`ci.yml` 213-216: other `crates/` only) allows them in the schemas commit. |

## 4. Guards
| guard | Guards line | oracle | bite and failing message |
|---|---|---|---|
| `guard_execution_wire::container_role_egress_round_trips_and_the_older_roles_still_read` | R01 container record: role `egress` reads and round-trips; `agent`, `gateway` still read | role texts written by hand | `Egress` renamed to `proxy` on the wire: "role `egress` is refused: unknown variant `egress`, expected one of `agent`, `gateway`, `proxy`" |
| `guard_wire_shapes::agent_environments_are_optional_and_read_when_set` | R01 plug-in manifest: old manifests (five committed core fixtures plus a bare one) read and write no map; the map reads | hand-written TOML, committed fixtures | `skip_serializing_if` removed: red at the "empty map is omitted" assert |
| `guard_wire_shapes::agent_environments_refuse_unknown_keys_and_audiences` | environment is closed; input has no audience; input obeys `at` and reserved-kind rules | hand-written tables | `deny_unknown_fields` removed from `AgentEnvironment`: "an unknown key in an environment is accepted"; removed from `AgentInput`: "an audience on an input is accepted" |
| `guard_wire_shapes::agent_environments_refuse_a_value_path_outside_the_allowed_set` | only `isolation.build.*`, `isolation.egress`, `mcp.*`; segments decide | R06 D4 set as a table | prefix widened to `"mcp"`: "`mcp` is accepted" |
| `guard_settings_out_of_cell` row `build_plugin` | the setting never reaches a cell | hand row, control digest | `is_setting` excludes `plugin`: "build_plugin: ... must not be in `comparable.values`", "the cell digest differs", "the resolution lost the setting" |

Not guarded: the two `Egress` role arms (`guard.rs`, `lost.rs`). No owned test reaches them; `guard_container_guard` and the lost-job sweep tests pass unchanged. A guard row would sit in a file outside OWNED. Request: add one to the later leaf that first creates an `egress` container.

Red-first note: the schema guards did not compile before the types existed; the red seen is by bite on the finished code.

## 5. Findings
- **F1 (spec defect, name differs): the code name.** `schemas.plugin_manifest.environment_path_unsupported` cannot be a code here. A code is `<surface>.<reason>`; the surface is the first segment and must be in the contract pages' surface table (`d3_every_author_code_has_a_page` failed with "the surface `schemas` ... is not named"), and `_` is not allowed in the surface (`is_valid_code`). `schemas` is the codes.toml section, not a surface. I registered **`plugin.environment_path_unsupported`** in `[schemas]`, beside `plugin.command_empty`. Later leaves and docs must use that name. Needs the coordinator's ruling if the long name matters.
- **F2 (wording): "an unknown key inside an environment ... refused with the new code".** An unknown key is refused by the existing `plugin.unknown_field`; only an unsupported value path takes the new code. Guards test it that way.
- **F3 (additive, schemas crate):** `impl Eq for JsonValue`; a new `EvaluationError::EnvironmentPathUnsupported` variant (pointer `/agent_environments/<env>/values/<path>`); `AgentInput::new` reuses `ExperimentInput::new`'s checks; hand count `EvaluationError` 13 to 14 in `guard_refusal.rs`; `field_classes.toml` row `agent_environments = optional`.
- **F4 (outside OWNED, one line):** `crates/candler-core/tests/it/guard_resolve_surface.rs` holds the class count `(19, 18, 5)` for `field_paths.toml`; adding the two rows required `(20, 19, 5)`. Also the shown core file list adds nothing else outside OWNED besides `docs/contract/{codes.toml,plugin-manifest.md}` and `crates/candler-schemas/tests/{vectors/field_classes.toml, it/guard_refusal.rs}` (inside the schemas leaf's tree or generated). Revert the count line to refuse it; then b19 fails.
- **F5: `mcp` and `isolation.build` alone are refused** as environment value paths (prefix form, as written). Newly refused input applies only to the new map, so no released document changes.
- No released behaviour, recorded data shape, stored digest or `argv.json` changed. `codes.toml` and `field_paths.toml`: additions only. Reviewer (Sonnet): two minor findings fixed (BORROW comment, `const fn path`); a third (private field docs) left, matching the file's habit.
- Released-line note: the container record role set grows by `egress`; an older reader refuses such a record. Expected for the minor bump.

## 6. Docker
Not touched: no image, container or network was read, created or removed. e2e tests added: none.

Target dir `/private/tmp/claude-501/m5-target-wire_shapes` and scratch removed.
