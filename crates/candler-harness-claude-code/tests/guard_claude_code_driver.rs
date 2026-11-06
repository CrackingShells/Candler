#![cfg(test)]
//! The Claude Code driver: the agent command, the skill tree and output discovery.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr as _;

use candler_harness::driver::HarnessDriver;
use candler_harness::spec::{Cell, CollectSpec, EnvValue, Observed, Workspace};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_claude_code::prepare::{skill_archive_command, skill_files};
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::{FieldPath, Name};
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::json;

// TRAIT_OBJECT: a test returns any error its steps raise; the error is only printed
type Res<T> = Result<T, Box<dyn Error>>;

/// The cell of a run: the values the driver reads.
fn values(mode: &str) -> Res<Vec<ResolvedValue>> {
    let entries = [
        ("model.id", json!("claude-haiku-4-5")),
        ("permission_mode", json!(mode)),
        ("prompt.template", json!("Build {thing} now")),
        ("prompt.values", json!({"thing": "a domain"})),
    ];
    entries
        .into_iter()
        .map(|(path, value)| {
            Ok(ResolvedValue::recorded(
                FieldPath::from_str(path)?,
                JsonValue::new(value)?,
            ))
        })
        .collect()
}

/// The directory a test builds its fixture in, empty.
fn scratch(name: &str) -> Res<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// The output of `git` run in `repo`.
fn git(repo: &Path, args: &[&str]) -> Res<Vec<u8>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(output.stdout)
}

/// Writes `contents` at `path` under `repo`, with its parent directories.
fn put(repo: &Path, path: &str, contents: &str) -> Res<()> {
    let file = repo.join(path);
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(file, contents)?;
    Ok(())
}

/// Guards: B21 — the agent command is the reference command line.
/// Oracle: `feature_map/reference_flags.txt` (read here, not copied).
/// Bite: drop `--setting-sources project,local` from the argv.
#[test]
fn b21_reference_command_line() -> Res<()> {
    let flags = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../__roadmap__/m2b-execution-parity/feature_map/reference_flags.txt");
    let prompt = "Build a domain now";
    // Provenance: reference_flags.txt, the non-comment lines; `<prompt>` and `<model>` are the
    // run's values and `bypassPermissions` is the cell value of the reference.
    let expected = |mode: &str| -> Res<Vec<String>> {
        let mut argv = Vec::new();
        for line in fs::read_to_string(&flags)?.lines() {
            if line.starts_with('#') {
                continue;
            }
            for token in line.split_whitespace() {
                // BORROW: `.to_owned()` makes the owned argument the command line holds
                argv.push(match token {
                    "<prompt>" => prompt.to_owned(),
                    "<model>" => "claude-haiku-4-5".to_owned(),
                    "bypassPermissions" => mode.to_owned(),
                    other => other.to_owned(),
                });
            }
        }
        Ok(argv)
    };
    let root = Path::new("/work/run");
    let driver = ClaudeCodeDriver::new();
    let reference = values("bypassPermissions")?;
    let command = driver.command(&Cell::new(&reference), &Workspace::new(root))?;
    // BORROW: `.as_slice()` views the expected arguments as a slice
    assert_eq!(command.argv(), expected("bypassPermissions")?.as_slice());
    // Provenance: reference_flags.txt, "NOT passed: --bare, --allowedTools, --max-budget-usd".
    for flag in [
        "--bare",
        "--allowedTools",
        "--max-budget-usd",
        "--effort",
        "--max-turns",
    ] {
        assert!(!command.argv().iter().any(|arg| arg == flag), "{flag}");
    }
    // Provenance: R15, the driver emits no endpoint and no token, and A26 no variable at all.
    assert_eq!(command.env_overlay().len(), 0);
    for entry in command.env_overlay() {
        assert!(!entry.name.starts_with("ANTHROPIC_"), "{}", entry.name);
        assert!(!matches!(entry.value, EnvValue::Secret(_)));
    }
    // Positive control: the permission mode follows the cell.
    let plan = values("plan")?;
    let command = driver.command(&Cell::new(&plan), &Workspace::new(root))?;
    // BORROW: `.as_slice()` views the expected arguments as a slice
    assert_eq!(command.argv(), expected("plan")?.as_slice());
    Ok(())
}

/// Guards: I7 — the treatment is `{name, tag, commit}`, and the skill tree is the tagged tree.
/// Oracle: `git ls-tree -r <tag>` and `git cat-file blob` on a fixture repo with an untracked file.
/// Bite: archive `HEAD` instead of the pinned commit.
#[test]
fn b22_skill_tree_is_the_tag() -> Res<()> {
    let repo = scratch("b22-fixture-repo")?;
    git(&repo, &["init", "-q"])?;
    put(&repo, ".claude/skills/s/SKILL.md", "skill at the tag\n")?;
    put(&repo, "scripts/run.sh", "#!/bin/sh\n")?;
    put(&repo, "README.md", "readme at the tag\n")?;
    put(&repo, "a/b/c.txt", "nested\n")?;
    git(&repo, &["add", "."])?;
    git(&repo, &["update-index", "--chmod=+x", "scripts/run.sh"])?;
    git(&repo, &["commit", "-q", "-m", "tagged"])?;
    git(&repo, &["tag", "v1"])?;
    put(&repo, "README.md", "readme after the tag\n")?;
    put(&repo, "later.txt", "later\n")?;
    git(&repo, &["add", "."])?;
    git(&repo, &["commit", "-q", "-m", "later"])?;
    put(&repo, "untracked.txt", "untracked\n")?;
    put(
        &repo,
        ".claude/skills/s/SKILL.md",
        "skill edited in the tree\n",
    )?;

    let sha = String::from_utf8(git(&repo, &["rev-parse", "v1^{commit}"])?)?;
    let mut treatment = TreatmentRef::new(Name::from_str("treatment")?);
    // BORROW: `.to_owned()` makes the owned tag the treatment holds
    treatment.tag = Some("v1".to_owned());
    treatment.commit = Some(GitCommit::from_str(sha.trim())?);
    let command = skill_archive_command(&treatment, &repo)?;
    let (program, args) = command.argv().split_first().ok_or("empty argv")?;
    let archive = Command::new(program).args(args).output()?;
    assert!(archive.status.success());
    let files = skill_files(&archive.stdout)?;

    let listing = String::from_utf8(git(&repo, &["ls-tree", "-r", "v1"])?)?;
    let mut tagged = BTreeMap::new();
    for line in listing.lines() {
        let (meta, path) = line.split_once('\t').ok_or("bad ls-tree line")?;
        let mode = meta.split(' ').next().ok_or("bad ls-tree meta")?;
        let blob = git(&repo, &["cat-file", "blob", &format!("v1:{path}")])?;
        // BORROW: `.to_owned()` makes the owned path the map keys on
        tagged.insert(path.to_owned(), (blob, mode == "100755"));
    }
    let got: BTreeMap<String, (Vec<u8>, bool)> = files
        .into_iter()
        .map(|file| (file.path.to_string(), (file.contents, file.executable)))
        .collect();
    assert_eq!(got, tagged);
    // Positive control: a tracked file at the tag is there with its tagged content.
    let skill = got
        .get(".claude/skills/s/SKILL.md")
        .ok_or("skill file missing")?;
    assert_eq!(skill.0, b"skill at the tag\n");
    assert!(got.get("scripts/run.sh").ok_or("script missing")?.1);
    assert!(!got.contains_key("untracked.txt") && !got.contains_key("later.txt"));
    Ok(())
}

/// The paths under `dir`, relative to `root`, joined with `/`.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Res<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let rel = path.strip_prefix(root)?;
            let parts: Vec<_> = rel.iter().map(|part| part.to_string_lossy()).collect();
            out.push(parts.join("/"));
        }
    }
    Ok(())
}

/// Whether the `/`-separated `path` matches the glob `pattern`, where `**` stands for any number
/// of segments and `*` for any one segment.
fn glob(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.split_first(), path.split_first()) {
        (None, None) => true,
        (Some((&"**", rest)), _) => {
            glob(rest, path)
                || path
                    .split_first()
                    .is_some_and(|(_, tail)| glob(pattern, tail))
        }
        (None, Some(_)) | (Some(_), None) => false,
        (Some((&head, rest)), Some((&segment, tail))) => {
            (head == "*" || head == segment) && glob(rest, tail)
        }
    }
}

/// Whether `path` matches the glob text `pattern`.
fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    glob(&pattern, &path)
}

/// Guards: I8 — output discovery leaves out `.claude/` at any depth.
/// Oracle: `SANDBOX_REFERENCE.md` v5 lines 166-167 (htn-creator-eval@0.4.2): any path under a
/// `.claude/` directory is excluded from discovery.
/// Bite: remove `.claude` from the excluded directories.
#[test]
fn b23_output_discovery_exclusions() -> Res<()> {
    let root = scratch("b23-fixture-tree")?;
    for path in [
        "out/a.txt",
        "domain.py",
        ".claude/x",
        "a/.claude/y",
        "x/.claudeish/q",
    ] {
        put(&root, path, "x\n")?;
    }
    let mut paths = Vec::new();
    walk(&root, &root, &mut paths)?;
    paths.sort();
    let specs = ClaudeCodeDriver::new().collect(&Workspace::new(&root), &Observed::default());
    let globs: Vec<&str> = specs
        .iter()
        .filter_map(|spec| {
            let CollectSpec::File { glob, .. } = spec else {
                return None;
            };
            Some(glob.as_str()) // BORROW: .as_str() borrows the glob
        })
        .collect();
    let kept: Vec<&str> = paths
        .iter()
        .map(String::as_str) // BORROW: .as_str() borrows the text
        .filter(|path| {
            let wanted = globs
                .iter()
                .any(|g| !g.starts_with('!') && matches(g, path));
            let left_out = globs
                .iter()
                .any(|g| g.strip_prefix('!').is_some_and(|rest| matches(rest, path)));
            wanted && !left_out
        })
        .collect();
    // Provenance: the reference lines above; `.claudeish` is no `.claude` directory.
    assert_eq!(kept, vec!["domain.py", "out/a.txt", "x/.claudeish/q"]);
    // Positive control: without the exclusions every path is a candidate.
    assert_eq!(
        paths.iter().filter(|p| matches("**/*", p)).count(),
        paths.len()
    );
    Ok(())
}
