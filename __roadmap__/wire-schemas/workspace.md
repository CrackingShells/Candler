# Workspace and schema foundation

**Goal**: The cargo workspace exists under Grit's lints, and `candler-schemas` holds the shared string formats and the schema drift check every wire type uses.
**Pre-conditions**:
- [x] The wire encodings are recorded in R01 §Wire encodings on `feat/wire-schemas`
- [ ] `cargo metadata --format-version 1 --no-deps | jq -r .workspace_root` prints this leaf's worktree path (after Step 1)
**Success Gates**:
- ⬜ `cargo build --workspace`, `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass
- ⬜ `grep -L 'workspace = true' crates/*/Cargo.toml` prints nothing
- ⬜ A scratch `.unwrap()` in `crates/candler-schemas/src/lib.rs` makes `cargo clippy -p candler-schemas -- -D warnings` fail; without it, clippy passes
- ⬜ `cargo nextest run -p candler-schemas primitives` passes, with an accepting and a rejecting test for every primitive
- ⬜ `cargo nextest run -p candler-schemas drift` passes, including the test that mutates a committed file of a test-only root and expects a failure, then blesses it and expects success
- ⬜ The answer to "does `wildcard_enum_match_arm = deny` fire on a downstream `_` arm over a `#[non_exhaustive]` enum?" is written into the campaign README's Gotchas
**References**: [R01 §Workspace, §Wire encodings](../../__design__/architecture.md) — crates, tooling, string formats, schema layout, drift check; [R05 Grit](https://github.com/PCfVW/Amphigraphic-Strict/tree/master/Grit) — `Cargo.toml` lints, `lib.rs`, `prompt.md`

## Step 1: Lay out the workspace
**Goal**: Five crates build under one set of Grit lints.
**Implementation Logic**:
1. Root `Cargo.toml`: `[workspace]` with members `crates/*` and resolver 3; `[workspace.package]` with version `0.1.0`, edition 2024, `rust-version = "1.99"`, license `AGPL-3.0-or-later`, repository `https://github.com/CrackingShells/Candler`.
2. `[workspace.dependencies]`: `serde` (derive), `serde_json` (`float_roundtrip`, no `preserve_order`), `schemars` 1.x, `thiserror`, `semver`, `uuid`, `jiff`; dev-only `jsonschema` and `proptest`.
3. `[workspace.lints.rust]` and `[workspace.lints.clippy]`: Grit's `Cargo.toml` template verbatim, plus `missing_docs = "warn"` from its `lib.rs` template. Every crate declares `[lints] workspace = true`; no crate repeats the lints in a `#![deny]` header.
4. `rust-toolchain.toml` (channel `1.99.0`, components `rustfmt`, `clippy`); `rustfmt.toml` (edition 2024); `clippy.toml` with `allow-unwrap-in-tests`, `allow-expect-in-tests`, `allow-panic-in-tests`, `allow-indexing-slicing-in-tests`.
5. Crates under `crates/`: `candler` (binary with an empty `main`), `candler-core`, `candler-harness`, `candler-harness-claude-code` (libraries with only a `//!` crate doc), and `candler-schemas` (library). The crate docs say what each crate holds, per R01 §Workspace.
6. Check how `wildcard_enum_match_arm` treats a `_` arm over a `#[non_exhaustive]` enum from another crate (a throwaway match in `candler-core` against a test enum in `candler-schemas`, removed before the commit) and record the answer in the campaign README's Gotchas.
**Deliverables**: `Cargo.toml` (`[workspace]`, `[workspace.package]`, `[workspace.dependencies]`, `[workspace.lints]`); `rust-toolchain.toml`; `rustfmt.toml`; `clippy.toml`; `crates/candler/{Cargo.toml,src/main.rs}`; `crates/candler-core/{Cargo.toml,src/lib.rs}`; `crates/candler-harness/{Cargo.toml,src/lib.rs}`; `crates/candler-harness-claude-code/{Cargo.toml,src/lib.rs}`; `crates/candler-schemas/{Cargo.toml,src/lib.rs}`; `Cargo.lock`
**Consistency Checks**: `cargo clippy --workspace --all-targets -- -D warnings` (expected: PASS)
**Commit**: `build(repo): lay out the cargo workspace under Grit's lints`

## Step 2: Fix the shared string formats
**Goal**: Every string format of R01 §Wire encodings is a validated newtype.
**Implementation Logic**:
Module `candler_schemas::primitives`. Each type is a newtype with a private field, built through `FromStr` and `TryFrom<String>`, serialised with `#[serde(try_from = "String", into = "String")]`, exposed through `as_str()`, and given a `JsonSchema` impl whose `pattern` matches its parser. Parsing uses `split_once` and `strip_prefix`, never indexing. Errors are one `thiserror` enum, `FormatError`, worded `"<noun> <problem> (<context>)"`.
- `Digest`: `sha256:` + 64 lowercase hex.
- `InstrumentDigest`: a `Digest`, or `git:` + 40 or 64 lowercase hex.
- `NameVersion` (SemVer 2.0 version) and `NameToken` (free version token `[0-9A-Za-z.+-]+`): split at the last `@`; the name is non-empty, without `@` or whitespace.
- `RelPath`: POSIX, non-empty, no leading `/`, no `.` or `..` segment, no `\`.
- `Timestamp`: RFC 3339 with an explicit offset, checked with `jiff`; the lexical form is kept.
- `RunKey { cell: Digest, replicate: u32, attempt: u32 }`: both counters ≥ 1. Serialised as an object; `Display` and `FromStr` give the path form `<cell hex>-r<replicate>-a<attempt>`.
- `RequestId`: UUIDv7, lowercase hyphenated.
- `EvidenceRef`: `event:<session-id>#<seq>` or `path:<RelPath>` with an optional `#L<n>` or `#L<n>-<m>`, where `n ≤ m`.
- `ExpectationId`: `^[A-Za-z0-9][A-Za-z0-9._-]*$`.
- `ReasonCode`: `^[a-z][a-z0-9_]*$`.

Each type has tests that accept its valid forms and reject each malformed form, and that round-trip `RunKey` through both its forms.
**Deliverables**: `crates/candler-schemas/src/primitives.rs` (`Digest`, `InstrumentDigest`, `NameVersion`, `NameToken`, `RelPath`, `Timestamp`, `RunKey`, `RequestId`, `EvidenceRef`, `ExpectationId`, `ReasonCode`, `FormatError`); `crates/candler-schemas/src/lib.rs` (`pub mod primitives`)
**Consistency Checks**: `cargo nextest run -p candler-schemas primitives` (expected: PASS)
**Commit**: `feat(schemas): fix the string formats every record shares`

## Step 3: Check committed schemas against the structs
**Goal**: A committed schema file that differs from its generated form fails the test suite.
**Implementation Logic**:
1. `candler_schemas::generated_schemas() -> Vec<GeneratedSchema>`, where `GeneratedSchema { file_name, schema }` is one root. The list is empty until the record leaves add their roots.
2. One generator configuration: JSON Schema draft 2020-12, `Option` fields without a null type, `$id` = `urn:candler:schemas:<name>`, `$defs` inline, no crate version in the output. Files are written as 2-space-indented JSON with a trailing newline.
3. `tests/schema_drift.rs`: for every root, compare the generated text byte for byte with `crates/candler-schemas/schema/<file_name>`; with `CANDLER_BLESS_SCHEMAS=1`, write the file instead. The comparison and the write are one function, `check_or_bless(root, dir, bless)`, so a unit test can run it on a test-only root in a temporary directory: it passes on a fresh bless, fails after the file is mutated, and passes again after a second bless.
**Deliverables**: `crates/candler-schemas/src/generate.rs` (`GeneratedSchema`, `generated_schemas`, `schema_settings`, `check_or_bless`); `crates/candler-schemas/tests/schema_drift.rs`; `crates/candler-schemas/schema/` (created, empty but for `.gitkeep`)
**Consistency Checks**: `cargo nextest run -p candler-schemas drift` (expected: PASS)
**Commit**: `feat(schemas): fail the build when a committed schema drifts from its struct`

## Step 4: Give agents the Rust rules
**Goal**: Every agent working in the repository finds Grit's rules and the source-file rule without being told.
**Implementation Logic**:
`AGENTS.md` holds: Grit's 12-rule prompt from its `prompt.md`; the source-file rule (source says what is: no history, origin or meta-discourse in comments, docs, `#[deprecated(note)]` or `$comment`; reasons go to `__design__/` or `__reports__/`); the `rust-conventions` skill and its `rust-conventions:rust-conventions-reviewer` agent; that the workspace lints are the enforced subset; and pointers to CONTRIBUTING and R01. `CLAUDE.md` contains only a pointer to `AGENTS.md`.
**Deliverables**: `AGENTS.md`; `CLAUDE.md`
**Consistency Checks**: `grep -q 'rust-conventions' AGENTS.md && grep -q 'AGENTS.md' CLAUDE.md` (expected: PASS)
**Commit**: `docs(contributing): give agents Grit's rules for Rust`
