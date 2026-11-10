# Candler: rules for agents

Candler is a Rust workspace (`crates/`). These rules apply to every `.rs` file and every
document an agent writes here. Humans follow them too.

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
annotations. The `rust-conventions:rust-conventions-reviewer` agent reviews the Rust written,
at the points [Plans, gates and reviews](#plans-gates-and-reviews) sets.

The workspace lints in the root `Cargo.toml` (`[workspace.lints]`, inherited by every crate with
`[lints] workspace = true`) are the enforced subset of these rules. Passing them is necessary, not
sufficient: the rules above and the skill's checklist still apply where no lint reaches.

Rule 1 holds without exception: public functions that return a borrow name their lifetime
(`fn name<'a>(&'a self) -> &'a str`). `clippy::needless_lifetimes` is allowed workspace-wide for
that reason, so do not elide them.

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

## Plans, gates and reviews

- **A gate shows its failure first.** Each command a plan or a leaf gives as a gate is run once on
  the tree before the change, and the plan records the failure it shows there. A gate that already
  passes before the change tests nothing. A `git grep` pathspec over the crates' sources is
  `':(glob)crates/*/src/**'`; `'crates/*/src'` matches no file.
- **A mechanism finds the sites.** A change to every site of a pattern names what finds the sites:
  a lint, a `git grep` gate or a type. Its output is the list; sites are not counted by hand. A
  pattern that is open-ended or has come back gets such a mechanism. A single instance is fixed
  where it is, with no guard of its own.
- **Verification is per milestone.** Each leaf passes its own gates. The `rust-conventions-reviewer`
  reviews a leaf only when the leaf adds or changes a `pub` item. Before a milestone's pull request,
  one `spec-verifier` pass checks every leaf of the milestone, and the `rust-conventions-reviewer`
  reviews the milestone's whole diff. Adversarial verification runs where a milestone's leaf asks
  for it.
- **A review stays on the diff.** A reviewer reports what is wrong in the change and where it falls
  short of its spec. Outside the diff it reports a defect only: wrong behaviour, a broken invariant
  or a test that cannot fail. Any other finding outside the diff is one line, either in the
  planning item of the next planned milestone that edits that code or, when none does, in the
  campaign's knowledge-transfer report. It gets no pull request of its own.

## Where things are written down

- [CONTRIBUTING.md](CONTRIBUTING.md): commit messages, types and scopes, versioning, branches
  and integration.
- [__design__/architecture.md](__design__/architecture.md): the architecture, the crate layout
  (§Workspace) and the wire encodings (§Wire encodings).
