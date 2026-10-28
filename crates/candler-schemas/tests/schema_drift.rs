//! The committed schema files match the schemas generated from the structs, and no committed
//! schema file lacks a struct.
//!
//! Setting `CANDLER_BLESS_SCHEMAS=1` rewrites the files instead of comparing them.

use std::path::{Path, PathBuf};

use candler_schemas::generate::{BLESS_ENV, check_or_bless, generated_schemas, unexpected_files};

fn schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("schema")
}

#[test]
fn committed_schemas_have_no_drift() {
    let bless = std::env::var(BLESS_ENV).is_ok_and(|value| value == "1");
    for root in generated_schemas() {
        if let Err(e) = check_or_bless(&root, &schema_dir(), bless) {
            panic!("{e}");
        }
    }
}

#[test]
fn every_committed_schema_file_has_a_root() {
    match unexpected_files(&generated_schemas(), &schema_dir()) {
        Ok(stale) => assert!(
            stale.is_empty(),
            "schema files without a generated root, delete them: {stale:?}"
        ),
        Err(e) => panic!("{e}"),
    }
}
