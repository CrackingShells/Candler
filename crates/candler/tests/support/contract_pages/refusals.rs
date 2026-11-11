//! The refusals the pages list: the rows of the code's catalogue and of the helper's registry
//! section, sorted by code, and the table that says which page a surface's codes go to.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::fs;

use candler_core::refusal::catalog;
use candler_schemas::refusal::Side;

use super::repo_root;

/// Where the codes of one surface are listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    /// The page of this file under `docs/contract/`.
    Page(&'static str),
    /// The index's list of other refusals: the surface names no page of its own.
    Index,
}

/// The surface of a code is the part before its dot. Every surface the catalogue and the helper's
/// section hold is named here; a surface that is not named fails the guard with its name.
pub const SURFACES: [(&str, Home); 23] = [
    ("plugin", Home::Page("plugin-manifest.md")),
    ("suite", Home::Page("suite-file.md")),
    ("protocol", Home::Page("evaluator-protocol.md")),
    ("outcome", Home::Page("outcome-record.md")),
    ("experiment", Home::Page("experiment-file.md")),
    ("helper", Home::Page("evaluator-helper.md")),
    // The shared surfaces check the values an experiment file carries.
    ("format", Home::Page("experiment-file.md")),
    ("canonical", Home::Page("experiment-file.md")),
    ("values", Home::Page("experiment-file.md")),
    // The fallback code of an error that is not an author's; its side is Candler's, so no list
    // holds it.
    ("candler", Home::Index),
    ("analysis", Home::Index),
    ("catalog", Home::Index),
    ("config", Home::Index),
    ("evaluation", Home::Index),
    ("harness", Home::Index),
    ("import", Home::Index),
    ("isolation", Home::Index),
    ("jobs", Home::Index),
    ("manifest", Home::Index),
    ("resolve", Home::Index),
    ("runner", Home::Index),
    ("store", Home::Index),
    ("supervise", Home::Index),
];

/// One refusal as a table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The code, `<surface>.<reason>`.
    pub code: String,
    /// The side to fix, as written on the wire.
    pub side: String,
    /// When the refusal arises.
    pub when: String,
    /// What the path points at, or `-`.
    pub at: String,
}

impl Entry {
    /// The surface: the part of the code before its dot.
    pub fn surface(&self) -> &str {
        surface_of(&self.code)
    }
}

/// The surface of a code.
pub fn surface_of(code: &str) -> &str {
    code.split('.').next().unwrap_or(code)
}

/// Where the codes of `surface` are listed, or `None` for a surface the table does not name.
pub fn home_of(surface: &str) -> Option<Home> {
    SURFACES
        .iter()
        .find(|(name, _)| *name == surface)
        .map(|(_, home)| *home)
}

/// The surface names the table holds.
pub fn surface_names() -> Vec<&'static str> {
    SURFACES.iter().map(|(name, _)| *name).collect()
}

/// The rows of the code's catalogue, one per code: a variant that delegates to the error it
/// wraps has no code of its own, and a code that several variants raise is listed once.
pub fn catalogue_entries() -> Vec<Entry> {
    let mut by_code: BTreeMap<String, Entry> = BTreeMap::new();
    for row in catalog() {
        if row.delegates || row.code.is_empty() {
            continue;
        }
        by_code.entry(row.code.to_owned()).or_insert_with(|| Entry {
            code: row.code.to_owned(),
            side: row.side.as_str().to_owned(),
            when: row.when.to_owned(),
            at: row.at.to_owned(),
        });
    }
    by_code.into_values().collect()
}

/// The rows of the `[helper]` section of the registry.
pub fn helper_entries() -> Result<Vec<Entry>, String> {
    let path = repo_root().join("docs/contract/codes.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let table: toml::Table = text.parse().map_err(|e| format!("codes.toml: {e}"))?;
    let rows = table
        .get("helper")
        .and_then(toml::Value::as_table)
        .ok_or("codes.toml has no [helper] section")?;
    let field = |row: &toml::Table, code: &str, name: &str| -> Result<String, String> {
        row.get(name)
            .and_then(toml::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("[helper.\"{code}\"] has no `{name}`"))
    };
    rows.iter()
        .map(|(code, row)| {
            let row = row
                .as_table()
                .ok_or_else(|| format!("[helper.\"{code}\"] is not a table"))?;
            Ok(Entry {
                code: code.clone(),
                side: field(row, code, "side")?,
                when: field(row, code, "when")?,
                at: field(row, code, "at")?,
            })
        })
        .collect()
}

/// Every row the pages draw on, sorted by code. A code of a side other than Candler's is an
/// author's; a code of Candler's own is not listed, except the helper's, whose Candler-side rows
/// say what a malformed request from Candler does.
pub fn all_entries() -> Result<Vec<Entry>, String> {
    let mut rows: Vec<Entry> = catalogue_entries()
        .into_iter()
        .filter(|row| row.side != Side::Candler.as_str())
        .collect();
    rows.extend(helper_entries()?);
    rows.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(rows)
}

/// The codes of the catalogue and of the helper's section, whatever their side.
pub fn known_codes() -> Result<BTreeSet<String>, String> {
    let mut known: BTreeSet<String> = catalogue_entries().into_iter().map(|e| e.code).collect();
    known.extend(helper_entries()?.into_iter().map(|e| e.code));
    Ok(known)
}

fn cell(text: &str) -> String {
    text.replace('|', "\\|").replace('\n', " ")
}

/// The refusals table of `rows`, or a sentence when there are none.
fn table(rows: &[&Entry]) -> String {
    if rows.is_empty() {
        return "No refusal of this surface has a code.\n".to_owned();
    }
    let mut out =
        String::from("| Code | When | Path at fault | Side to fix |\n|---|---|---|---|\n");
    for row in rows {
        let at = if row.at == "-" {
            "-".to_owned()
        } else {
            cell(&row.at)
        };
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            row.code,
            cell(&row.when),
            at,
            row.side
        );
    }
    out
}

/// The generated Refusals of the page `file`: the rows whose surface the table gives to it.
pub fn page_refusals(file: &str) -> Result<String, String> {
    let rows = all_entries()?;
    let mine: Vec<&Entry> = rows
        .iter()
        .filter(|row| matches!(home_of(row.surface()), Some(Home::Page(name)) if name == file))
        .collect();
    Ok(format!(
        "<!-- generated:refusals -->\n{}<!-- /generated:refusals -->",
        table(&mine)
    ))
}

/// The generated list of the index: the rows whose surface names no page.
pub fn other_refusals() -> Result<String, String> {
    let rows = all_entries()?;
    let mine: Vec<&Entry> = rows
        .iter()
        .filter(|row| home_of(row.surface()) == Some(Home::Index))
        .collect();
    Ok(format!(
        "<!-- generated:refusals -->\n{}<!-- /generated:refusals -->",
        table(&mine)
    ))
}

/// What follows the dot of a backticked file name such as `plugin.toml`, which is not a code.
const FILE_EXTENSIONS: [&str; 5] = ["toml", "json", "md", "py", "rs"];

/// The codes a text names in backticks, among the tokens whose surface the table names.
pub fn codes_named_in(text: &str) -> Vec<String> {
    let surfaces = surface_names();
    let mut found = Vec::new();
    for (at, part) in text.split('`').enumerate() {
        if at % 2 == 0 {
            continue;
        }
        let token = part.trim();
        let Some((surface, reason)) = token.split_once('.') else {
            continue;
        };
        let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_');
        if word(surface)
            && word(reason)
            && surfaces.contains(&surface)
            && !FILE_EXTENSIONS.contains(&reason)
        {
            found.push(token.to_owned());
        }
    }
    found
}

/// The codes in the first cell of each row of the generated refusals block of `text`.
pub fn listed_codes(text: &str) -> Vec<String> {
    let mut inside = false;
    let mut found = Vec::new();
    for line in text.lines() {
        if line.starts_with("<!-- generated:refusals -->") {
            inside = true;
        } else if line.starts_with("<!-- /generated:refusals -->") {
            inside = false;
        } else if inside
            && let Some(rest) = line.strip_prefix("| `")
            && let Some((code, _)) = rest.split_once('`')
        {
            found.push(code.to_owned());
        }
    }
    found
}
