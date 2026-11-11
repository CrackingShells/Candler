//! Support of the contract-page guards: the walk of a committed schema, the renderer of a page and
//! of the index, the frame check and the version blocks. Everything is read from the committed
//! schema files and the three manifests, never from the Rust types.

pub mod frame;
pub mod hand;
pub mod index;
pub mod pages;
pub mod schema;
pub mod versions;

use std::fs;
use std::path::{Path, PathBuf};

/// The environment variable that makes the guards write the pages instead of comparing them.
pub const BLESS_ENV: &str = "CANDLER_BLESS_CONTRACT";

/// The workspace root.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The committed schema files.
pub fn schema_dir() -> PathBuf {
    repo_root().join("crates/candler-schemas/schema")
}

/// The contract pages.
pub fn docs_dir() -> PathBuf {
    repo_root().join("docs/contract")
}

/// Whether the guards write instead of compare.
pub fn blessing() -> bool {
    std::env::var(BLESS_ENV).is_ok_and(|value| value == "1")
}

/// Writes the file through a rename, so a guard reading it in another process never sees half.
pub fn write_atomically(path: &Path, text: &str) -> Result<(), String> {
    let temporary = path.with_extension(format!("md.{}.tmp", std::process::id()));
    fs::write(&temporary, text).map_err(|e| format!("{}: {e}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|e| format!("{}: {e}", path.display()))
}
