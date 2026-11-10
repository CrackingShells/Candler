//! Records which build of the evaluation infrastructure this binary is, from the origin the
//! build proves, tried in this order:
//!
//! 1. **A git checkout** records the tag nearest to the commit and the commits since it, the full
//!    40-hex commit, and whether the sources the binary is built from differ from it.
//! 2. **A packaged crate** (crates.io or `cargo package`) records the commit of its
//!    `.cargo_vcs_info.json` and whether that file marks the tree dirty, with the tag
//!    `v<package version>`. The distance is not recorded.
//! 3. **The packager** records `CANDLER_EVAL_INFRA_TAG`, `CANDLER_EVAL_INFRA_COMMIT` and
//!    `CANDLER_EVAL_INFRA_DIRTY` as set in the build's environment. A name left unset is not
//!    recorded, except that the tag falls back to `v<package version>`.
//! 4. **Otherwise** only the tag `v<package version>` is recorded.
//!
//! A commit or flag that no origin gives is left unrecorded, never recorded as `false`. Every
//! field is exported under a `CANDLER_BUILT_*` name, empty when unrecorded, so that nothing in the
//! builder's shell can stand in for it; the packager's `CANDLER_EVAL_INFRA_*` names are inputs
//! only. The resolution is `build_infra.rs`, shared with the guard that tests it.

use std::path::Path;

#[path = "build_infra.rs"]
#[expect(
    dead_code,
    reason = "the read side of the identity belongs to the binary, not the build"
)]
mod build_infra;

use build_infra::{Origin, PACKAGER_VARS, WATCHED, git, resolve};

/// Hands one value to the compiler as an environment variable of the build.
fn export(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

fn main() {
    let dir = Path::new(".");
    // The build is stale when the commit moves (HEAD, the branch ref HEAD names, packed refs) or
    // the index changes, or when anything watched changes.
    // BORROW: .to_owned() makes the owned paths the list keeps
    let mut git_paths = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
    ];
    if let Some(branch) = git(dir, &["symbolic-ref", "-q", "HEAD"]) {
        git_paths.push(branch);
    }
    for path in &git_paths {
        if let Some(found) = git(dir, &["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={found}");
        }
    }
    for path in WATCHED {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-changed=.cargo_vcs_info.json");
    for name in PACKAGER_VARS {
        println!("cargo:rerun-if-env-changed={name}");
    }

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let identity = resolve(dir, &version, |name| std::env::var(name).ok());
    if identity.origin == Origin::None {
        println!(
            "cargo:warning=candler is not built from a git checkout, a packaged crate or \
             CANDLER_EVAL_INFRA_* variables: only its version is recorded, and a stratum that \
             compares builds will refuse it"
        );
    }
    for (name, value) in identity.exported() {
        export(name, &value);
    }
}
