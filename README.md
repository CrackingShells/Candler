# Candler

A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one operation API:

- **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation, per-run tool-catalog snapshots, provenance).
- **Evaluation:** grade traces and artifacts, fresh or already on disk, through pluggable evaluators that emit one outcome record format.

A library, a lean CLI and an MCP server are thin surfaces over that API. Domain science (for example the HTN-creator evaluation) plugs in as evaluators, suite specs, fixtures and catalog providers. The core names no domain.

## Status

Design stage. Nothing is implemented yet.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the commit convention, versioning and branch workflow.

## License

[GNU Affero General Public License v3.0](LICENSE)
