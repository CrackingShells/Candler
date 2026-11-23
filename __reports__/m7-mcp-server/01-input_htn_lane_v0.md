# M7 pre-study: input from the HTN lane (v0)

Date: 2026-11-23. Source: the HTN Phase coordinator session, at Eliott's request, sent to the Candler
milestone coordinator. It is one consumer's view, and the sender expects compromise. It is kept here
as evidence for `01-findings_agent_consumability_v1.md`, and lightly condensed.

## How HTN uses Candler today

The coordinator does not run Candler; its sessions do.

1. **"Can Candler do X?" before routing a gap to Candler.**
   - Sessions answer this by reading Rust source and guard tests.
   - Once the answer was wrong: "Candler installs only requirements.txt". The Dockerfile extension
     layer existed, but only `isolation/image.rs` showed it.
   - Guard tests named after their guarantee are good docs, but a poor discovery path.
2. **Briefing.** The coordinator copies version-dependent correctness facts into every brief, because
   a run's validity depends on the Candler commit. Examples:
   - "main ≥ 1e6dc91a, or mcp.* silently gave no servers";
   - the M6a F2–F4 caveats.
3. **Runs (Phase 4.5 probes, 3 runs).**
   - The session wrote `candler.toml` and `experiment.toml`.
   - It ran a debug build under `env -i`, with `CANDLER_STORE` and `XDG_STATE_HOME` in scratch,
     behind a `lockf` on a shared docker lock.
   - It proved the agent's world with real model turns:
     - probe 1: the agent lists every server's tools (4 servers, 36 tools);
     - probe 2: a planted canary evaluator input, then `find` and `grep` over `/`;
     - probe 3: the gate.
   - It wrote its own `stream.py` to decode `raw/stdout.jsonl.gz`.
   - It got the backing model from a separate RiVault script, because the stream records only the
     alias.
   - It removed `candler-env:*` images by hand, and measured spend from transcripts.
4. **Plug-in authoring.**
   - `[agent_environments.bio-opentrons]`: mocks as agent inputs, `mcp.servers.*`, requirements.
   - Next: per-skill-version dependencies.

## Automate in code (ranked by value to HTN)

1. **An agent-world probe with no model** (`candler probe <experiment> --cell …`, JSON). It builds
   the cell's container and reports:
   - the rendered `.mcp.json`;
   - for each server: connected or not, `tools/list` count, inputSchema digests;
   - mounted inputs with their audience;
   - `/opt/env` packages;
   - the treatment commit and tree;
   - a leak check that no evaluator-audience input is reachable, by path or digest.

   It replaces the 3 model probes and the canary, and would have caught the M2b-era "no servers" bug
   on day one.
2. **Resolved config with provenance** (`resolve --explain`): for each field, the layer that set it
   (plug-in environment, experiment, treatment), which build layers exist, and what came from
   `treatment:` sources.
3. **A field and capability reference as data, generated from code.** It holds:
   - every field path;
   - the allowed source forms (`treatment:<path>`);
   - the pairing rules (dockerfile+context and lock+requirements come from one layer);
   - the caveats (the backend must already be in the lock; install uses
     `--no-build-isolation --no-deps`).

   Served through `--json`, because copied prose rots.
4. **Run provenance and errata.**
   - Record the backing model the endpoint reports, not only the alias.
   - Record the Candler commit.
   - Keep an errata registry, so that `candler check <run|store>` flags runs made by versions with
     a known defect.
5. **Analysis output a reader can't misread.** Name effects by level labels, make the Holm decision
   the headline, and write a short text or markdown summary next to the JSON (M6a F2, F4, F5).
6. **Spend and tokens per run and per experiment**, plus a pre-run estimate (cells × replicates)
   that warns when n gives too wide an interval (n=3 gave ±28 %).
7. **Transcript rendering** (`show <run> --transcript`).
8. **`candler gc`** for a store's images, containers and networks, with a listing dry run. The run
   could own the docker lock, so callers don't wrap it in `lockf`.

## Keep authoring, with better guidance

- **Experiment design** (question, then factors, arms, controls, n): a skill that walks that path,
  plus more worked fixtures like `golden_design/experiment.toml`, one per common shape.
- **Agent-environment design** (agent versus evaluator audience; domain versus treatment
  dependencies): a checklist with the caveats and the rule "never put the grading reference in the
  build context or treatment".
- **Gap ownership** (Candler or the plug-in), and reading results under the caveats: notes that map
  a symptom to the layer that owns it.

## Form

- The base is the CLI with a stable `--json`, because agents drive Bash well.
- The skill is a pointer map plus decision checklists. It never copies the contract, and it injects
  live `--help` rather than curated flag lists.
- An MCP server only where it holds state an agent would otherwise rediscover: the store index,
  errata, resolved-config queries. A thin wrapper over `candler run` adds little.

## Keep as they are

- Guard tests named after the guarantee.
- The manifest's separate `evaluation_inputs`.
- Paths resolved relative to the experiment file.
- The importer declaring the treatment.

Evidence: the HTN coordinator's Phase 4.5 probe folder (PROBE_EVIDENCE.md, experiment TOMLs, decoded
transcripts), in that session's notes folder.
