# Candler

A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one operation API:

- **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).
- **Evaluation:** grade traces and artifacts, fresh or already on disk, through pluggable evaluators that emit one outcome record format.

A library, a lean CLI and an MCP server are thin surfaces over that API. Domain science (for example the HTN-creator evaluation) plugs in as evaluators, suite specs, fixtures and catalog providers. The core names no domain.

## Status

The design is in [`__design__/architecture.md`](__design__/architecture.md) and the milestones in [`__roadmap__/candler-v1/`](__roadmap__/candler-v1/README.md).

The wire schemas, the shared contracts, evaluation with plug-ins and execution in containers are built. The cargo workspace holds six crates:

- `candler-schemas`: every wire type of the `schemas@` line (outcome record, tool-catalog snapshot, raw frames, normalised trace, run manifest, experiment, suite, plug-in manifest, evaluator messages, job and child records, events), each with its JSON Schema committed under [`crates/candler-schemas/schema/`](crates/candler-schemas/schema/);
- `candler-core`: argument resolution, the job and process supervisors, the orchestrator, Docker isolation with its gateway sidecar, the run store, the legacy importer, and the evaluation engine with its registry, outcome selection and `diff`;
- `candler-harness`: the `HarnessDriver`, `TraceReader`, `SessionTracker` and `SkillTree` traits, and the fixture suite every driver runs;
- `candler-harness-claude-code`: the Claude Code driver and trace reader;
- `candler-harness-opencode`: the OpenCode driver and its export reader;
- `candler`: the CLI, with `run`, `resolve`, `ls`, `ps`, `kill`, `watch`, `ingest`, `inspect`, `evaluate`, `show`, `diff` and `analyze`.

The Python helper for evaluator plug-ins is in [`python/candler-evaluator/`](python/candler-evaluator/). Nothing is published to crates.io or PyPI.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the commit convention, versioning and branch workflow.

## License

[GNU Affero General Public License v3.0](LICENSE)
