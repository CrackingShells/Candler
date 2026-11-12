//! Candler harness for OpenCode V2.
//!
//! OpenCode runs as `opencode run --standalone --format json`, stores its sessions in SQLite and
//! hands them out as JSON through `opencode session export <id>`. The recorded runs that the
//! driver is written against are under `tests/fixtures/streams/`, and
//! `tests/fixtures/PROVENANCE.toml` names the binary they came from.
