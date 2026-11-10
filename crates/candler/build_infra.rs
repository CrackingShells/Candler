//! The build identity of the `candler` binary, resolved from the origin of the build. `build.rs`
//! includes this file with `#[path]`, and so does `tests/guard_eval_infra.rs`.
//!
//! A field an origin cannot know is `None`, never a guess: the verdict refuses an unrecorded
//! `commit` or `dirty` and names the cause.

use std::path::Path;
use std::process::Command;

/// What the binary is built from, relative to the crate directory: the sources and assets of
/// every crate it links, the manifests and the build script. Cargo reruns the build when any of
/// it changes, and a checkout is dirty when any of it differs from the commit.
pub const WATCHED: [&str; 15] = [
    "src",
    "build.rs",
    "build_infra.rs",
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

/// The file `cargo package` writes beside the manifest of a packaged crate.
const VCS_FILE: &str = ".cargo_vcs_info.json";

/// The names the packager sets in the build's environment.
pub const PACKAGER_VARS: [&str; 3] = [
    "CANDLER_EVAL_INFRA_TAG",
    "CANDLER_EVAL_INFRA_COMMIT",
    "CANDLER_EVAL_INFRA_DIRTY",
];

/// Where an identity came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A git checkout: the commit and whether the tree is dirty are measured.
    Git,
    /// A packaged crate: `.cargo_vcs_info.json` names the commit and whether it was dirty.
    Packaged,
    /// The packager set `CANDLER_EVAL_INFRA_*` in the build's environment.
    Packager,
    /// Nothing proves the commit: only the package version is known.
    None,
}

/// The identity a build records. Each field is exported as one `CANDLER_EVAL_INFRA_*` name when
/// it is known.
#[derive(Debug, PartialEq, Eq)]
pub struct Identity {
    /// The origin the fields came from.
    pub origin: Origin,
    /// The release tag.
    pub tag: Option<String>,
    /// The commits since the tag, known only from a git checkout.
    pub commits_ahead: Option<u64>,
    /// The commit the build is made from.
    pub commit: Option<String>,
    /// Whether the sources differed from the commit.
    pub dirty: Option<bool>,
}

/// The text `git` prints for `args` in `dir`, or none when it fails or prints nothing.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let text = text.trim();
    // BORROW: .to_owned() gives the caller owned text
    (!text.is_empty()).then(|| text.to_owned())
}

/// Splits `git describe --tags --long` output, `<tag>-<n>-g<commit>`, into the tag and distance.
fn split_describe(text: &str) -> Option<(String, u64)> {
    let (rest, _commit) = text.rsplit_once("-g")?;
    let (tag, ahead) = rest.rsplit_once('-')?;
    // BORROW: .to_owned() gives the caller owned text
    Some((tag.to_owned(), ahead.parse().ok()?))
}

/// Whether `text` is a full commit name: 40 lowercase hexadecimal digits.
fn is_full_commit(text: &str) -> bool {
    text.len() == 40 && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The identity of a git checkout, or none when `dir` is not inside one or has no commit.
fn from_git(dir: &Path) -> Option<Identity> {
    let commit = git(dir, &["rev-parse", "HEAD"]).filter(|c| is_full_commit(c))?;
    let mut status = vec!["status", "--porcelain", "--"];
    status.extend(WATCHED);
    // A status that fails is an unknown state, not a clean tree.
    let dirty = Command::new("git")
        .current_dir(dir)
        .args(&status)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| !out.stdout.iter().all(u8::is_ascii_whitespace));
    let described = git(dir, &["describe", "--tags", "--long", "--match", TAG_MATCH])
        .and_then(|text| split_describe(&text));
    let (tag, commits_ahead) = match described {
        Some((tag, ahead)) => (Some(tag), Some(ahead)),
        None => (None, None),
    };
    Some(Identity {
        origin: Origin::Git,
        tag,
        commits_ahead,
        commit: Some(commit),
        dirty,
    })
}

/// The text of the JSON string value that follows `key` in `text`.
fn string_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = text.split_once(key)?.1;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest.split_once('"')?.0)
}

/// `.cargo_vcs_info.json`'s commit and flag. Cargo writes `"dirty": true` only when the tree was
/// dirty, so a file that names a commit and no flag is a clean tree.
fn parse_vcs(text: &str) -> Option<(String, bool)> {
    let git_object = text.split_once("\"git\"")?.1;
    let git_object = git_object
        .split_once('}')
        .map_or(git_object, |(inner, _)| inner);
    let sha = string_after(git_object, "\"sha1\"").filter(|s| is_full_commit(s))?;
    let dirty = git_object
        .split_once("\"dirty\"")
        .map(|(_, rest)| rest.trim_start().strip_prefix(':').map(str::trim_start))
        .is_some_and(|value| value.is_some_and(|v| v.starts_with("true")));
    // BORROW: .to_owned() gives the caller owned text
    Some((sha.to_owned(), dirty))
}

/// The identity of a packaged crate: the vcs file beside the manifest, and `v<version>`.
fn from_package(dir: &Path, version: &str) -> Option<Identity> {
    let text = std::fs::read_to_string(dir.join(VCS_FILE)).ok()?;
    let (commit, dirty) = parse_vcs(&text)?;
    Some(Identity {
        origin: Origin::Packaged,
        tag: Some(format!("v{version}")),
        commits_ahead: None,
        commit: Some(commit),
        dirty: Some(dirty),
    })
}

/// The identity the packager set in the environment, when it set any of the three names. An
/// unset or unreadable flag is unrecorded.
fn from_packager(version: &str, env: &impl Fn(&str) -> Option<String>) -> Option<Identity> {
    let [tag, commit, dirty] = PACKAGER_VARS.map(env);
    if tag.is_none() && commit.is_none() && dirty.is_none() {
        return None;
    }
    Some(Identity {
        origin: Origin::Packager,
        tag: tag.or_else(|| Some(format!("v{version}"))),
        commits_ahead: None,
        commit: commit.filter(|c| !c.is_empty()),
        dirty: dirty.and_then(|flag| match flag.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }),
    })
}

/// The identity of the build of the crate in `dir`, at package version `version`, in an
/// environment that `env` looks names up in. The origins are tried in order: a git checkout, a
/// packaged crate's vcs file, the packager's variables, and last the version alone.
///
/// A directory that holds a vcs file is a packaged crate even when it sits inside someone
/// else's git checkout, so the checkout is not consulted for it.
pub fn resolve(dir: &Path, version: &str, env: impl Fn(&str) -> Option<String>) -> Identity {
    let packaged = dir.join(VCS_FILE).is_file();
    (if packaged { None } else { from_git(dir) })
        .or_else(|| from_package(dir, version))
        .or_else(|| from_packager(version, &env))
        .unwrap_or_else(|| Identity {
            origin: Origin::None,
            tag: Some(format!("v{version}")),
            commits_ahead: None,
            commit: None,
            dirty: None,
        })
}
