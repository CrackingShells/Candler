//! Records which build of the evaluation infrastructure this binary is: the nearest git tag, the
//! commits since it, the commit and whether the tree had uncommitted changes. A build outside a
//! git checkout records nothing.

use std::process::Command;

/// The text `git` prints for `args`, or none when it fails or prints nothing.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

/// Splits `git describe --tags --long` output, `<tag>-<n>-g<commit>`, into its three parts.
fn split_describe(text: &str) -> Option<(String, u64, String)> {
    let (rest, commit) = text.rsplit_once("-g")?;
    let (tag, ahead) = rest.rsplit_once('-')?;
    Some((tag.to_owned(), ahead.parse().ok()?, commit.to_owned()))
}

/// Hands one value to the compiler as an environment variable of the build.
fn export(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

fn main() {
    // The build is stale when the commit, the index or any source changes.
    for path in ["HEAD", "index"] {
        if let Some(found) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={found}");
        }
    }
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=../candler-core/src");
    println!("cargo:rerun-if-changed=../candler-core/assets");
    println!("cargo:rerun-if-changed=../candler-harness/src");
    println!("cargo:rerun-if-changed=../candler-harness-claude-code/src");
    println!("cargo:rerun-if-changed=../candler-schemas/src");
    println!("cargo:rerun-if-changed=../../Cargo.toml");
    println!("cargo:rerun-if-changed=../../Cargo.lock");

    let Some(dirty) = git(&["status", "--porcelain"]).map(|_| true).or_else(|| {
        // an empty status is a clean tree, but only inside a checkout
        git(&["rev-parse", "--git-dir"]).map(|_| false)
    }) else {
        return;
    };
    export(
        "CANDLER_EVAL_INFRA_DIRTY",
        if dirty { "true" } else { "false" },
    );
    if let Some((tag, ahead, commit)) =
        git(&["describe", "--tags", "--long"]).and_then(|text| split_describe(&text))
    {
        export("CANDLER_EVAL_INFRA_TAG", &tag);
        export("CANDLER_EVAL_INFRA_COMMITS_AHEAD", &ahead.to_string());
        export("CANDLER_EVAL_INFRA_COMMIT", &commit);
    } else if let Some(commit) = git(&["rev-parse", "--short", "HEAD"]) {
        export("CANDLER_EVAL_INFRA_COMMIT", &commit);
    }
}
