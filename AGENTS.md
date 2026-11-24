# Candler: rules for agents

Candler is a Rust workspace (`crates/`). These rules apply to every `.rs` file and every
document an agent writes here. Humans follow them too.

## What sane work means in Candler

The README promises provenance, per-run tool-catalog snapshots, and grading of artifacts "already on
disk". Those promises hold only if each kind of state Candler stores (a sealed run, its outputs, an
outcome, a catalog snapshot) has one producer, and that producer is Candler's own code. A test that
needs such a state asks Candler to make it, under a scripted agent if it must; it does not assemble
the state by hand, because a second producer can only agree with the first by luck. Where Candler
leans on something it does not own (a container daemon, a harness binary, a model), a stand-in is
fine for as long as a test that runs in CI holds it to the real thing. The README also promises
"thin surfaces over that API", so each input a consumer writes has one reader, below every surface,
and that reader refuses what it does not understand: a second, more lenient reading of the same
declaration is a defect even when nothing reaches it yet. A change is done when a consumer's path
through it has been seen working, and seen failing without it; how many tests or lines it moved says
nothing either way.

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
- [__roadmap__/leaf_template.md](__roadmap__/leaf_template.md): the shape of a roadmap leaf and the
  conventions behind it.
