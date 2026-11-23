//! The pages of the configuration surface, read from the path table of `candler_core::resolve`,
//! and the exit codes of the index, read from `EXIT_CODES` in the binary's source.

use std::fmt::Write;
use std::fs;

use candler_core::resolve::paths::{CONFIGURATION_PATHS, ConfigFile, Kind};
use syn::{Expr, ExprArray, ExprLit, Item, Lit};

use super::harness::{HarnessPage, assemble, cell};
use super::refusals::{Entry, all_entries};
use super::repo_root;
use super::versions::{CLOSE, OPEN, Versions};

/// The page of the configuration paths.
pub const PATHS_PAGE: HarnessPage = HarnessPage {
    file: "configuration-paths.md",
    title: "Configuration paths",
    writer: "experimenter",
};

/// The page of the two configuration files.
pub const FILE_PAGE: HarnessPage = HarnessPage {
    file: "config-file.md",
    title: "Configuration files",
    writer: "project owner, user",
};

const PATHS_SURFACE: &str = "The dotted paths an experiment file, a `--set` and a configuration file name: which are values of the experiment, which are operational settings that never reach a cell digest, and which are keys of a configuration file.";

const FILE_SURFACE: &str = "The keys of the project configuration `candler.toml` and of the user configuration `config.toml`; Candler reads them to find the store, the plug-in directories, the run slots and the gateway's trust.";

const PATHS_CODES: [&str; 3] = [
    "experiment.unknown_path",
    "experiment.unknown_request_key",
    "config.retired_setting",
];

const FILE_CODES: [&str; 6] = [
    "config.unknown_key",
    "config.wrong_type",
    "config.machine_wide_key",
    "config.toml_invalid",
    "config.file_unreadable",
    "config.upstream_ca_invalid",
];

fn version_block(versions: &Versions) -> Result<String, String> {
    let core = versions.tag("v").ok_or("no core version")?;
    Ok(format!(
        "<!-- generated:version -->\n{OPEN}\nCore line: {core}.\n{CLOSE}\n<!-- /generated:version -->"
    ))
}

fn entries(codes: &[&str]) -> Result<Vec<Entry>, String> {
    let known: Vec<Entry> = all_entries().into_iter().flatten().collect();
    codes
        .iter()
        .map(|code| {
            known
                .iter()
                .find(|row| row.code == *code)
                .cloned()
                .ok_or_else(|| format!("`{code}` is no row of the catalogue"))
        })
        .collect()
}

fn names(kind: Kind) -> &'static str {
    match kind {
        Kind::Leaf => "the path",
        Kind::Subtree => "the path and every path below it",
        Kind::Below => "every path below it",
        Kind::Carveout => "the path and every path below it, which the enclosing family does not reach",
        Kind::NotAPath => "text that is not a field path",
        _ => "other",
    }
}

fn paths_fields() -> String {
    let mut out = String::from("| Path | Names | Class | Meaning |\n|---|---|---|---|\n");
    for row in &CONFIGURATION_PATHS {
        let _ = writeln!(
            out,
            "| `{}` | {} | {} | {} |",
            cell(row.path),
            names(row.kind),
            row.class.text(),
            cell(row.meaning)
        );
    }
    out
}

fn file_fields() -> String {
    let mut out = String::new();
    for (heading, file) in [
        ("### `candler.toml` (project)", ConfigFile::Project),
        ("### `config.toml` (user)", ConfigFile::User),
    ] {
        let _ = write!(out, "{heading}\n\n| Table | Key | Meaning |\n|---|---|---|\n");
        for row in CONFIGURATION_PATHS.iter().filter(|row| row.file == file) {
            let (table, key) = row.path.split_once('.').unwrap_or((row.path, ""));
            let _ = writeln!(out, "| `{table}` | `{key}` | {} |", cell(row.meaning));
        }
        out.push('\n');
    }
    out
}

/// The page of the configuration paths.
pub fn render_paths(versions: &Versions, existing: Option<&str>) -> Result<String, String> {
    Ok(assemble(
        &PATHS_PAGE,
        PATHS_SURFACE,
        &version_block(versions)?,
        &paths_fields(),
        &entries(&PATHS_CODES)?,
        existing,
    ))
}

/// The page of the configuration files.
pub fn render_file_page(versions: &Versions, existing: Option<&str>) -> Result<String, String> {
    Ok(assemble(
        &FILE_PAGE,
        FILE_SURFACE,
        &version_block(versions)?,
        &file_fields(),
        &entries(&FILE_CODES)?,
        existing,
    ))
}

/// The rows of `EXIT_CODES`, read from the binary's source: `(code, meaning)`.
pub fn exit_codes() -> Result<Vec<(u8, String)>, String> {
    let path = repo_root().join("crates/candler/src/commands/mod.rs");
    let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file = syn::parse_file(&source).map_err(|e| e.to_string())?;
    let array: &ExprArray = file
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(found) if found.ident == "EXIT_CODES" => match &*found.expr {
                Expr::Array(array) => Some(array),
                _ => None,
            },
            _ => None,
        })
        .ok_or("no `EXIT_CODES` array")?;
    array
        .elems
        .iter()
        .map(|elem| {
            let Expr::Tuple(pair) = elem else {
                return Err("a row of `EXIT_CODES` is not a pair".to_owned());
            };
            let mut parts = pair.elems.iter();
            match (parts.next(), parts.next()) {
                (
                    Some(Expr::Lit(ExprLit { lit: Lit::Int(code), .. })),
                    Some(Expr::Lit(ExprLit { lit: Lit::Str(meaning), .. })),
                ) => Ok((
                    code.base10_parse::<u8>().map_err(|e| e.to_string())?,
                    meaning.value(),
                )),
                _ => Err("a row of `EXIT_CODES` is not a number and a string".to_owned()),
            }
        })
        .collect()
}

/// The generated Exit codes section of the index.
pub fn exit_codes_section() -> Result<String, String> {
    let mut out = String::from(
        "<!-- generated:exit-codes -->\n| Code | Meaning |\n|---|---|\n",
    );
    for (code, meaning) in exit_codes()? {
        let _ = writeln!(out, "| {code} | {} |", cell(&meaning));
    }
    out.push_str("<!-- /generated:exit-codes -->");
    Ok(out)
}
