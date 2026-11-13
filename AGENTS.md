# Candler: rules for agents

Candler is a Rust workspace (`crates/`). These rules apply to every `.rs` file and every
document an agent writes here. Humans follow them too.

## Sustainable, scalable, efficient

Everything produced or run here passes three tests. That covers the code, its tests and CI, every
script and document, and the work itself: each build, test run, model call and agent run. Nothing
runs in a silo. All of it shares the hardware, the quotas and the energy with other software and
other people.

- **Sustainable:** a small RSS, CPU and GPU footprint, and with it disk, tokens and energy. What is
  created has an owner that removes it.
- **Scalable:** code design patterns, known algorithms, easy-to-use scripts, DRY. One owner per
  invariant, and one source per fact.
- **Efficient:** the smallest wall time. Think about how to be fast by default and how to use what
  the hardware offers, in the spirit of Casey Muratori's Better Software manifesto. Don't
  over-optimise.

Before writing, running, reviewing or dispatching anything, answer:
1. What does it cost the shared machine and its energy?
2. Is it built from known patterns, algorithms and scripts, with nothing duplicated?
3. Is it fast by default here?

A review checks these three alongside correctness.

Cheap is not a reason to pay. A cost that looks negligible still counts: a free model call, an
extra agent, a rerun, one more check. Such flaws compound across the whole environment, and taking
the trouble to find the sustainable, scalable and efficient way is part of the work.

These fail the tests:
- band-aids;
- defensive checks against the repository's own code;
- the same validation at a second layer;
- a special case kept for a fixture;
- a test added per incident instead of a fix at the root.

## Rust: Grit

The Rust conventions are [Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit).
When writing Rust, follow Grit (strict Rust) rules:

1. ADD explicit lifetimes to all public function signatures
2. USE explicit conversions (.as_str(), .as_bytes(), .to_owned()) instead of relying on Deref
3. NEVER use .unwrap() or .expect() in library code - propagate errors with ?
4. NEVER use Box<dyn Any> - use generics or concrete types
5. ISOLATE unsafe code in dedicated modules with SAFETY comments
6. USE owned types (String, Vec<T>) in async function signatures, not references
7. MATCH all enum variants explicitly - never use _ catch-all without documenting why
8. USE thiserror for library error types, anyhow for applications
9. PREFER iterator chains (.iter().map().collect()) over imperative loops for transformations
10. STANDARDIZE on tokio for async runtime - never mix runtimes
11. ADD #[non_exhaustive] to public enums that may gain variants
12. ADD #[must_use] to public functions that return a value with no side effects

Before writing or editing any `.rs` file, load the `rust-conventions` skill
(`rust-conventions:rust-conventions`) and follow its trigger checklist: doc comments, `# Errors`
sections, `#[must_use]`, `#[non_exhaustive]`, and the `// CAST:`, `// INDEX:` and `// BORROW:`
annotations. After writing Rust, have the `rust-conventions:rust-conventions-reviewer` agent
review it.

The workspace lints in the root `Cargo.toml` (`[workspace.lints]`, inherited by every crate with
`[lints] workspace = true`) are the enforced subset of these rules. Passing them is necessary, not
sufficient: the rules above and the skill's checklist still apply where no lint reaches.

Rule 1 holds without exception: public functions that return a borrow name their lifetime
(`fn name<'a>(&'a self) -> &'a str`). `clippy::needless_lifetimes` is allowed workspace-wide for
that reason, so do not elide them.

A local lint suppression is `#[expect(<lint>, reason = "…")]`, so it fails once the lint stops
firing. `#[allow]` is denied in `src/`. In `tests/` it is allowed, with a reason, only where an
`expect` cannot hold, such as a support module shared by several test binaries. Grit's markers
(`EXHAUSTIVE:`, `WIRE:`, `CAST:`) open the reason or stay beside it.

A crate's modules are its namespace: a crate root declares its modules and does not re-export their
items, and callers write `candler_schemas::primitives::Digest`.

## Source files say what is

Comments, doc comments, `#[deprecated(note)]` and schema `$comment`s describe the code as it is.
They carry no history, origin, "legacy" or "previously", no roadmap or plan references, and no
other meta-discourse; the code reads as if it had always been intended that way. The reasons
behind a decision go to `__design__/` or `__reports__/`.

## Checks

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The JSON Schema files under `crates/candler-schemas/schema/` are generated from the wire structs
and committed. A struct or doc-comment edit that changes them fails the drift test until the
files are regenerated with `CANDLER_BLESS_SCHEMAS=1 cargo nextest run -p candler-schemas drift`.

## Where things are written down

- [CONTRIBUTING.md](CONTRIBUTING.md): commit messages, types and scopes, versioning, branches
  and integration.
- [__design__/architecture.md](__design__/architecture.md): the architecture, the crate layout
  (§Workspace) and the wire encodings (§Wire encodings).
