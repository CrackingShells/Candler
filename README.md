# Candler

A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one operation API:

- **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).
- **Evaluation:** grade traces and artifacts, fresh or already on disk, through pluggable evaluators that emit one outcome record format.

A library, a lean CLI and an MCP server are thin surfaces over that API. Domain science (for example the HTN-creator evaluation) plugs in as evaluators, suite specs, fixtures and catalog providers. The core names no domain.

## Status

The design is in [`__design__/architecture.md`](__design__/architecture.md) and the milestones in [`__roadmap__/candler-v1/`](__roadmap__/candler-v1/README.md).

The cargo workspace holds five crates. `candler-schemas` defines the first two wire types, each with its JSON Schema committed under [`crates/candler-schemas/schema/`](crates/candler-schemas/schema/):

- the **outcome record**, the result every evaluator emits per run and expectation;
- the **tool-catalog snapshot**, the tools a run's MCP servers offered, in MCP `tools/list` shape.

`candler`, `candler-core`, `candler-harness` and `candler-harness-claude-code` are still empty. Nothing is published.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the commit convention, versioning and branch workflow.

## License

[GNU Affero General Public License v3.0](LICENSE)
