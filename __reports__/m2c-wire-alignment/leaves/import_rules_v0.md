# Leaf report: import_rules (v0)

Branch `m2c/import_rules`, cut from `feat/m2c-wire-alignment` at 2343ecd. Nothing pushed.

## 1. Commits

| sha | subject |
|---|---|
| 7872a22 | `feat(importer)!: take the imported entry's params from the rules` |
| (this file, last commit) | `reports(importer): record the import_rules leaf` |

The fix round (the F1 pin and the CLI guard) is folded into the first commit by `--fixup` and autosquash onto 2343ecd, so the branch holds two commits; `cargo check --all-targets -p candler -p candler-core` passes on each. The first commit carries a `BREAKING CHANGE:` footer naming the migration (`entry_params = ["scale", "text", "tier"]` for the HTN rules).

## 2. Gates

| gate | result | last relevant line |
|---|---|---|
| Constant and word `tier` gone from core and binary sources | PASS | `! git grep ENTRY_PARAMS ... && ! git grep -w -i tier ...` exit 0 |
| R1 (`binary(guard_import_mini) & test(/entry_params/)`) | PASS | 3 tests run: 3 passed |
| G5c: `guard_cell_digest` (schemas) | PASS | 9 tests run: 9 passed |
| G5c: core digest set plus `guard_layering_i1` | PASS | 40 tests run: 40 passed (`guard_import_mini`, `guard_legacy_mapping`, `guard_oracle_mini`, `guard_run_context`, `guard_engine`, `guard_seams`) |
| GOLDEN, `guard_golden` | PASS | 2 tests run: 2 passed (2 slow), 82.5 s |
| GOLDEN, `guard_oracle_ac` | PASS | 1 test run: 1 passed |
| `guard_cli` | PASS | 5 tests run: 5 passed, after the pin update (F1) and the new guard |
| `guard_cli & test(/entry_params/)` | PASS | 1 test run: 1 passed |
| `guard_golden_parity` | NOT RUN | `CANDLER_HTN_PLUGIN` is unset, the plug-in is not at hand (see F2) |
| STD `-p candler-core -p candler`: fmt, clippy `-D warnings`, rustdoc `-D warnings` | PASS | clippy and doc finish clean |
| STD nextest `-p candler-core -p candler` | PASS | 160 tests run: 160 passed, 17 skipped |
| LINUX (`--target x86_64-unknown-linux-gnu`, `-p candler-core -p candler`) | PASS | Finished `dev` profile |
| SUBJ (base 2343ecd) | PASS | `cog check`: No errored commits; subject 63 characters |
| SPAWN (`guard_signal`, after `cargo clean -p candler-core`) | PASS | 9 tests run: 9 passed |
| OWNED check | PASS | the diff lists only `import/{grading,legacy,rules}.rs`, `commands/import.rs`, the mini-corpus rules fixture, `support/legacy_rules.rs`, `guard_import_mini.rs`, and the `rules_without_markers` hunk (one line) of `guard_layering_i1.rs` |
| DRIFT, G5a | not applicable | `candler-schemas` and the test vectors are untouched |

The G5c, GOLDEN and SPAWN rows were run before the fix round; the round touched only `guard_cli.rs` (test code), which none of them reads. After the round I re-ran `guard_cli`, R1, nextest of both crates, clippy, LINUX, doc, fmt and SUBJ.

Clippy: one run failed once on `guard_layering_i1` ("4 previous errors") before the round and passed on every rerun; I did not capture its text and did not reproduce it. In the round, clippy rejected my first draft of the new guard (`clippy::format_collect`: `format!` inside `.map(...).collect::<String>()`); that is a real lint, fixed, and unrelated to the flake.

## 3. Guards

Three are in `guard_import_mini.rs` and one in `guard_cli.rs`; I saw the first two red before the importer change, for the right reason.

| id | Guards | Oracle | Bite and seen failure |
|---|---|---|---|
| `entry_params_come_from_the_rules` | R01 §Golden corpus: rules naming `["scale","text"]` import the mini-corpus with the oracle's rows and a suite entry without `tier`; the HTN rules (`legacy_rules::rules()`) keep the entry with `tier` | `shasum -a 256` over the hand-written canonical entry JSON of A1, run 6, with and without `tier` (`3982...bc24` and `b0db...4d44701`; the second equals the pinned digest of `guard_legacy_mapping`), and `oracle::expected_rows` | Constant restored (importer reads a fixed list, rules ignored): `left: "sha256:b0db76be..." right: "sha256:39822149..."`. Ownership check: dropping `tier` from `legacy_rules::rules()` gives the same assertion with the sides swapped, so this guard is red |
| `entry_params_naming_a_key_an_entry_lacks_are_refused` | I12: a named key that an entry lacks is refused, naming the key and the entry | the entry ids of `oracle::expected_rows`; no grading file holds `bogus` | Constant restored: `the import is refused: ImportReport {...}` (the import succeeded). The refusal text read is "`<id>` has no `bogus`" |
| `entry_params_are_required_by_the_builder` | I12: no silent default; the builder refuses without `entry_params`, naming the field, while an explicit empty list builds | the hand-written expectation | Default injected (`unwrap_or_default()`): `the rules lack the entry params: ImportRules { ... entry_params: [] ... }` |

Fourth guard, in `guard_cli.rs` (OWNED extended by the coordinator):

- **`rules_without_entry_params_are_refused_naming_the_key`**
  - Guards: I12. The fixture rules without their `entry_params` line are refused by `candler import` (exit 2), the refusal names `entry_params`, and no run is written.
  - Oracle: the fixture with that one line removed, and the binary's exit-status contract.
  - Bite (`#[serde(default)]` on `RulesFile::entry_params`): `assertion left == right failed: Output { status: ExitStatus(unix_wait_status(0)), stdout: "{\"records\":136,\"runs\":6}\n", stderr: "" }`; the import succeeded.

## 4. Probe answers

None; the leaf has no probe.

## 5. Findings

- **F1, spec defect, resolved.** `guard_cli.rs` pins `RULES_SHA256`, the `shasum -a 256` of `mini-corpus-rules.toml`, so adding `entry_params` to the fixture broke it. The coordinator accepted this and put `guard_cli.rs` in OWNED. The pin went from `f26fef6c...cd87e` to `b4387a08...00ccb`, recomputed with `shasum -a 256` on the file at the tip; its oracle stays the file's own hash.
- **F2, external dependency.** `guard_golden_parity` pins the sha256 of the plug-in's `import/htn-results-rules.toml` and imports with it. That file lacks `entry_params`, and the new key is required, so the plug-in's rules file (and the pin at `guard_golden_parity.rs:44`) needs updating before the ignored test can pass. The plug-in's rules file is not in Candler, so this is out of the leaf's reach; the coordinator carries it to Eliott. `guard_golden_parity` is left as it is and was not run.
- **F3, gap in the leaf's checks, closed.** The leaf expects `guard_import_mini` to go red when `tier` leaves the HTN rules; before this leaf it held no suite-entry digest check, so only `guard_legacy_mapping` caught it. I made the HTN half of `entry_params_come_from_the_rules` use `legacy_rules::rules()` with the pinned A1 digest. The binary-interface guard is the fourth guard above.
- **Deviations from the leaf:**
  - `legacy_rules.rs` gained `builder`, `entry_params` and a `try_rules` that calls them, so the guards can vary the list. This is inside OWNED.
  - The builder keeps `entry_params` as `Option<Vec<Name>>`, so an explicit empty list is allowed and only an absent one is refused.
  - The only rules TOML under `tests/fixtures` is `mini-corpus-rules.toml`.
- **Names:** no difference from the shared constraints; `ImportRules::entry_params: Vec<Name>`, `ImportRulesBuilder::entry_params(Vec<Name>)`, `RulesFile::entry_params: Vec<String>` (required).
- No confirm tests remain.
