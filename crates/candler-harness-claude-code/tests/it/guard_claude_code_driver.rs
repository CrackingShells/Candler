#![cfg(test)]
//! The Claude Code driver: the agent command, the skill tree and output discovery.

use candler_testkit::file_list;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr as _;

use candler_harness::driver::HarnessDriver;
use candler_harness::spec::{
    Cell, CollectSpec, EnvValue, FileRoot, Observed, StopSignal, Workspace,
};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::{GitCommit, TreatmentRef};
use candler_schemas::primitives::{FieldPath, Name};
use candler_schemas::refusal::Refusal;
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
        // The one addition to the reference: the observation hooks' settings file, which the
        // census of loaded instruction files reads.
        argv.extend([
            "--settings".to_owned(),
            "/home/agent/.candler/settings.json".to_owned(),
        ]);
        Ok(argv)
    };
    let root = Path::new("/work/run");
    let driver = ClaudeCodeDriver::new();
    let reference = values("bypassPermissions")?;
    let command = driver
        .plan(&Cell::new(&reference), &Workspace::new(root))?
        .command;
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
    let command = driver
        .plan(&Cell::new(&plan), &Workspace::new(root))?
        .command;
    // BORROW: `.as_slice()` views the expected arguments as a slice
    assert_eq!(command.argv(), expected("plan")?.as_slice());
    Ok(())
}

/// Guards: a stored cell, which replays without going through resolve, that holds a permission mode
/// or an effort no mode or level names is refused by the driver with `experiment.cell_value_invalid`
/// at the field, as a fresh cell is, and is not run as the default mode or without an effort.
/// Oracle: the code and the field pointers written by hand.
/// Bite: read the text with a `from_text` that falls back to the default or to unset.
#[test]
fn a_stored_cell_with_an_unnamed_mode_or_effort_is_refused_at_its_field() -> Res<()> {
    let driver = ClaudeCodeDriver::new();
    let root = Path::new("/work/run");
    for (path, pointer) in [
        ("permission_mode", "/permission_mode"),
        ("model.effort", "/model/effort"),
    ] {
        let mut stored = values("plan")?;
        stored.retain(|value| value.path().as_str() != path);
        stored.push(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(json!("nonsense"))?,
        ));
        let Err(error) = driver.plan(&Cell::new(&stored), &Workspace::new(root)) else {
            return Err(format!("{path}: an unnamed text planned").into());
        };
        assert_eq!(
            Refusal::code(&error).as_str(),
            "experiment.cell_value_invalid",
            "{path}"
        );
        assert_eq!(Refusal::path(&error).as_deref(), Some(pointer), "{path}");
    }
    Ok(())
}

/// One regular file of a ustar archive: the header, then the contents padded to a block.
fn tar_entry(path: &str, contents: &[u8]) -> Vec<u8> {
    let mut header = vec![0_u8; 512];
    // INDEX: the header is 512 bytes and the path is shorter than the name field
    header[..path.len()].copy_from_slice(path.as_bytes());
    // INDEX: 100..108 lies inside the 512-byte header
    header[100..108].copy_from_slice(b"0000644\0");
    // INDEX: 124..136 lies inside the 512-byte header
    header[124..136].copy_from_slice(format!("{:011o}\0", contents.len()).as_bytes());
    // INDEX: 148..156 lies inside the 512-byte header
    header[148..156].copy_from_slice(b"        ");
    // INDEX: 156 lies inside the 512-byte header
    header[156] = b'0';
    // INDEX: 257..263 lies inside the 512-byte header
    header[257..263].copy_from_slice(b"ustar\0");
    // INDEX: 263..265 lies inside the 512-byte header
    header[263..265].copy_from_slice(b"00");
    let sum: u32 = header.iter().map(|b| u32::from(*b)).sum();
    // INDEX: 148..156 lies inside the 512-byte header
    header[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
    let mut entry = header;
    entry.extend_from_slice(contents);
    entry.resize(512 + contents.len().div_ceil(512) * 512, 0);
    entry
}

/// The archive of `entries` with its end marker.
fn tar_of(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut tar: Vec<u8> = entries
        .iter()
        .flat_map(|(path, contents)| tar_entry(path, contents.as_bytes()))
        .collect();
    tar.resize(tar.len() + 1024, 0);
    tar
}

/// Guards: I7 — the treatment is `{name, tag, commit}`, and the skill tree is the tagged subtree
/// `.claude/skills/<name>`, and nothing else of the repository.
/// Oracle: `git ls-tree -r <tag> -- .claude/skills/<name>` and `git cat-file blob` on a fixture
/// repo with a sibling skill, files outside the subtree, a repo-root symlink into the subtree and
/// an untracked file; a crafted tar with an entry outside the subtree.
/// Bite: drop the pathspec from the archive command; drop the scope check of the reader.
#[test]
fn b22_skill_tree_is_the_tag() -> Res<()> {
    let repo = scratch("b22-fixture-repo")?;
    git(&repo, &["init", "-q"])?;
    put(&repo, ".claude/skills/s/SKILL.md", "skill at the tag\n")?;
    put(&repo, ".claude/skills/s/scripts/run.sh", "#!/bin/sh\n")?;
    put(&repo, ".claude/skills/s2/SKILL.md", "a sibling skill\n")?;
    put(&repo, "scripts/run.sh", "#!/bin/sh\n")?;
    put(&repo, "README.md", "readme at the tag\n")?;
    put(&repo, "a/b/c.txt", "nested\n")?;
    git(&repo, &["add", "."])?;
    git(
        &repo,
        &[
            "update-index",
            "--chmod=+x",
            ".claude/skills/s/scripts/run.sh",
        ],
    )?;
    // A repo-root symlink into the subtree, as a tree entry so that the fixture needs no
    // symlink support from the file system.
    let target = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args(["hash-object", "-w", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin = target.stdin.as_ref().ok_or("no stdin")?;
    std::io::Write::write_all(&mut stdin, b".claude/skills/s/SKILL.md")?;
    let blob = String::from_utf8(target.wait_with_output()?.stdout)?;
    git(
        &repo,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{},link.md", blob.trim()),
        ],
    )?;
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
    let mut treatment = TreatmentRef::new(Name::from_str("s")?);
    // BORROW: `.to_owned()` makes the owned tag the treatment holds
    treatment.tag = Some("v1".to_owned());
    treatment.commit = Some(GitCommit::from_str(sha.trim())?);
    let skills = ClaudeCodeDriver::new().skills();
    let command = skills.archive_command(&treatment, &repo)?;
    let (program, args) = command.argv().split_first().ok_or("empty argv")?;
    let archive = Command::new(program).args(args).output()?;
    assert!(archive.status.success());
    let files = skills.files(&treatment, &archive.stdout)?;

    let listing = String::from_utf8(git(
        &repo,
        &["ls-tree", "-r", "v1", "--", ".claude/skills/s"],
    )?)?;
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
    // Positive control: a tracked file at the tag is there with its tagged content and mode.
    let skill = got
        .get(".claude/skills/s/SKILL.md")
        .ok_or("skill file missing")?;
    assert_eq!(skill.0, b"skill at the tag\n");
    assert!(
        got.get(".claude/skills/s/scripts/run.sh")
            .ok_or("script missing")?
            .1
    );
    assert_eq!(got.len(), 2);
    for outside in ["untracked.txt", "later.txt", "link.md", "scripts/run.sh"] {
        assert!(!got.contains_key(outside), "{outside}");
    }

    // A crafted archive with an entry outside the subtree is refused, whole.
    let inside = [
        (".claude/skills/s/SKILL.md", "skill\n"),
        (".claude/skills/s/notes.txt", "notes\n"),
    ];
    let [first, second] = inside;
    let accepted = skills.files(&treatment, &tar_of(&inside))?;
    assert_eq!(accepted.len(), 2);
    for outside in [
        "README.md",
        ".claude/skills/s2/SKILL.md",
        ".claude/skills/s",
    ] {
        let crafted = tar_of(&[first, (outside, "outside\n"), second]);
        assert!(skills.files(&treatment, &crafted).is_err(), "{outside}");
    }
    // The subtree without its `SKILL.md` is refused.
    assert!(skills.files(&treatment, &tar_of(&[second])).is_err());
    // A treatment name that is not one plain path segment is refused.
    for name in ["..", "a/b", "s:t"] {
        let mut other = treatment.clone();
        other.name = Name::from_str(name)?;
        assert!(skills.archive_command(&other, &repo).is_err(), "{name}");
        assert!(skills.files(&other, &tar_of(&inside)).is_err(), "{name}");
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
    let paths = file_list(&root);
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

/// Guards: nothing moves for Claude Code when the core applies a driver's stop — the driver
/// declares `SIGTERM`, no pre-stop action and the five seconds of grace the core applied before it
/// read the driver's stop.
/// Oracle: `DEFAULT_GRACE`, `Duration::from_secs(5)`, of `supervise/spawn.rs` at 40ef2fe.
/// Bite: `STOP_GRACE` of the driver back to 15 seconds.
#[test]
fn b24_stop_is_term_with_the_grace_the_core_applied_before() {
    let stop = ClaudeCodeDriver::new().stop();
    assert_eq!(stop.signal, StopSignal::Term);
    assert_eq!(stop.pre_stop, None);
    assert_eq!(stop.grace, std::time::Duration::from_secs(5));
}

/// Guards: G1 — `mcp.servers.<key>.name` gives a server its production name: `.mcp.json` keys the
/// server by it, the server object carries no `name`, and the tool rows of `tools.allowed` that
/// use the name pass through to the settings; a cell with a bad name is refused before any run.
/// Oracle: the literal JSON written by hand (`x-y` is the production name `a` cannot spell).
#[test]
fn a_server_name_renders_the_server_under_it() -> Res<()> {
    let mut cell = values("default")?;
    for (path, value) in [
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.a.name", json!("x-y")),
        ("tools.allowed", json!(["mcp__x-y__t"])),
    ] {
        cell.push(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value)?,
        ));
    }
    let driver = ClaudeCodeDriver::new();
    let files = driver
        .plan(&Cell::new(&cell), &Workspace::new(Path::new("/work/run")))?
        .prepared
        .files;
    let read = |suffix: &str| -> Res<serde_json::Value> {
        let file = files
            .iter()
            .find(|file| file.path.as_str().ends_with(suffix))
            .ok_or(format!("no {suffix}"))?;
        Ok(serde_json::from_slice(&file.contents)?)
    };
    assert_eq!(
        read(".mcp.json")?,
        json!({"mcpServers": {"x-y": {"type": "stdio", "command": "uvx"}}})
    );
    assert_eq!(
        read("settings.json")?,
        json!({"permissions": {"allow": ["mcp__x-y__t"]}})
    );
    // The refusal comes at plan time too, before any run.
    cell.push(ResolvedValue::recorded(
        FieldPath::from_str("mcp.servers.x_y.command")?,
        JsonValue::new(json!("uvx"))?,
    ));
    cell.push(ResolvedValue::recorded(
        FieldPath::from_str("mcp.servers.x_y.name")?,
        JsonValue::new(json!("x-y"))?,
    ));
    assert!(driver.check_settings(&Cell::new(&cell)).is_err());
    Ok(())
}

/// The `.mcp.json` the driver writes for a cell that sets `extra` besides the default values,
/// parsed.
fn mcp_json(extra: &[(&str, serde_json::Value)]) -> Res<serde_json::Value> {
    let mut cell = values("default")?;
    for (path, value) in extra {
        cell.push(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value.clone())?, // BORROW: .clone() copies the value into the cell
        ));
    }
    let plan =
        ClaudeCodeDriver::new().plan(&Cell::new(&cell), &Workspace::new(Path::new("/work/run")))?;
    let file = plan
        .prepared
        .files
        .iter()
        .find(|file| file.path.as_str().ends_with(".mcp.json"))
        .ok_or("no .mcp.json")?;
    Ok(serde_json::from_slice(&file.contents)?)
}

/// Guards: the `.mcp.json` of Claude Code holds, for each server, the keys its documentation gives
/// the server's `type` and no other: `stdio` takes `command`, `args` and `env`; `http` and `sse`
/// take `url` and `headers`; `local` is written as `stdio` and `remote` as `http`; the server's
/// `name` is its key and `codemode` is no key.
/// Oracle: the Claude Code MCP documentation, `code.claude.com/docs/en/mcp` and
/// `code.claude.com/docs/en/agent-sdk/typescript` (`McpStdioServerConfig`, `McpSSEServerConfig`,
/// `McpHttpServerConfig`), read through Context7 `/websites/code_claude`; the literal JSON below.
/// Bite: `serde_json::to_value(servers)` back in `render_mcp`.
#[test]
fn the_mcp_file_holds_the_keys_claude_code_documents() -> Res<()> {
    let documented = |kind: &str| -> &'static [&'static str] {
        match kind {
            "stdio" => &["type", "command", "args", "env"],
            "http" | "sse" => &["type", "url", "headers"],
            _ => &[],
        }
    };
    let written = mcp_json(&[
        ("mcp.servers.a.type", json!("local")),
        ("mcp.servers.a.command", json!("uvx")),
        ("mcp.servers.a.args", json!(["srv"])),
        ("mcp.servers.a.env", json!({"K": "v"})),
        ("mcp.servers.a.codemode", json!(false)),
        ("mcp.servers.a.name", json!("x-y")),
        ("mcp.servers.b.type", json!("remote")),
        ("mcp.servers.b.url", json!("http://h/mcp")),
        ("mcp.servers.b.headers", json!({"X": "y"})),
        ("mcp.servers.c.type", json!("sse")),
        ("mcp.servers.c.url", json!("http://h/sse")),
        ("mcp.servers.d.command", json!("uvx")),
    ])?;
    assert_eq!(
        written,
        json!({"mcpServers": {
            "x-y": {"type": "stdio", "command": "uvx", "args": ["srv"], "env": {"K": "v"}},
            "b": {"type": "http", "url": "http://h/mcp", "headers": {"X": "y"}},
            "c": {"type": "sse", "url": "http://h/sse"},
            "d": {"type": "stdio", "command": "uvx"},
        }})
    );
    for (name, entry) in written["mcpServers"].as_object().ok_or("no servers")? {
        let kind = entry["type"].as_str().ok_or("no type")?;
        let allowed = documented(kind);
        assert!(
            !allowed.is_empty(),
            "{name}: `{kind}` is no documented type"
        );
        for key in entry.as_object().ok_or("no entry")?.keys() {
            assert!(
                allowed.contains(&key.as_str()),
                "{name}: `{key}` is no documented key of `{kind}`"
            );
        }
    }
    Ok(())
}

/// Guards: Claude Code has no code mode, so a cell that sets `codemode = true` on a server is
/// refused with the driver's reason, and a cell that leaves `codemode` out or sets it to `false`
/// is not.
/// Oracle: the reason written by hand, from the leaf's gate.
/// Bite: `Answer::Mapped` for `Codemode(true)` in the driver's `answer`.
#[test]
fn claude_code_refuses_codemode_true() -> Res<()> {
    let refusal_of = |codemode: Option<serde_json::Value>| -> Res<Option<String>> {
        let mut cell = values("default")?;
        let mut entries = vec![("mcp.servers.a.command", json!("uvx"))];
        entries.extend(codemode.map(|value| ("mcp.servers.a.codemode", value)));
        for (path, value) in entries {
            cell.push(ResolvedValue::recorded(
                FieldPath::from_str(path)?,
                JsonValue::new(value)?,
            ));
        }
        Ok(candler_core::resolve::paths::refuse_unanswered(
            &ClaudeCodeDriver::new(),
            &Cell::new(&cell),
        )
        .err()
        .map(|error| error.to_string()))
    };
    let refused = refusal_of(Some(json!(true)))?.ok_or("codemode = true ran")?;
    assert!(
        refused.contains("Claude Code offers MCP tools directly; it has no code mode"),
        "{refused}"
    );
    assert_eq!(refusal_of(None)?, None);
    assert_eq!(refusal_of(Some(json!(false)))?, None);
    Ok(())
}

/// The cell of `values("default")` with `extra` values set.
fn values_with(extra: &[(&str, serde_json::Value)]) -> Res<Vec<ResolvedValue>> {
    let mut all = values("default")?;
    for (path, value) in extra {
        all.push(ResolvedValue::recorded(
            FieldPath::from_str(path)?,
            JsonValue::new(value.clone())?, // BORROW: .clone() copies the test's value
        ));
    }
    Ok(all)
}

/// Guards: G3 plan — a cell with the four values installs the global text in the home as
/// `.claude/CLAUDE.md` and reads it through the `user` source, lists the workspace text as the
/// read-only `CLAUDE.md` and writes none, renders `hooks` of `.claude/settings.json` equal to the
/// payload, places the hook files under `.claude/hooks/` executable, and passes the observers'
/// settings file, with `InstructionsLoaded`, to the headless command as to the session's.
/// Oracle: the payload and texts written here by hand.
/// Bite: leave `user` out of the sources, write `CLAUDE.md` into the prepared files, or pass the
/// observers only in session mode.
#[test]
fn a_cell_with_the_four_values_installs_them_where_claude_code_reads_them() -> Res<()> {
    let hooks = json!({"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": ".claude/hooks/guard.sh"}]}]});
    let cell = values_with(&[
        ("instructions.global", json!({"claude-code": "global text"})),
        ("instructions.workspace", json!("workspace text")),
        (
            "hooks",
            json!({"claude-code": {"settings": hooks, "files": {"guard.sh": "#!/bin/sh\nexit 0\n"}}}),
        ),
    ])?;
    let plan =
        ClaudeCodeDriver::new().plan(&Cell::new(&cell), &Workspace::new(Path::new("/work/run")))?;
    let find = |path: &str| {
        plan.prepared
            .files
            .iter()
            .find(|file| file.path.as_str() == path)
    };

    let global = find(".claude/CLAUDE.md").ok_or("no global file")?;
    assert_eq!(global.root, FileRoot::Home);
    assert_eq!(global.contents, b"global text");
    assert!(
        find("CLAUDE.md").is_none(),
        "the driver never writes the workspace file"
    );
    let [workspace] = plan.workspace_files.as_slice() else {
        return Err("expected one workspace file".into());
    };
    assert_eq!(workspace.path.as_str(), "CLAUDE.md");
    assert_eq!(workspace.root, FileRoot::Workspace);
    assert_eq!(workspace.contents, b"workspace text");

    let settings: serde_json::Value =
        serde_json::from_slice(&find(".claude/settings.json").ok_or("no settings")?.contents)?;
    assert_eq!(settings["hooks"], hooks);
    let script = find(".claude/hooks/guard.sh").ok_or("no hook file")?;
    assert!(script.executable);
    assert_eq!(script.contents, b"#!/bin/sh\nexit 0\n");

    let observers: serde_json::Value = serde_json::from_slice(
        &find(".candler/settings.json")
            .ok_or("no observers")?
            .contents,
    )?;
    assert!(observers["hooks"]["InstructionsLoaded"].is_array());
    let argv = plan.command.argv();
    let at = |flag: &str| argv.iter().position(|arg| arg == flag);
    let sources = at("--setting-sources").ok_or("no sources")?;
    assert_eq!(
        argv.get(sources + 1).map(String::as_str),
        Some("user,project,local")
    );
    let settings_flag = at("--settings").ok_or("the headless command has no --settings")?;
    assert_eq!(
        argv.get(settings_flag + 1).map(String::as_str),
        Some("/home/agent/.candler/settings.json")
    );
    Ok(())
}

/// Guards: G3 plan, no regression — a cell without these values renders the same
/// `.claude/settings.json` as before, reads the project and local sources only, and installs the
/// home observers file and no other file of the four values.
/// Oracle: `{}` and the sources of the reference line, written by hand.
/// Bite: add `user` or an empty `hooks` key for every cell.
#[test]
fn a_cell_without_the_values_changes_nothing_but_the_observers() -> Res<()> {
    let cell = values("default")?;
    let plan =
        ClaudeCodeDriver::new().plan(&Cell::new(&cell), &Workspace::new(Path::new("/work/run")))?;
    let paths: Vec<&str> = plan
        .prepared
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(paths, [".claude/settings.json", ".candler/settings.json"]);
    assert_eq!(plan.prepared.files[0].contents, b"{}\n");
    assert_eq!(plan.workspace_files, []);
    assert!(plan.command.argv().iter().any(|arg| arg == "project,local"));
    Ok(())
}

/// Guards: invariant 6 — a payload cannot disable the observers or write outside `.claude/`: its
/// `settings` land under `hooks` and nowhere else, a hook file path that leaves the directory is
/// refused, and the observers sit in a file the payload cannot name.
/// Oracle: payloads written by hand.
/// Bite: merge the payload into the root of the settings, or join paths without checking them.
#[test]
fn a_payload_cannot_reach_beyond_hooks_and_the_hooks_directory() -> Res<()> {
    let driver = ClaudeCodeDriver::new();
    let hostile = values_with(&[(
        "hooks",
        json!({"claude-code": {"settings": {"disableAllHooks": true, "Stop": []},
                                "files": {"ok.sh": "x"}}}),
    )])?;
    let plan = driver.plan(
        &Cell::new(&hostile),
        &Workspace::new(Path::new("/work/run")),
    )?;
    let settings: serde_json::Value = serde_json::from_slice(&plan.prepared.files[0].contents)?;
    let root: Vec<&String> = settings
        .as_object()
        .ok_or("not an object")?
        .keys()
        .collect();
    assert_eq!(root, ["hooks"]);
    for file in &plan.prepared.files {
        let path = file.path.as_str();
        let under = path.starts_with(".claude/hooks/")
            || path == ".claude/settings.json"
            || path == ".candler/settings.json";
        assert!(under, "{path}");
    }
    for path in ["../x.sh", "a/../../x.sh", "/etc/x.sh"] {
        let cell = values_with(&[("hooks", json!({"claude-code": {"files": {path: "x"}}}))])?;
        assert!(
            driver
                .plan(&Cell::new(&cell), &Workspace::new(Path::new("/work/run")))
                .is_err(),
            "{path} was accepted"
        );
    }
    Ok(())
}
