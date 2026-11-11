# Docs refusals

**Goal**: Each contract page lists the refusals of its surface, generated from the code's catalog and the helper's registry, and carries a few tentative notes, each true at its stamped version; the helper's own errors carry the codes of `codes.toml [helper]`.
**Pre-conditions**:
- [ ] [cmd] The leaf's worktree is its own checkout: `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints the worktree path
- [ ] [cmd] The frame and the pages are in: `test -f crates/candler/tests/contract_pages.rs && test -f docs/contract/index.md && test -f docs/contract/experiment-file.md && grep -q '^## Refusals' docs/contract/experiment-file.md`
- [ ] [cmd] The code's catalog is complete: `git grep -q 'pub fn catalog' -- crates/candler-core/src/refusal.rs && ! git grep -n -i 'pending' -- crates/candler-core/tests/guard_refusal_codes.rs`
- [ ] [cmd] The registry has a helper section: `grep -q '^\[helper\]' docs/contract/codes.toml`
- [ ] [cmd] The helper's raise sites exist as the leaf expects: `git grep -q 'class OutcomeError' -- python/candler-evaluator/src/candler_evaluator/plugin.py && git grep -q 'class IdentityError' -- python/candler-evaluator/src/candler_evaluator/identity.py`
- [ ] [nl] OWNED: `docs/contract/*.md` (the Refusals and Notes sections, and the index's generated list of other refusals), `crates/candler/tests/contract_pages.rs` and `crates/candler/tests/support/contract_pages/**`, `docs/contract/codes.toml` (`[helper]` only), `python/candler-evaluator/src/candler_evaluator/{plugin,frames,identity,loop}.py`, `python/candler-evaluator/tests/test_guard_codes.py`. `driver-traits.md` only through Step 5, and only when it exists
**Success Gates**:
- ⬜ [cmd] OWNED: `test -z "$(git diff --name-only <base>..HEAD | grep -vE '^(docs/contract/[a-z-]+\.md$|docs/contract/codes\.toml$|crates/candler/tests/(contract_pages\.rs|support/contract_pages/)|python/candler-evaluator/src/candler_evaluator/(plugin|frames|identity|loop)\.py$|python/candler-evaluator/tests/test_guard_codes\.py$)')"`, `<base>` being the leaf's branch point
- ⬜ [cmd] D1 holds with the filled sections, and D2 with the notes: `cargo nextest run -p candler --no-tests=fail -E 'test(/^d1_/)|test(/^d2_/)'` and `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)' && test -z "$(git status --porcelain docs/contract)"`
- ⬜ [cmd] D3 codes on pages: `cargo nextest run -p candler --no-tests=fail -E 'test(/^d3_/)'`
- ⬜ [cmd] D4, the helper half (pytest): `(cd python/candler-evaluator && uv run --frozen pytest -q -k d4_ tests/test_guard_codes.py)`, then the whole helper suite, lint and format: `(cd python/candler-evaluator && uv run --frozen pytest -q && uv run --frozen ruff check && uv run --frozen ruff format --check)`
- ⬜ [cmd] G-codes: `git diff --numstat a26fe53 -- docs/contract/codes.toml | awk '$2!=0{b=1}END{exit b}'`
- ⬜ [cmd] The helper raises a coded error or a listed standard one only: `! git grep -n -E '^\s*raise (RuntimeError|ValueError|TypeError)\(' -- 'python/candler-evaluator/src/candler_evaluator/{plugin,frames,identity,loop}.py'` (the allowlist is the test's, one line each, with a reason)
- ⬜ [cmd] No development process in the pages: `! grep -n -i -E 'milestone|roadmap|amendment|leaf|campaign|\bM[0-9]\b|\bA5\b' docs/contract/*.md`
- ⬜ [cmd] If the driver page exists, the generator covers it: `if test -f docs/contract/driver-traits.md; then CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)' && test -z "$(git status --porcelain docs/contract)"; fi`
- ⬜ [cmd] No Rust source outside the tests moved: `git diff --quiet <base>..HEAD -- crates/candler/src crates/candler-core crates/candler-schemas crates/candler-harness crates/candler-harness-claude-code`
- ⬜ [cmd] STD (the `-p candler` forms) and SUBJ pass
- ⬜ [nl] The leaf report records the bites:
  - D3: a code deleted from the catalog but left in a page; an author code with no page or index entry;
  - D4 helper: a bare `raise ValueError(...)` added; a code raised that `[helper]` lacks; a `[helper]` code no raise site uses
- ⬜ [nl] The `docs_refusals` verifier (R10 §6) closes with no open finding. For each of the six pages it takes five claims (at least one field class, one refusal and one Note), builds a hand input for each and tries it against the real binary or helper, and reports each as confirmed or false. A false Note is rewritten or deleted before the merge; a stamp is the version it was tried against
**References**: [R03 frame: Refusals, Notes, checks](../../../../__reports__/contract-docs/gap_analysis_contract_docs_v0.md); [R10 §2 D5, §5 D2–D4, §6](/Users/hacker/Library/Application%20Support/Claude/side-session-notes/local_7f577b41-1698-4eeb-9de3-4ee9460ae606/m3-approved-plan.md); `__roadmap__/m3-lean-cli/verbs/docs_frame.md` (the generator, the stamp format, the markers); `__roadmap__/m3-lean-cli/verbs/surfaces/refusal_late.md`; campaign README §Gotchas, "Shared constraints" (`codes.toml`), "M4 overlap" (the driver page)

## Step 1: Register the helper's codes
**Goal**: `[helper]` holds a row for every error the helper can raise at a plug-in author, written before the raise sites change.
**Implementation Logic**:
1. Inventory the raise sites by reading them, not by grep alone: `plugin.py` (19, `OutcomeError` for a kind's result, plus a `TypeError` for a module with no `KINDS`), `frames.py` (`ProtocolError`, `Refused` and a `RuntimeError` for the fd guard), `identity.py` (9 `IdentityError`), `loop.py` (`_ParamsError`, a `ValueError` for a plug-in with no name or version), and `__init__.py` (an `AttributeError` for an unknown attribute, which stays a standard one).
2. Write one row per distinct reason in `[helper]`: code `helper.<reason>`, side (`plugin` for what the author wrote, `candler` where Candler's own request is malformed), and, because the helper has no Rust catalog, a `when` sentence and an `at` sentence by hand, as the schemas' macro rows have. The reason names the cause (`helper.outcome_cause_missing`), never the function. Rows are append-only.
3. Two raise sites with one reason share a row; a site that is not an author's (`the fd guard is already installed`, the module `__getattr__`) is listed in the test's allowlist in Step 2, not in `[helper]`.
**Deliverables**: `docs/contract/codes.toml`: `[helper]` rows
**Consistency Checks**: `git diff --quiet HEAD -- crates python && git diff --numstat HEAD -- docs/contract/codes.toml | awk '$2!=0{b=1}END{exit b}'` (expected: PASS)
**Commit**: `docs(evaluator-helper): register the codes of the helper's refusals`

## Step 2: Give every raise a code, guard first
**Goal**: The helper's raise sites carry a code from `[helper]`, and a pytest guard fails on any raise that does not.
**Implementation Logic**:
1. Write `python/candler-evaluator/tests/test_guard_codes.py` first, and see it red against the unchanged sources:
   - `test_d4_every_raise_carries_a_listed_code`: parse `src/candler_evaluator/*.py` (not `_models/`) with `ast`; every `raise` of a call must be an exception class that takes a code, passing a string literal that is a key of `[helper]`; a bare `raise` re-raising a caught error is allowed, as is a raise in the allowlist (the module `__getattr__` and the fd-guard `RuntimeError`, each with a reason);
   - `test_d4_every_helper_code_is_raised`: every `[helper]` row has a raise site.
   Read `docs/contract/codes.toml` with `tomllib` (`tomli` on Python 3.10, which pytest already pulls in). Oracle: `[helper]`, hand-written in Step 1. Each test's docstring has `Guards:`, `Oracle:`, `Bite:` lines.
2. Give the helper's exception classes a `refusal_code` argument and attribute: `OutcomeError`, `IdentityError`, `ProtocolError`, `_ParamsError`. Do not use `code`: `Refused` already has an integer `code`, the JSON-RPC error. Add a code to each raise site. `str(error)` stays what it is, so no existing assertion changes; an existing test that fails is reported, not edited. Where the helper already writes an error reply, the code goes into its `data` field only if no existing vector changes; otherwise it stays an attribute and the report says so.
3. Replace the bare `ValueError`, `TypeError` at `loop.py` and `plugin.py` that an author causes with a coded class; leave the allowlisted ones.
4. Run the whole helper suite, `ruff check` and `ruff format --check`; the generated `_models/` are untouched.
**Deliverables**: `python/candler-evaluator/src/candler_evaluator/plugin.py`: `OutcomeError`, the raise sites; `python/candler-evaluator/src/candler_evaluator/frames.py`: `ProtocolError`, the raise sites; `python/candler-evaluator/src/candler_evaluator/identity.py`: `IdentityError`, the raise sites; `python/candler-evaluator/src/candler_evaluator/loop.py`: `_ParamsError`, the raise sites; `python/candler-evaluator/tests/test_guard_codes.py`: `test_d4_every_raise_carries_a_listed_code`, `test_d4_every_helper_code_is_raised`
**Consistency Checks**: `(cd python/candler-evaluator && uv run --frozen pytest -q && uv run --frozen ruff check && uv run --frozen ruff format --check)` (expected: PASS)
**Commit**: `feat(evaluator-helper): carry a registered code on every refusal`

## Step 3: The Refusals sections
**Goal**: Each page's Refusals section lists the codes of its surface, generated, and every author code is on a page or in the index.
**Implementation Logic**:
1. Write D3 first (`d3_…`, in `contract_pages.rs`) and see it red against the empty placeholders:
   - `d3_every_code_on_a_page_is_in_the_catalog`: every code in backticks in a Refusals section or a Note is a row of `candler_core::refusal::catalog()` or of `[helper]`;
   - `d3_every_author_code_has_a_page`: every catalog code whose side is not `candler`, and every `[helper]` code, appears on one page's Refusals section, or in the index's list of other refusals (item 3). Oracle: the catalog, and `codes.toml` for the helper.
2. Refusals rows: code, when (the row's `when`), path at fault (the row's `at`), side to fix. Sorted by code. Generated between the Refusals markers; D1 compares them.
3. **Which page.** The six pages cover the surfaces `manifest`, `suite`, `protocol`, `outcome`, `experiment` and `helper`. A hand table in the support module maps each code's surface to a page; the shared `format`, `canonical` and `values` surfaces map to the page of the document that carries them. The author-facing surfaces that name no page (resolve, runner, isolation, store, supervise, frames, config, the harnesses, ingest and the rest the catalog holds) are listed in a generated `## Other refusals` section of `index.md`, under the same columns. A surface in the catalog that the table does not name fails D3 with its name. Report to the coordinator which surfaces landed in the index, since the frame says a page per surface.
4. Generate with `CANDLER_BLESS_CONTRACT=1`, then read each page: a code that looks wrong is a defect of the row in the catalog (report it to the leaf that owns it), never edited in the page.
**Deliverables**: `crates/candler/tests/contract_pages.rs`: `d3_every_code_on_a_page_is_in_the_catalog`, `d3_every_author_code_has_a_page`; `crates/candler/tests/support/contract_pages/`: the Refusals renderer, the surface-to-page table, the index's other-refusals renderer; `docs/contract/*.md` (Refusals sections, index)
**Consistency Checks**: `CANDLER_BLESS_CONTRACT=1 cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)' && cargo nextest run -p candler --no-tests=fail -E 'test(/^d1_/)|test(/^d2_/)|test(/^d3_/)'` (expected: PASS)
**Commit**: `docs(contributing): list the refusals of each contract page`

## Step 4: The tentative Notes
**Goal**: Each page holds the rules its tables cannot state, in at most 120 words, each note stamped with the version it was checked against.
**Implementation Logic**:
1. For each page, write the few rules a table cannot state: for example that a manifest's `command` is relative to the plug-in directory, that suite entries form a graph with no cycle, that a pinned input is hashed. Take each from code or a schema `$comment`, and try it with a hand input against the binary or the helper before it stays. A rule you cannot try is not written.
2. Stamp each note `[checked: <line>@<version>]` with the version in the manifest at the commit (the stamp format and cap are D2's). No tutorial, no narrative, no analogy, no process.
3. Run D2: the cap and the stamps hold for every page. Do not touch generated sections.
**Deliverables**: `docs/contract/{plugin-manifest,suite-file,evaluator-protocol,outcome-record,experiment-file,evaluator-helper}.md`: Notes sections
**Consistency Checks**: `cargo nextest run -p candler --no-tests=fail -E 'test(/^d1_/)|test(/^d2_/)|test(/^d3_/)'` (expected: PASS)
**Commit**: `docs(contributing): add tentative notes to the contract pages`

## Step 5: Fold in the driver page, only if it exists
**Goal**: If the driver milestone merged first and its hand-written `docs/contract/driver-traits.md` is on the branch, the generator owns it from here; otherwise this step is empty and is skipped.
**Implementation Logic**:
1. `test -f docs/contract/driver-traits.md`. If it is absent, write "skipped: the driver page is not on this branch" in the leaf report and make no commit.
2. If present: read its drift guard (it compares the page with `candler-harness`'s trait items) and keep that guard's assertions working or move them into `contract_pages.rs` as one more page of the generator, with its five sections, its Fields from the trait items and its Refusals from the harness crates' rows. Keep the guard's name where the driver milestone's tests refer to it. Remove the index's "reserved" wording for the row.
3. Run D1 to D4 and the driver guard, unchanged in what it asserts.
**Deliverables**: `crates/candler/tests/contract_pages.rs`: the driver page's generator; `docs/contract/driver-traits.md`; `docs/contract/index.md`
**Consistency Checks**: `if test -f docs/contract/driver-traits.md; then cargo nextest run -p candler --no-tests=fail -E 'binary(contract_pages)'; else true; fi` (expected: PASS)
**Commit**: `docs(contributing): generate the driver-traits page with the rest`
