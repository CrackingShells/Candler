#![cfg(test)]
//! Guards for the surface every verb of the `candler` binary shares: each verb has a row in the
//! working-directory matrix or a place on its pending list, and each option's help ends with a
//! stamp that says why the option exists. The binary is the real one; the verbs and options are
//! read from its `--help`, so a verb or an option added later is found without editing the guards.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

/// What `candler --help` lists as verbs and has no row for yet: each verb here works from a fixture
/// world only. A leaf that makes a verb work from any directory moves it to [`MATRIX`].
pub const PENDING_CWD: &[&str] = &[
    "run", "watch", "ingest", "evaluate", "show", "diff", "resolve", "ls", "analyze", "inspect",
];

/// The options whose help does not end with a stamp yet, as `"<verb> --<option>"`, or
/// `"--<option>"` for a global option. A leaf that writes the stamp removes the entry.
pub const PENDING_STAMPS: &[&str] = &[
    "--store",
    "run --set",
    "run --force",
    "kill --all",
    "kill --yes",
    "show --rates",
    "show --format",
    "show --evaluator",
    "diff --from",
    "diff --to",
    "diff --equivalence",
    "diff --format",
    "watch --json",
];

/// A verb and the arguments that run it, by a relative `--store`, on an empty directory.
struct Row {
    verb: &'static str,
    args: &'static [&'static str],
}

/// The verbs that run from an unrelated directory with no world around them.
const MATRIX: &[Row] = &[
    Row {
        verb: "ps",
        args: &["--store", "store", "ps"],
    },
    Row {
        verb: "kill",
        args: &["--store", "store", "kill", "--all", "--yes"],
    },
];

/// The two stamps an option's help may end with.
const STAMPS: [&str; 2] = ["cannot be derived: ", "not an experiment setting: "];

/// The real binary with `args`.
fn candler(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_candler"))
        .args(args)
        .output()
        .expect("the binary runs")
}

/// The text of `candler <args> --help`.
fn help(args: &[&str]) -> String {
    let mut all = args.to_vec(); // BORROW: the call line is extended
    all.push("--help");
    let output = candler(&all);
    assert!(output.status.success(), "{all:?} --help must succeed");
    String::from_utf8(output.stdout).expect("help is text")
}

/// The verbs in the `Commands:` block of `candler --help`, minus `help`.
fn listed_verbs() -> BTreeSet<String> {
    help(&[])
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .filter(|verb| *verb != "help")
        .map(str::to_owned)
        .collect()
}

/// The options of a help text, by long name, with the help of each: the lines of its block
/// joined by a space, without the `[default: …]`, `[env: …]` and `[possible values: …]` notes.
fn options_of(help: &str) -> Vec<(String, String)> {
    let mut found: Vec<(String, String)> = Vec::new();
    let mut in_options = false;
    for line in help.lines() {
        if !line.starts_with(' ') && !line.is_empty() {
            in_options = line == "Options:";
            continue;
        }
        if !in_options {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let text = line.trim();
        if (2..=6).contains(&indent) && text.starts_with('-') {
            let name = text
                .split([' ', ','])
                .find_map(|word| word.strip_prefix("--"))
                .unwrap_or_default();
            // The short form of the help puts the option's words after the declaration and two
            // spaces; the long form puts them on the lines that follow.
            let inline = text.split_once("  ").map_or("", |(_, rest)| rest.trim());
            found.push((name.to_owned(), inline.to_owned())); // BORROW: the entry owns its text
        } else if let Some((_, body)) = found.last_mut()
            && !text.is_empty()
        {
            body.push(' ');
            body.push_str(text);
        }
    }
    found
        .into_iter()
        .map(|(name, body)| (name, strip_notes(&body)))
        .collect()
}

/// `body` without its bracketed notes and without a list of possible values.
fn strip_notes(body: &str) -> String {
    let body = body.split(" Possible values:").next().unwrap_or_default();
    let mut out = String::new();
    let mut rest = body;
    while let Some(open) = rest.find('[') {
        out.push_str(rest.get(..open).unwrap_or_default());
        rest = rest.get(open..).unwrap_or_default();
        match rest.find(']') {
            Some(close) => rest = rest.get(close + 1..).unwrap_or_default(),
            None => break,
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the last sentence of `help` starts with a stamp. Clap drops the final full stop of a
/// one-sentence-per-line help, so the sentence is found by the stop before it.
fn ends_with_stamp(help: &str) -> bool {
    let last = help
        .rfind(". ")
        .map_or(help, |at| help.get(at + 2..).unwrap_or_default());
    STAMPS.iter().any(|stamp| last.starts_with(stamp))
}

/// Guards: U1 — every verb that `candler --help` lists has a row in the working-directory matrix
/// or a place on `PENDING_CWD`, and no verb has both. Oracle: the `Commands:` block of the real
/// binary's `--help`. Bite: a verb added to clap with neither row nor pending entry, or a pending
/// entry removed, names the verb and fails; so does a row kept on the list after it has one.
#[test]
fn u1_every_verb_has_a_row() {
    let listed = listed_verbs();
    let rows: BTreeSet<String> = MATRIX.iter().map(|row| row.verb.to_owned()).collect();
    let pending: BTreeSet<String> = PENDING_CWD.iter().map(|verb| (*verb).to_owned()).collect();
    assert!(!listed.is_empty(), "the help lists no verb");
    let both: Vec<_> = rows.intersection(&pending).collect();
    assert!(
        both.is_empty(),
        "verbs with a row and a pending entry: {both:?}"
    );
    let covered: BTreeSet<String> = rows.union(&pending).cloned().collect();
    assert_eq!(
        covered, listed,
        "the matrix and PENDING_CWD cover the listed verbs"
    );
}

/// Guards: U1 — each verb with a row runs from a fresh directory, with `HOME` and the XDG
/// variables inside it and a relative `--store`, exits 0 and leaves the directory empty. Oracle:
/// the exit status and the directory listing of that run. Bite: a verb that reads the checkout it
/// was built in, or writes beside the working directory, fails its row.
#[test]
fn u1_each_verb_runs_from_an_unrelated_directory() {
    assert!(!MATRIX.is_empty(), "the matrix has no row");
    for row in MATRIX {
        let dir = TempDir::new().expect("a directory");
        let home: &Path = dir.path();
        let output = Command::new(env!("CARGO_BIN_EXE_candler"))
            .args(row.args)
            .current_dir(home)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home)
            .env("XDG_DATA_HOME", home)
            .env("XDG_STATE_HOME", home)
            .env("XDG_CACHE_HOME", home)
            .output()
            .expect("the binary runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{}: {stderr}", row.verb);
        let left: Vec<_> = std::fs::read_dir(home)
            .expect("the directory lists")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert!(
            left.is_empty(),
            "{} left {left:?} in the directory",
            row.verb
        );
    }
}

/// Guards: U2 — the help of every option, global or of a verb, ends with a sentence that starts
/// `cannot be derived: ` or `not an experiment setting: `; `--help`, `--version` and positional
/// arguments are outside the walk. The options that fail are exactly `PENDING_STAMPS`. Oracle:
/// the `--help` of the real binary, each option's continuation lines joined. Bite: an entry
/// removed from the list, or an option whose stamp is deleted, names the option and fails; so
/// does an entry left on the list after its option has the stamp.
#[test]
fn u2_every_option_help_ends_with_a_stamp() {
    let mut failing: BTreeSet<String> = BTreeSet::new();
    let mut seen = 0_usize;
    let globals = options_of(&help(&[]));
    let skipped = |name: &str| name == "help" || name == "version";
    for (name, text) in globals.iter().filter(|(name, _)| !skipped(name)) {
        seen += 1;
        if !ends_with_stamp(text) {
            failing.insert(format!("--{name}"));
        }
    }
    let global_names: BTreeSet<&str> = globals.iter().map(|(name, _)| name.as_str()).collect();
    for verb in listed_verbs() {
        for (name, text) in options_of(&help(&[verb.as_str()])) {
            if skipped(&name) || global_names.contains(name.as_str()) {
                continue;
            }
            seen += 1;
            if !ends_with_stamp(&text) {
                failing.insert(format!("{verb} --{name}"));
            }
        }
    }
    assert!(seen > 10, "the walk saw {seen} options");
    assert!(
        !PENDING_STAMPS.contains(&"--error-format"),
        "--error-format has its stamp and is not pending"
    );
    let pending: BTreeSet<String> = PENDING_STAMPS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    assert_eq!(
        failing, pending,
        "the options without a stamp are the pending list"
    );
}
