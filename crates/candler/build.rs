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
    // BORROW: .to_owned() gives the caller owned text
    (!text.is_empty()).then(|| text.to_owned())
}

/// Splits `git describe --tags --long` output, `<tag>-<n>-g<commit>`, into its three parts.
fn split_describe(text: &str) -> Option<(String, u64, String)> {
    let (rest, commit) = text.rsplit_once("-g")?;
    let (tag, ahead) = rest.rsplit_once('-')?;
    // BORROW: .to_owned() gives the caller owned text
    Some((tag.to_owned(), ahead.parse().ok()?, commit.to_owned()))
}

/// Hands one value to the compiler as an environment variable of the build.
fn export(name: &str, value: &str) {
    println!("cargo:rustc-env={name}={value}");
}

/// What the binary is built from, relative to this crate: the sources and assets of every crate
/// it links, the manifests and the build script. Cargo reruns the build when any of it changes,
/// and the tree is dirty when any of it differs from the commit.
const WATCHED: [&str; 14] = [
    "src",
    "build.rs",
    "Cargo.toml",
    "../candler-core/src",
    "../candler-core/Cargo.toml",
    "../candler-harness/src",
    "../candler-harness/Cargo.toml",
    "../candler-harness-claude-code/src",
    "../candler-harness-claude-code/Cargo.toml",
    "../candler-schemas/src",
    "../candler-schemas/Cargo.toml",
    "../../assets",
    "../../Cargo.toml",
    "../../Cargo.lock",
];

/// The tags of the binary's own releases: `v` and a digit. Other tags in the repository belong
/// to other crates.
const TAG_MATCH: &str = "v[0-9]*";

fn main() {
    // The build is stale when the commit moves (HEAD, the branch ref HEAD names, packed refs) or
    // the index changes, or when anything watched changes.
    // BORROW: .to_owned() makes the owned paths the list keeps
    let mut git_paths = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
    ];
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
        git_paths.push(branch);
    }
    for path in &git_paths {
        if let Some(found) = git(&["rev-parse", "--git-path", path]) {
            println!("cargo:rerun-if-changed={found}");
        }
    }
    for path in WATCHED {
        println!("cargo:rerun-if-changed={path}");
    }

    let mut status = vec!["status", "--porcelain", "--"];
    status.extend(WATCHED);
    let Some(dirty) = git(&status)
        .map(|_| true)
        // an empty status is a clean tree, but only inside a checkout
        .or_else(|| git(&["rev-parse", "--git-dir"]).map(|_| false))
    else {
        return;
    };
    export(
        "CANDLER_EVAL_INFRA_DIRTY",
        if dirty { "true" } else { "false" },
    );
    if let Some((tag, ahead, commit)) = git(&["describe", "--tags", "--long", "--match", TAG_MATCH])
        .and_then(|text| split_describe(&text))
    {
        export("CANDLER_EVAL_INFRA_TAG", &tag);
        export("CANDLER_EVAL_INFRA_COMMITS_AHEAD", &ahead.to_string());
        export("CANDLER_EVAL_INFRA_COMMIT", &commit);
    } else if let Some(commit) = git(&["rev-parse", "--short", "HEAD"]) {
        export("CANDLER_EVAL_INFRA_COMMIT", &commit);
    }
}
