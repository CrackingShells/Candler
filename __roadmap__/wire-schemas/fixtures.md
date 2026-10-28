# Fixtures: golden shape classes, real catalogs, MCP spec

**Goal**: The raw examples the record leaves test against are committed, with their provenance: golden rows chosen by shape class, two real catalogs, and MCP's own tool schema.
**Pre-conditions**:
- [x] `git -C /Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact rev-parse candler-m2a-golden^{commit}` prints `7efa1b7…`
- [x] `git -C /Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion rev-parse 'htn-creator-eval@0.4.2^{commit}'` prints `28be2c15…`
**Success Gates**:
- ⬜ Re-running `python3 -I crates/candler-schemas/tests/fixtures/legacy/select_golden.py` leaves `git status --porcelain crates/candler-schemas/tests/fixtures/` empty
- ⬜ `jq -r .status crates/candler-schemas/tests/fixtures/legacy/golden_rows.jsonl | sort -u` prints `blocked`, `fail`, `not_measured`, `pass`
- ⬜ `shasum -a 256 crates/candler-schemas/tests/fixtures/upstream/rikyu_mcp.json` prints `8e331d938505087e34839ba8d8d0b596032041af2dd1710315684c8d39f5b438`
- ⬜ `jq '[.servers[] | select((.tools | length) > 0)] | length' crates/candler-schemas/tests/fixtures/upstream/bio_opentrons.json` prints `4`
- ⬜ Every `crates/candler-schemas/tests/fixtures/spec/tools/*.json` parses with `jq -e .name`
**References**: [R01 §Golden corpus, §Wire encodings](../../__design__/architecture.md) — legacy mapping, test layers; [R03 Golden corpus](/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact); [R04 HTN eval](/Users/hacker/Documents/src/External/PCfVW/mcp-python-ingestion)

## Step 1: Select golden rows by shape class
**Goal**: The golden fixtures cover every shape the corpus has, picked by a script rather than by hand.
**Implementation Logic**:
`legacy/select_golden.py` uses only the standard library, runs with `python3 -I`, and reads `campaign/20260806-cont-train/` from tag `candler-m2a-golden` through `git show` and `git cat-file --batch`, skipping `._*` blobs. For each row of `expectations.jsonl` it computes the shape class:
- status;
- value kind: none, int or float;
- weight class from `score`/`max_score`: the status default, `0/0`, or another value;
- cause kind: none, a `blocked_by` file, a marker reason (`TIER_D_REPLAY: … reason=<code>`), or none readable.

It writes:
- `legacy/golden_rows.jsonl`: the first row of each class in corpus order, verbatim;
- `legacy/golden_grading.jsonl`: that row's verbatim entry from its run's `eval-N-grading.json`, keyed by cell, run and id;
- `legacy/golden_floats.json`: every distinct float `value`, in sorted order.

It prints each class with its corpus count. `legacy/README.md` records the repository, the tag, the commit, the paths and the command.
**Deliverables**: `crates/candler-schemas/tests/fixtures/legacy/{select_golden.py,golden_rows.jsonl,golden_grading.jsonl,golden_floats.json,README.md}`
**Consistency Checks**: `python3 -I crates/candler-schemas/tests/fixtures/legacy/select_golden.py && git diff --exit-code crates/candler-schemas/tests/fixtures/legacy/` (expected: PASS)
**Commit**: `test(schemas): select golden rows by shape class, not by hand`

## Step 2: Keep two real catalogs
**Goal**: Two snapshots captured from real MCP servers are available verbatim.
**Implementation Logic**:
1. `upstream/rikyu_mcp.json`: the file at tag `htn-creator-eval@0.4.2`, `.claude/skills/htn-creator-workspace/infrastructure/htn_creator_eval/catalogs/rikyu_mcp.json`, byte for byte.
2. `upstream/bio_opentrons.json`: captured from mcp-python-ingestion's `.claude/skills/htn-creator-workspace/servers/bio-opentrons/` (four servers: gripper, liquid, module, movement) with `infrastructure/scripts/capture_catalog_snapshot.py`. Run it from that repository's root with `uv run --isolated --no-project --python 3.12 --with 'mcp==<version the servers need>' python -I … --out <this fixture>`, as the script's docstring shows. If the script cannot load these low-level `mcp.server.Server` servers, change it on a topic branch in mcp-python-ingestion, in that repository's commit vocabulary, unpushed, and report it; it lands there only on Eliott's yes.
3. `upstream/README.md` records, for each file, the repository, the commit, the source paths, the server source digests and the exact capture command.
**Deliverables**: `crates/candler-schemas/tests/fixtures/upstream/{rikyu_mcp.json,bio_opentrons.json,README.md}`
**Consistency Checks**: `shasum -a 256 crates/candler-schemas/tests/fixtures/upstream/rikyu_mcp.json | grep -q '^8e331d938505087e34839ba8d8d0b596032041af2dd1710315684c8d39f5b438 '` (expected: PASS)
**Commit**: `test(schemas): keep the rikyu and bio-opentrons snapshots as real catalogs`

## Step 3: Vendor MCP's tool schema
**Goal**: The `Tool` definition of the latest published MCP specification, and tool objects drawn from it, are available offline.
**Implementation Logic**:
Fetch `schema/<version>/schema.json` of the latest published version from `modelcontextprotocol/modelcontextprotocol` once, with `gh api`, at a pinned commit, together with the repository's licence file. Eliott approved this one download with the plan; report its size and commit. Write `spec/tools/*.json`: one file per tool example given in the specification, plus `full.json`, a tool with every optional field the `Tool` definition has (`title`, `description`, `outputSchema`, `annotations`, `icons`, `_meta`, and any other). `spec/README.md` records the repository, the commit, the path, the version and the licence.
**Deliverables**: `crates/candler-schemas/tests/fixtures/spec/{mcp-<version>.schema.json,LICENSE,README.md,tools/*.json}`
**Consistency Checks**: `for f in crates/candler-schemas/tests/fixtures/spec/tools/*.json; do jq -e .name "$f" >/dev/null || exit 1; done` (expected: PASS)
**Commit**: `test(schemas): vendor the MCP tool schema the snapshot must accept`
