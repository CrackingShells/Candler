//! Guards for the layering of the core: the instrument's domain stays out of the core, the
//! harness crate and the published schemas (I1), and the import takes every domain name from its
//! rules. The guard matches identifier segments (a name is split on non-alphanumerics, `_`,
//! case changes and acronym boundaries) against the denied segments below, which are a literal
//! of this file written from the names of the legacy domain; a test that rewrites those names
//! in a copy of the mini-corpus proves that the import does not need them.
#![cfg(test)]

use crate::oracle;
use crate::support;
use crate::support::legacy_import;

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::LazyLock;

use candler_core::import::assemble::import;
use candler_core::import::rules::{
    HarnessRule, ImportRules, ImportRulesBuilder, MarkerCode, MarkerRule, SnapshotRule,
    TreatmentRule,
};
use candler_core::store::Store;
use candler_schemas::primitives::{FieldPath, Name, NameToken, ReasonCode};
use flate2::Compression;
use flate2::read::MultiGzDecoder;
use flate2::write::GzEncoder;
use legacy_import::{mini, project, reader};
use oracle::{expected_rows, expected_runs};
use regex::Regex;

/// The identifier segments of the legacy domain that no layer below the rules may hold, each in
/// lowercase.
const DENIED_SEGMENTS: [&str; 13] = [
    "htn",
    "gtpyhop",
    "pyhop",
    "rikyu",
    "planner",
    "planners",
    "tier",
    "tiers",
    "problems",
    "libcst",
    "arango",
    "dotenv",
    "plantrace",
];

/// The multi-word names of the legacy domain, as runs of consecutive segments: they are found in
/// any separator style (`plan_trace`, `PLAN-TRACE`, `plan.trace`, `PlanTrace`). A name written in
/// one lowercase word with no separator (`htnplanner`) is a known limit of the segment split.
const DENIED_SEQUENCES: [&[&str]; 2] = [&["plan", "trace"], &["mcp", "python", "ingestion"]];

/// The names that no split can express, matched as exact literals.
const DENIED_LITERALS: [&str; 1] = ["D-E"];

/// The lines whose use of a denied segment is generic: `(path relative to the workspace root,
/// exact trimmed line)`. Only that file is excused, and every line of it that has that exact text
/// (a known limit: two identical lines of one file are excused together).
const ALLOWED_LINES: [(&str, &str); 0] = [];

/// The extensions of the known binary files, whose bytes are not read as text. None: the only
/// binary files met are tool caches, which `SKIPPED_NAMES` skips.
const BINARY_EXTENSIONS: [&str; 0] = [];

/// The exact names of the files and folders that tools and the OS leave untracked in any
/// folder; they are skipped and a skipped folder is not entered.
const SKIPPED_NAMES: [&str; 5] = [
    ".DS_Store",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".venv",
];

/// The identifier segments of the harnesses that the core and the published schemas may not
/// hold, each in lowercase: the driver crates and the binary name their harness freely.
const HARNESS_SEGMENTS: [&str; 3] = ["claude", "anthropic", "opencode"];

/// The harness names that split into several segments (`OpenCode`, `open_code`).
const HARNESS_SEQUENCES: [&[&str]; 1] = [&["open", "code"]];

/// The roots, relative to the workspace root, that also deny `HARNESS_SEGMENTS` and
/// `HARNESS_SEQUENCES`.
const HARNESS_ROOTS: [&str; 2] = ["crates/candler-core/src", "crates/candler-schemas/src"];

/// The domain and harness segments together, for the roots of `HARNESS_ROOTS`.
static HARNESS_DENIED: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| [&DENIED_SEGMENTS[..], &HARNESS_SEGMENTS[..]].concat());

/// The domain and harness sequences together, for the roots of `HARNESS_ROOTS`.
static HARNESS_DENIED_SEQUENCES: LazyLock<Vec<&'static [&'static str]>> =
    LazyLock::new(|| [&DENIED_SEQUENCES[..], &HARNESS_SEQUENCES[..]].concat());

/// What every scan of this file denies and excuses: the domain names.
fn rules() -> support::SegmentRules<'static> {
    support::SegmentRules {
        denied: &DENIED_SEGMENTS,
        sequences: &DENIED_SEQUENCES,
        literals: &DENIED_LITERALS,
        allow: &ALLOWED_LINES,
        binary: &BINARY_EXTENSIONS,
        skip: &SKIPPED_NAMES,
    }
}

/// The rules of the scan of `target` below `base`: the domain names, and the harness names too
/// for a root of `HARNESS_ROOTS`.
fn rules_for(base: &Path, target: &Path) -> support::SegmentRules<'static> {
    let rooted = HARNESS_ROOTS.iter().any(|root| {
        target
            .strip_prefix(base)
            .is_ok_and(|rel| rel.starts_with(root))
    });
    if rooted {
        support::SegmentRules {
            denied: &HARNESS_DENIED,
            sequences: &HARNESS_DENIED_SEQUENCES,
            ..rules()
        }
    } else {
        rules()
    }
}

/// What the domain names become in the renamed copy of the corpus, in the order they are applied.
const RENAMES: [(&str, &str); 21] = [
    ("mcp-python-ingestion", "mcp-ingest"),
    ("gtpyhop", "planlib"),
    ("GTPyhop", "Planlib"),
    ("rikyu", "zenith"),
    ("Rikyu", "Zenith"),
    ("RIKYU", "ZENITH"),
    ("tier_d", "tier_x"),
    ("TIER_D", "TIER_X"),
    ("tier", "band"),
    ("Tier", "Band"),
    ("TIER", "BAND"),
    ("planner", "scheduler"),
    ("Planner", "Scheduler"),
    ("problems", "cases"),
    ("Problems", "Cases"),
    ("PROBLEMS", "CASES"),
    ("arango", "graphdb"),
    ("Arango", "Graphdb"),
    ("htn", "acme"),
    ("HTN", "ACME"),
    ("D-E", "X-E"),
];

/// The targets that the guard scans under the workspace `root`: for every folder listed under
/// `crates/`, its `src`, its `Cargo.toml` and its `build.rs` and `build_infra.rs` where present;
/// the published schemas; `assets/`; and the evaluator helper's `src`, `scripts` and
/// `pyproject.toml`. Changelogs, which are history, and READMEs are not scanned.
fn scanned_targets(root: &Path) -> Vec<PathBuf> {
    let mut crates: Vec<PathBuf> = fs::read_dir(root.join("crates"))
        .expect("crates/ lists")
        .map(|entry| entry.expect("an entry"))
        .filter(|entry| entry.file_type().expect("a type").is_dir())
        .map(|entry| entry.path())
        .collect();
    crates.sort();
    let mut targets = Vec::new();
    for krate in crates {
        targets.push(krate.join("src"));
        targets.push(krate.join("Cargo.toml"));
        for optional in ["build.rs", "build_infra.rs"] {
            if krate.join(optional).is_file() {
                targets.push(krate.join(optional));
            }
        }
    }
    targets.push(root.join("crates/candler-schemas/schema"));
    targets.push(root.join("assets"));
    targets.push(root.join("python/candler-evaluator/src"));
    targets.push(root.join("python/candler-evaluator/scripts"));
    targets.push(root.join("python/candler-evaluator/pyproject.toml"));
    targets
}

/// The workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The hits of a segment scan of `target` below `base`, which must read a file.
fn segment_hits(base: &Path, target: &Path) -> Vec<support::SegmentHit> {
    let found = support::scan_segments(base, target, &rules_for(base, target));
    assert!(
        found.is_ok(),
        "scan of {} failed: {found:?}",
        target.display()
    );
    let found = found.unwrap_or_default();
    assert!(found.files > 0, "scan of {} read no file", target.display());
    found.hits
}

/// Asserts that no file under `dir` holds a denied name.
fn assert_no_segment(dir: &Path) {
    let hits = segment_hits(dir, dir);
    assert!(
        hits.is_empty(),
        "denied names under {}: {hits:#?}",
        dir.display()
    );
}

/// Guards: I1 — no line, file name or folder name under any crate's `src`, manifest or build
/// script, the published schemas, `assets/` or the Python evaluator helper holds a denied
/// identifier segment (`planner`, `tier`, `problems`, ...) or sequence, and a crate added under
/// `crates/` is scanned without editing the guard. The core's and the schemas' `src` also hold
/// no harness name (`claude`, `anthropic`, `opencode`, `OpenCode`).
/// Oracle: the deny literals of this file.
#[test]
fn workspace_holds_no_domain_segment() {
    let root = workspace_root();
    let targets = scanned_targets(&root);
    assert!(targets.len() >= 12, "the scanned targets: {targets:?}");
    for target in &targets {
        let hits = segment_hits(&root, target);
        assert!(
            hits.is_empty(),
            "denied names in {}: {hits:#?}",
            target.display()
        );
    }
}

/// The path of `path` below `base`, with `/` separators.
fn relative_to(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .expect("below the base")
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The tracked paths of the git repository at `repo` that lie in one of the scanned `scope`
/// paths (relative to `repo`, a folder or a file) and have a skipped name as any path
/// component, from `git ls-files -z`.
fn tracked_under_skipped_names(repo: &Path, scope: &[String]) -> Vec<String> {
    let listing = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["ls-files", "-z"])
        .output()
        .expect("git runs");
    assert!(listing.status.success(), "git ls-files failed: {listing:?}");
    String::from_utf8_lossy(&listing.stdout)
        .split('\0')
        .filter(|path| {
            scope
                .iter()
                .any(|target| path == target || path.starts_with(&format!("{target}/")))
        })
        .filter(|path| {
            path.split('/')
                .any(|component| SKIPPED_NAMES.contains(&component))
        })
        .map(str::to_owned)
        .collect()
}

/// Guards: I1 — nothing that ships hides under a skipped name: no tracked file of a scanned
/// target has `.DS_Store`, `__pycache__`, `.pytest_cache`, `.mypy_cache` or `.venv` as a path
/// component, so the scan's skip list never hides committed content.
/// Oracle: `git ls-files -z` over the workspace.
#[test]
fn workspace_tracks_no_file_under_a_skipped_name() {
    let root = workspace_root();
    let scope: Vec<String> = scanned_targets(&root)
        .iter()
        .map(|target| relative_to(&root, target))
        .collect();
    let tracked = tracked_under_skipped_names(&root, &scope);
    assert!(
        tracked.is_empty(),
        "tracked paths under a skipped name: {tracked:#?}"
    );
}

/// Guards: I1 — the tracked-file check names a tracked file under a skipped folder and leaves an
/// untracked `.DS_Store` alone. Oracle: the hand-written repository.
#[test]
fn tracked_check_names_the_tracked_file_under_a_skipped_name_only() {
    let repo = tempfile::tempdir().expect("a temporary directory");
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("git runs");
        assert!(done.status.success(), "git {args:?}: {done:?}");
    };
    git(&["init", "-q"]);
    fs::create_dir_all(repo.path().join("src/__pycache__")).expect("a folder");
    fs::write(repo.path().join("src/__pycache__/x.rs"), "fn x() {}\n").expect("tracked file");
    fs::write(repo.path().join("src/y.rs"), "fn y() {}\n").expect("clean file");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "tracked"]);
    fs::write(repo.path().join(".DS_Store"), [0xff_u8, 0x00]).expect("untracked junk");
    assert_eq!(
        tracked_under_skipped_names(repo.path(), &["src".to_owned()]),
        ["src/__pycache__/x.rs"]
    );
}

/// A temporary tree laid out like the workspace, each scanned target holding one clean file,
/// with `content` written at `at`.
fn planted_tree(at: &str, content: &str) -> tempfile::TempDir {
    let tree = tempfile::tempdir().expect("a temporary directory");
    for krate in [
        "candler",
        "candler-core",
        "candler-harness",
        "candler-harness-claude-code",
        "candler-harness-opencode",
        "candler-schemas",
    ] {
        let dir = tree.path().join("crates").join(krate);
        fs::create_dir_all(dir.join("src")).expect("a crate folder");
        fs::write(dir.join("src/lib.rs"), "pub fn run_plan() {}\n").expect("a clean file");
        fs::write(dir.join("Cargo.toml"), "[package]\n").expect("a clean manifest");
    }
    for (folder, file) in [
        ("crates/candler-schemas/schema", "s.json"),
        ("assets", "a.txt"),
        ("python/candler-evaluator/src", "h.py"),
        ("python/candler-evaluator/scripts", "g.py"),
    ] {
        let dir = tree.path().join(folder);
        fs::create_dir_all(&dir).expect("a folder");
        fs::write(dir.join(file), "clean\n").expect("a clean file");
    }
    fs::write(
        tree.path().join("python/candler-evaluator/pyproject.toml"),
        "[project]\n",
    )
    .expect("a clean manifest");
    let target = tree.path().join(at);
    fs::create_dir_all(target.parent().expect("a parent")).expect("the parent folder");
    fs::write(target, content).expect("the planted file");
    tree
}

/// Every hit that the guard's scan of the planted `tree` finds across its scanned targets.
fn planted_hits(tree: &Path) -> Vec<String> {
    scanned_targets(tree)
        .iter()
        .flat_map(|target| segment_hits(tree, target))
        .map(|hit| hit.segment)
        .collect()
}

/// Guards: I1 — a domain symbol planted in the binary, the core, the assets, the schemas' Rust
/// sources, a build script, a manifest, the helper's scripts or a file or folder name, in any
/// separator style, fails the guard, and `run_plan`, `ContainerPlan`, `plan`, `planning`,
/// `tiered`, `WeightNotAllowed`, `frontier` and `problem_count` do not.
/// Oracle: the hand-written table below.
#[test]
fn plant_table_is_found_or_passes_as_written() {
    // (where, content, the name found or none)
    let table: &[(&str, &str, Option<&str>)] = &[
        (
            "crates/candler/src/x.rs",
            "struct PlannerAxis;\n",
            Some("planner"),
        ),
        (
            "crates/candler-core/src/y.rs",
            "let planner_package = 1;\n",
            Some("planner"),
        ),
        (
            "assets/docker/env/z.in",
            "gtpyhop-core>=2\n",
            Some("gtpyhop"),
        ),
        (
            "crates/candler-schemas/src/w.rs",
            "\"tier-a\"\n",
            Some("tier"),
        ),
        (
            "crates/candler-core/src/v.rs",
            "const TIER_D: u8 = 1;\n",
            Some("tier"),
        ),
        (
            "crates/candler-harness/src/u.rs",
            "struct PlanTrace;\n",
            Some("plan_trace"),
        ),
        (
            "crates/candler-core/src/u1.rs",
            "let plan_trace = 1;\n",
            Some("plan_trace"),
        ),
        (
            "crates/candler-core/src/u2.rs",
            "let Plan_Trace = 1;\n",
            Some("plan_trace"),
        ),
        (
            "crates/candler-core/src/u3.rs",
            "PLAN-TRACE\n",
            Some("plan_trace"),
        ),
        (
            "crates/candler-core/src/u4.rs",
            "plan.trace\n",
            Some("plan_trace"),
        ),
        (
            "crates/candler-core/src/u5.rs",
            "let plantrace = 1;\n",
            Some("plantrace"),
        ),
        (
            "crates/candler-core/src/u6.rs",
            "mcp_python_ingestion\n",
            Some("mcp_python_ingestion"),
        ),
        (
            "crates/candler-core/src/u7.rs",
            "mcp-python-ingestion\n",
            Some("mcp_python_ingestion"),
        ),
        (
            "python/candler-evaluator/src/t.py",
            "import dotenv\n",
            Some("dotenv"),
        ),
        ("crates/candler/src/n.rs", "let x = \"D-E\";\n", Some("D-E")),
        ("assets/planner/x", "clean\n", Some("planner")),
        ("assets/gtpyhop_notes.txt", "clean\n", Some("gtpyhop")),
        (
            "crates/candler-core/build.rs",
            "// planner\n",
            Some("planner"),
        ),
        ("crates/candler/build_infra.rs", "// rikyu\n", Some("rikyu")),
        (
            "crates/candler-schemas/Cargo.toml",
            "htn = \"1\"\n",
            Some("htn"),
        ),
        (
            "python/candler-evaluator/pyproject.toml",
            "name = \"libcst\"\n",
            Some("libcst"),
        ),
        (
            "python/candler-evaluator/scripts/p.py",
            "# arango\n",
            Some("arango"),
        ),
        (
            "crates/candler-core/src/s.rs",
            "run_plan(ContainerPlan)\n",
            None,
        ),
        (
            "crates/candler-core/src/s1.rs",
            "plan planning tiered\n",
            None,
        ),
        (
            "crates/candler-core/src/r.rs",
            "enum E { WeightNotAllowed }\n",
            None,
        ),
        ("crates/candler-core/src/q.rs", "let frontier = 1;\n", None),
        (
            "crates/candler-core/src/p.rs",
            "let problem_count = 1;\n",
            None,
        ),
        ("crates/candler/src/o.rs", "struct HTTPServer;\n", None),
    ];
    for (at, content, expected) in table {
        let tree = planted_tree(at, content);
        let found = planted_hits(tree.path());
        let want: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(found, want, "plant {content:?} at {at}");
    }
}

/// Guards: I1 — a harness name planted in the core's or the schemas' sources, as a constant, a
/// type or a string, in any separator style, fails the guard, and the same plants in a harness
/// driver crate or the binary do not.
/// Oracle: the hand-written table below.
#[test]
fn harness_plants_are_found_only_in_the_core_and_the_schemas() {
    // (where, content, the name found or none)
    let table: &[(&str, &str, Option<&str>)] = &[
        (
            "crates/candler-core/src/a.rs",
            "const claude_version: u8 = 1;\n",
            Some("claude"),
        ),
        (
            "crates/candler-schemas/src/b.rs",
            "struct AnthropicKey;\n",
            Some("anthropic"),
        ),
        (
            "crates/candler-schemas/src/c.rs",
            "let k = \"ANTHROPIC_API_KEY\";\n",
            Some("anthropic"),
        ),
        (
            "crates/candler-core/src/d.rs",
            "let p = \"opencode-db\";\n",
            Some("opencode"),
        ),
        (
            "crates/candler-core/src/h.rs",
            "enum Harness { OpenCode }\n",
            Some("open_code"),
        ),
        (
            "crates/candler-harness-claude-code/src/e.rs",
            "const claude_version: u8 = 1;\n",
            None,
        ),
        (
            "crates/candler-harness-opencode/src/f.rs",
            "let p = \"opencode-db\";\n",
            None,
        ),
        (
            "crates/candler/src/g.rs",
            "let k = \"ANTHROPIC_API_KEY\";\n",
            None,
        ),
    ];
    for (at, content, expected) in table {
        let tree = planted_tree(at, content);
        let found = planted_hits(tree.path());
        let want: Vec<String> = expected.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(found, want, "plant {content:?} at {at}");
    }
}

/// Guards: I1 — a planted name is found inside a gzip file too, as the corpora are stored.
/// Oracle: the hand-written plant.
#[test]
fn plant_inside_a_gzip_file_is_found() {
    let tree = planted_tree("assets/z.gz", "x");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(b"[rikyu]\n").expect("compresses");
    fs::write(
        tree.path().join("assets/z.gz"),
        encoder.finish().expect("finishes"),
    )
    .expect("written");
    assert_eq!(planted_hits(tree.path()), ["rikyu"]);
}

/// Guards: I1 — a symlink under a scanned folder, and a file that is not text, fail the scan and
/// name the offender instead of being skipped, while `.DS_Store` and cache folders are skipped. Oracle: the hand-written trees.
#[test]
fn plant_symlink_or_binary_file_under_assets_fails_the_scan_naming_it() {
    let tree = planted_tree("assets/a2.txt", "clean\n");
    std::os::unix::fs::symlink("a.txt", tree.path().join("assets/link.txt")).expect("a symlink");
    let failed = support::scan_segments(tree.path(), &tree.path().join("assets"), &rules());
    let message = failed.expect_err("the symlink fails the scan").to_string();
    assert!(
        message.contains("link.txt") && message.contains("symlink"),
        "{message}"
    );

    let tree = planted_tree("assets/blob.bin", "x");
    fs::write(
        tree.path().join("assets/blob.bin"),
        [0xff_u8, 0xfe, 0x00, 0x9f],
    )
    .expect("bytes");
    let failed = support::scan_segments(tree.path(), &tree.path().join("assets"), &rules());
    let message = failed
        .expect_err("the binary file fails the scan")
        .to_string();
    assert!(message.contains("blob.bin"), "{message}");

    let tree = planted_tree("assets/.DS_Store", "x");
    fs::write(
        tree.path().join("assets/.DS_Store"),
        [0x00_u8, 0x00, 0x00, 0x01, 0xff],
    )
    .expect("bytes");
    let cache = tree.path().join("python/candler-evaluator/src/__pycache__");
    fs::create_dir_all(&cache).expect("a cache folder");
    fs::write(cache.join("x.pyc"), [0xff_u8, 0xfe, 0x00]).expect("bytes");
    for target in scanned_targets(tree.path()) {
        assert!(
            support::scan_segments(tree.path(), &target, &rules()).is_ok(),
            "{target:?}"
        );
    }

    let tree = planted_tree("assets/notes.dat", "x");
    fs::write(tree.path().join("assets/notes.dat"), [0xff_u8, 0xfe, 0x00]).expect("bytes");
    let failed = support::scan_segments(tree.path(), &tree.path().join("assets"), &rules());
    let message = failed
        .expect_err("an unlisted binary type fails")
        .to_string();
    assert!(message.contains("notes.dat"), "{message}");

    let listed = support::SegmentRules {
        binary: &["dat"],
        ..rules()
    };
    let found = support::scan_segments(tree.path(), &tree.path().join("assets"), &listed);
    assert!(found.is_ok_and(|scan| scan.hits.is_empty()));
}

/// Guards: I1 — the splitter cuts at case changes and acronym boundaries, and an allowlist
/// entry excuses its exact path and exact line only: the same line in `xa.rs` or in `sub/a.rs`
/// is still found. Oracle: hand-written splits and tree.
#[test]
fn segments_split_on_case_and_an_allowlist_entry_excuses_one_line_only() {
    assert_eq!(support::split_run("HTTPServer"), ["http", "server"]);
    assert_eq!(
        support::split_run("WeightNotAllowed"),
        ["weight", "not", "allowed"]
    );
    assert_eq!(support::split_run("tier2"), ["tier", "2"]);

    let tree = tempfile::tempdir().expect("a temporary directory");
    fs::create_dir(tree.path().join("sub")).expect("sub");
    fs::write(tree.path().join("a.rs"), "let tier = 1;\nlet tier = 2;\n").expect("a.rs");
    fs::write(tree.path().join("xa.rs"), "let tier = 2;\n").expect("xa.rs");
    fs::write(tree.path().join("sub/a.rs"), "let tier = 2;\n").expect("sub/a.rs");
    let allow = [("a.rs", "let tier = 2;")];
    let only_tier = support::SegmentRules {
        denied: &["tier"],
        sequences: &[],
        literals: &[],
        allow: &allow,
        binary: &[],
        skip: &[],
    };
    let found = support::scan_segments(tree.path(), tree.path(), &only_tier).expect("scans");
    let mut at: Vec<(String, usize)> = found
        .hits
        .iter()
        .map(|h| {
            let shown = h.file.strip_prefix(tree.path()).expect("below the tree");
            (shown.to_string_lossy().into_owned(), h.line)
        })
        .collect();
    at.sort();
    assert_eq!(
        at,
        [
            ("a.rs".to_owned(), 1),
            ("sub/a.rs".to_owned(), 1),
            ("xa.rs".to_owned(), 1)
        ]
    );
}

/// The bytes with every domain name renamed.
fn renamed(bytes: &[u8]) -> Vec<u8> {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    for (from, to) in RENAMES {
        text = text.replace(from, to);
    }
    text.into_bytes()
}

/// Copies `from` to `to` with every directory name, file name and file renamed, a gzip file
/// decompressed, rewritten and compressed again.
fn copy_renamed(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the target directory is created");
    for entry in fs::read_dir(from).expect("the directory lists") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let target = to.join(String::from_utf8(renamed(name.as_bytes())).expect("a UTF-8 name"));
        if entry.file_type().expect("a type").is_dir() {
            copy_renamed(&entry.path(), &target);
            continue;
        }
        let bytes = fs::read(entry.path()).expect("the file reads");
        let written = if bytes.starts_with(&[0x1f, 0x8b]) {
            let mut plain = Vec::new();
            // BORROW: the decoder reads the slice of `bytes` in place
            MultiGzDecoder::new(bytes.as_slice())
                .read_to_end(&mut plain)
                .expect("the stream gunzips");
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&renamed(&plain))
                .expect("the stream compresses");
            encoder.finish().expect("the stream finishes")
        } else {
            renamed(&bytes)
        };
        fs::write(&target, written).expect("the file is written");
    }
}

/// A rules builder with everything but the markers: the instrument and the legacy format are
/// the caller's.
fn rules_without_markers(instrument: &str, format: &str, entry_key: &str) -> ImportRulesBuilder {
    let name = |text: &str| Name::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let path = |text: &str| FieldPath::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    ImportRules::builder()
        .format(NameToken::from_str(format).unwrap_or_else(|e| panic!("{format}: {e}")))
        .harness(HarnessRule::new(
            name("claude-code"),
            "/claude_code_version".into(),
        ))
        .treatment(TreatmentRule::new(
            "/plan/skill_version".into(),
            path("resolved_skill_version"),
        ))
        .instrument(name(instrument))
        .snapshot(SnapshotRule::new(name("catalog"), "/catalog/sha256".into()))
        .suite_kind(name("legacy-check"))
        .entry_params(vec![name("scale"), name("text"), name(entry_key)])
        .metadata(path("plan.model"))
}

/// The status counts of the oracle's rows over the mini-corpus.
fn oracle_counts() -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for row in expected_rows(&mini()) {
        *counts.entry(row.status).or_default() += 1;
    }
    counts
}

/// The projected rows and the run count of an import of `folder` with `rules` into a fresh
/// store; the store folder is returned so that the caller can scan it.
fn import_rows(
    folder: &Path,
    rules: &ImportRules,
    store_dir: &Path,
) -> (usize, Vec<oracle::ExpectedRow>) {
    let store = Store::init(store_dir).expect("the store initialises");
    let report = import(folder, rules, &reader, &store).expect("the folder imports");
    let mut rows = Vec::new();
    for entry in store.runs().expect("the runs list") {
        for file in store
            .outcome_files(&entry.key)
            .expect("the outcome files list")
        {
            for record in store.read_outcomes(&file).expect("the file reads") {
                let value = serde_json::to_value(&record).expect("a record serializes");
                rows.push(project("", entry.key.replicate(), &value));
            }
        }
    }
    (usize::try_from(report.runs).expect("a run count"), rows)
}

/// Guards: I1 — a copy of the mini-corpus whose every file, directory and stream has the domain
/// names rewritten (`tier` becomes `band`, and the entry params name `band`), imported with
/// rules that name an `acme-eval` instrument, a renamed legacy format and a renamed marker, gives the oracle's run count and the oracle's count of each
/// status, and the store holds no denied segment. Oracle: `oracle::expected_runs`
/// and `oracle::expected_rows` over the original mini-corpus, and the deny literals of this file.
#[test]
fn a_renamed_domain_imports_to_the_same_counts_and_leaves_no_name_in_the_store() {
    let copy = tempfile::tempdir().expect("a temporary directory");
    copy_renamed(&mini(), copy.path());
    assert_no_segment(copy.path());

    let pattern = Regex::new(r"BAND_X_REPLAY:[^\n]*reason=(?P<code>[A-Za-z0-9_]+)")
        .expect("the pattern compiles");
    let zero = Regex::new(r"declares zero 'mappings' entries").expect("the pattern compiles");
    let rules = rules_without_markers("acme-eval", "acme-results@1", "band")
        .marker(MarkerRule::new(pattern, MarkerCode::Capture("code".into())))
        .marker(MarkerRule::new(
            zero,
            MarkerCode::Fixed(ReasonCode::from_str("zero_mappings").expect("a reason code")),
        ))
        .build()
        .expect("the renamed rules build");

    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, rows) = import_rows(copy.path(), &rules, store_dir.path());
    assert_eq!(runs, expected_runs(&mini()).len());
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for row in &rows {
        // BORROW: `.clone()` copies the status
        *counts.entry(row.status.clone()).or_default() += 1;
    }
    assert_eq!(counts, oracle_counts());
    assert_no_segment(store_dir.path());
}

/// Guards: I1 and D6 — a reason code comes from the rules and not from the core: with rules that
/// hold no marker, every `not_measured` record of the mini-corpus has the cause
/// `{legacy: unrecorded}`, while the oracle's rows show that the rules with markers give some
/// `not_measured` record another cause. Oracle: `oracle::expected_rows` over the mini-corpus.
#[test]
fn without_markers_every_unmeasured_record_is_unrecorded() {
    let marked: Vec<_> = expected_rows(&mini())
        .into_iter()
        .filter(|row| row.status == "not_measured")
        .filter(|row| row.cause.as_deref() != Some("legacy: unrecorded"))
        .collect();
    assert!(
        !marked.is_empty(),
        "the mini-corpus has no `not_measured` record with a marker code"
    );

    let rules = rules_without_markers("htn-creator-eval", "htn-results@1", "tier")
        .build()
        .expect("the rules without markers build");
    let store_dir = tempfile::tempdir().expect("a temporary directory");
    let (runs, rows) = import_rows(&mini(), &rules, store_dir.path());
    assert_eq!(runs, 6);
    let unmeasured: Vec<_> = rows
        .iter()
        .filter(|row| row.status == "not_measured")
        .collect();
    assert!(
        !unmeasured.is_empty(),
        "the mini-corpus has no `not_measured` record"
    );
    for row in unmeasured {
        assert_eq!(row.cause.as_deref(), Some("legacy: unrecorded"), "{row:?}");
    }
}
