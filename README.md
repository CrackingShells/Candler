# Candler

A domain-agnostic core for evaluating LLM-based agents. Candler has two equal halves behind one
operation API:

- **Execution:** run agents under controlled conditions (experiment matrix, runners, isolation,
  per-run tool-catalog snapshots, provenance).
- **Evaluation:** grade traces and artifacts, fresh or already on disk, through pluggable
  evaluators that emit one outcome record format.

A library, a lean CLI and an MCP server are thin surfaces over that API. Domain science (for
example the HTN-creator evaluation) plugs in as evaluators, suite specs, fixtures and catalog
providers. The core names no domain.

Candler runs and grades LLM-based agents under controlled conditions, and compares the results
with statistics that say what they can and cannot support.

## What it is, and what it is not

You describe an experiment in one file: the models, the agent harness, the treatment under test
(for example a skill), the budget and the factors that vary. Candler runs every cell of that
matrix in an isolated container, stores each run, grades the runs with evaluators you plug in,
and reports rates, intervals and comparisons.

It is a tool for people and agents who need a result they can defend, not a benchmark suite, a
leaderboard or a hosted service. It names no domain: the science you evaluate arrives as
evaluator plug-ins, suites and fixtures.

## Main features

- **One experiment file.** Factors, replicates, budget, harness and treatment are declared in
  TOML; every value Candler uses is shown with the source it came from.
- **Isolated runs.** Each run executes in its own Docker container, with the tools and network
  the experiment declares and a per-run budget.
- **Provenance.** Each run records what produced it, so two runs are compared only when their
  conditions are comparable.
- **Plug-in grading.** Evaluators are separate programs that speak a small protocol; a Python
  helper makes writing one short.
- **Honest statistics.** Wilson and Newcombe intervals, exact and stratified tests, a multiplicity
  correction, and a refusal to compare what should not be compared.
- **Resumable and safe to interrupt.** Runs are content-addressed and a job picks up where it
  stopped.
- **Agent-friendly surfaces.** Every command prints a versioned JSON record by default and text
  on request, and every refusal names its code.

## Quick start

Candler is not published yet. Build it from source with a recent Rust toolchain:

```sh
git clone https://github.com/CrackingShells/Candler.git
cd Candler
cargo build -p candler
./target/debug/candler --help
```

List the experiment files of a fixture directory, which starts nothing and spends nothing:

```sh
./target/debug/candler ls --format text crates/candler/tests/fixtures/observe/ls/experiments
```

Each experiment file is listed, and a file that is not an experiment is listed as skipped with
the reason. Running needs Docker, and so does `candler resolve <experiment>`,
which prints every value an experiment comes to and checks that its treatment, harness and model
gateway are usable; a real experiment names a treatment repository and a model endpoint.

## Learn more

- [The contract pages](docs/contract/index.md): every file format, field and refusal code.
- [The default statistics](docs/statistics.md): what each method answers and where it stops
  being valid.
- `candler --help` and `candler <command> --help`.

## Status

Candler is at `0.y.z` and under active development; expect changes.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the commit convention, versioning and branch workflow,
and [AGENTS.md](AGENTS.md) for the rules agents follow.

## License

[GNU Affero General Public License v3.0](LICENSE)
