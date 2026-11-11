# docs_refusals, leaf report v0

The six contract pages now list their refusals, each page holds tentative Notes tried by hand, and the helper's own errors carry the codes of `codes.toml [helper]`. Branch `m3/docs_refusals` has 4 commits from `a72342b` before this report. Step 5 is skipped: the driver page is not on this branch.

## What landed
| Step | Commit | Result |
|---|---|---|
| 1 | `docs(evaluator-helper)` | 27 rows in `[helper]` (13 outcome, 2 plug-in, 7 instrument, 5 protocol); 135 lines added, none removed |
| 2 | `feat(evaluator-helper)` | `OutcomeError`, `IdentityError`, `ProtocolError`, `Refused`, `_ParamsError` and a new `PluginError` take a keyword `refusal_code`, kept as an attribute |
| 3 | `docs(contributing)` | generated Refusals on six pages and the index's Other refusals, with D3 |
| 4 | `docs(contributing)` | 3 to 6 Notes per page, 67 to 113 words each page |
| 5 | none | skipped: the driver page is not on this branch |

## Decisions in the leaf
- **Helper rows.** Side `plugin` for what an author wrote or installed, `candler` for a line or request Candler sent that the models refuse. Each row has `side`, `when` and `at` (`-` throughout, the helper reports no path).
- **`PluginError`** is a `ValueError` and a `TypeError`, so a caller that caught either still does. It replaces the bare `TypeError` of a module with no `KINDS` and the bare `ValueError` of a plug-in with no name or version. `str(error)` is unchanged everywhere. The error replies carry no `data` field: a code there would change the wire vectors, so `refusal_code` stays an attribute.
- **Allowlist of the guard** (one line each with its reason): the module `__getattr__`'s `AttributeError`, and the fd guard's `RuntimeError`.
- **Catalogue rows on pages.** A page lists the catalogue's codes whose side is not `candler`, plus every `[helper]` row (the helper's Candler-side rows say what a malformed request does). A code that several variants raise is listed once. A variant that delegates has no code of its own.
- **Which page.** By the code's surface, from a hand table in `support/contract_pages/refusals.rs`: `plugin`, `suite`, `protocol`, `outcome`, `experiment`, `helper` to their pages; `format`, `canonical` and `values` to the experiment page. `experiment.value_invalid` and `outcome.value_invalid` (rows of `[manifest]` and `[evaluation]`) land on their pages by their code.
- The D3 finder reads a backticked `<surface>.<reason>` whose surface the table names, and skips file names (`plugin.toml`).

## Surfaces that landed in the index, not on a page
`analysis`, `catalog`, `config`, `evaluation`, `harness`, `import`, `isolation`, `jobs`, `resolve`, `runner`, `store` and `supervise`. `manifest` and `candler` are in the table too, but every code of theirs has side `candler`, so no list holds them.

## Bites (each seen red, then reverted)
| Guard | Mutation | Result |
|---|---|---|
| `d3_every_code_on_a_page_is_in_the_catalog` | a code of the suite page changed to `suite.list_gone`, which no row has | red, names the page and the code |
| `d3_every_author_code_has_a_page` | the same edit (the real code is then unlisted) | red |
| `d3_every_author_code_has_a_page` | a row removed from the suite page | red, names `suite.id_repeated` |
| `d3_every_author_code_has_a_page` | a row removed from the index | red, names `store.io_failed` |
| `d3_every_author_code_has_a_page` | `store` removed from the surface table | red, names the surface and `store.digest_mismatch` |
| `test_d4_every_raise_carries_a_listed_code` | `raise ValueError("x")` added to `plugin.py` | red |
| `test_d4_every_raise_carries_a_listed_code` | a code `[helper]` lacks passed at a raise site | red (the unused row trips the second guard too) |
| `test_d4_every_helper_code_is_raised` | a `[helper]` row no raise site uses | red |

D3's first guard passes vacuously against the empty placeholders (no code is named there yet); its bite is the page-side mutation above. The second was red before generation.

## Notes and the input that confirmed each
Scratch inputs lived under `/private/tmp/claude-501/dr_try` and are removed. `candler` is the binary at this head. `EV` is `candler --error-format json evaluate <suite>` from a project whose `candler.toml` lists `plugins.dirs = ["../plugin"]`, over a store holding only `store.json` of schema `candler-store/1`. `RS` is `candler --error-format json resolve <file>`. `HP` is `python -m candler_evaluator <module>` with the module on `PYTHONPATH`, fed lines on stdin; request ids are UUID v7.

| Page | Note | Input |
|---|---|---|
| plugin-manifest | unknown key anywhere is `plugin.unknown_field` | EV; `color = "red"` at the top gives path `/color`; `kind = "none"` with `project` gives `/environment/project` |
| plugin-manifest | `command` runs in the plug-in directory | EV; `command = ["./run.sh"]` and `run.sh` doing `pwd >&2; exit 3`: the `plugin.describe_failed` message ends with the plug-in directory |
| plugin-manifest | `uv` needs `project` and `lock` | EV; `kind = "uv"`, `project` only: `plugin.manifest_invalid`, path `/environment`, "missing field `lock`" |
| plugin-manifest | `plugin.toml` or `candler-plugin.toml`, not both | EV; both files: `plugin.manifest_ambiguous`; only `candler-plugin.toml`: reaches `describe` |
| suite-file | `source` is open | EV; `[source]` with a nested table and a list, and a JSON suite with `source`, `group` and deep `params`: all pass the suite check. The words "stores it and never reads it" are the coordinator's ruling; no hand input shows them |
| suite-file | unknown key is `suite.unknown_field` | EV; `color = 1` at the top (`/color`); `zzz` in an entry of a JSON suite (`/expectations/0/zzz`) |
| suite-file | wrong form is `suite.document_invalid` | EV; `id = "r 1"` (path `/expectations/0/id`); `any_of = []` (path `/expectations/0/applies_to/0`) |
| suite-file | TOML, or JSON by the `.json` extension | EV; a JSON suite named `.json` passes; the same text named `.toml` is `suite.document_invalid`; a TOML suite named `.json` is `suite.json_invalid` |
| suite-file | cycle, unknown id, repeated id | EV; `R1` depends on `["R1"]`, `["R9"]`, and `R2` and `R1` in a loop; two entries with id `R1` |
| suite-file | suite is checked before a plug-in describes itself | EV; with a plug-in that cannot describe itself, each bad suite above gives its suite code, a good one gives `plugin.describe_failed` |
| evaluator-protocol | non-UUID-v7 id or non-JSON line: dropped, logged | HP; ids `d1`, `m1`, `e0` and the line `not json`: stdout empty, stderr "dropped a malformed line" |
| evaluator-protocol | -32601 and -32602 | HP; `{"jsonrpc":"2.0","id":"<v7>","method":"frobnicate"}` and an `evaluate` with `params: {}` |
| evaluator-protocol | result `{"reported": n}` after the outcomes | HP; one `evaluate` item: one `outcome` line, then `{"reported":1}` |
| outcome-record | cause rule | the helper's `normalise_outcome` on a `pass` with a cause, a `blocked` with none and with two keys; and the schema crate's `OutcomeRecord` deserialiser on the same, by a scratch test since deleted |
| outcome-record | weight rule | the same two routes: `not_measured` with a weight, `pass` 1/2, `fail` 2/2, `blocked` 1/2, `fail` 3/2 refused; `fail` 1/2 and `blocked` 0/2 accepted |
| outcome-record | helper placeholders | HP; the `outcome` line has `role` `descriptive` and both digests of 64 zeros |
| experiment-file | no free fields; unknown key | RS; `color = "x"` at the top (`/color`); `foo = 1` under `[budget]` (path `/foo`) |
| experiment-file | date is `experiment.value_unrepresentable` | RS; `[fixed] model = 1979-05-27`; also `x = nan` |
| experiment-file | wrong-form value is `experiment.document_invalid` | RS; `secrets = ["bad name"]` (`/secrets/0`), harness name `bad name` (`/harness/name`), treatment name `bad name`, `[fixed] "x y" = 1`, `replicates = 0`, a factor with `levels = []` |
| experiment-file | overlapping paths | RS; a factor `model` twice; `[fixed] model` with a factor `model`; a level that is a table under a factor |
| evaluator-helper | a bad outcome costs its item | HP; kinds returning `7`, `{"status":"pass","color":"red"}`, `{"status":"blocked"}` and a `pass` of weight 1/2: each record is `not_measured`, cause `instrument_error`, with the error in the note |
| evaluator-helper | deadline | HP; `deadline_ms` 100 with a kind that sleeps 5 s: note "item deadline of 100 ms exceeded (SIGALRM)" |
| evaluator-helper | `print` goes to stderr | HP; a kind that prints: stdout holds only frames |
| evaluator-helper | index install fails `describe` | HP; `INSTRUMENTS = ["pydantic"]`: error -32000, "no direct_url.json" |
| evaluator-helper | no `KINDS` exits 1 | HP; a module with a name and a version only: exit 1 and a `PluginError` traceback, no frame |

## Spec defects and findings for the coordinator
- **The raise gate is vacuous.** `! git grep -n -E '^\s*raise (RuntimeError|ValueError|TypeError)\(' -- 'python/.../{plugin,frames,identity,loop}.py'` finds nothing on macOS: BSD `-E` has no `\s`, and git does not expand the braces in a plain pathspec. With `[[:space:]]` and the four paths written out it finds `frames.py:114`, the fd guard's `RuntimeError`, which the test allowlists. The pytest guard is the real check and passes.
- **Several catalogue rows are unreachable through the file they name.** The document check wraps the inner reason: a factor with no level, `replicates = 0`, a name with a space, an overlap of paths, an empty `any_of` and a bad id all print `experiment.document_invalid` or `suite.document_invalid`, never `experiment.factor_without_levels`, `experiment.count_zero`, `experiment.path_overlap` or `suite.list_empty`; the `format.*` rows I tried (a name, a secret name, a field path, the schema tag) print `experiment.document_invalid` too. The pages list those rows, since they are catalogue rows with an author side, and the Notes say what prints. If those codes are never raised, the owning leaves should drop them from the catalogue (the registry only grows, so they would stay in `codes.toml`).
- **`format.*`, `canonical.*` and `values.null` on the experiment page.** The leaf says the page of the document that carries them. The format rules apply to names in every document, and none of the four `format.*` codes I tried printed for an experiment file. They sit on the experiment page by the table; moving them is one line of the table.
- **An unknown key under `[budget]` reports path `/foo`**, not `/budget/foo`, and its message lists the root's keys. Found with `resolve`; the code is in `candler-schemas` or core, not here.
- **`protocol.version_mismatch`, `plugin.describe_mismatch` and the evaluator-side codes** were not tried by hand: a plug-in that answers `describe` needs the uv environment of a built plug-in. No Note claims them.

## Behaviour changes
None. No input is newly refused, no digest moved, no Rust source moved: `git diff --quiet a72342b..HEAD -- crates/candler/src crates/candler-core crates/candler-schemas crates/candler-harness crates/candler-harness-claude-code` holds. The helper's exception classes gain a keyword argument that the helper alone passes; a plug-in that constructs `OutcomeError` itself must now pass `refusal_code`.

## Re-bless after `cli_surface`
`cli_surface` appends `[cli]` rows to `codes.toml`. They enter the catalogue, so the index's Other refusals changes and needs one `CANDLER_BLESS_CONTRACT=1` run after that leaf lands; `cli.usage_invalid` is a caller mistake, and the surface `cli` has to be added to the table, or D3 fails naming it.

## Production
Commit timestamps: first commit 2026-11-12 07:19:03 +0900, last commit before this report 07:30:49 +0900, so about 12 minutes of commits after about 10 minutes of reading and probing from the base (07:09). The report commit follows. The build directory stayed under 10 GiB and is removed.
