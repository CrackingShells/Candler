//! The driver-traits page, read from the trait definitions in `crates/candler-harness/src`, and the
//! page frame the harness pages share.

use std::fmt::Write;
use std::fs;

use super::capabilities::CAPABILITY_PAGE;
use syn::spanned::Spanned as _;

use super::hand::section;
use super::refusals::{Entry, entries_of_enums, refusals_block};
use super::repo_root;
use super::versions::{CLOSE, OPEN, Versions};

/// One page of the harness surface, as the index lists it.
pub struct HarnessPage {
    /// The file under `docs/contract/`.
    pub file: &'static str,
    /// The surface's name: the page's title and its index row.
    pub title: &'static str,
    /// Who writes the surface, for the index.
    pub writer: &'static str,
}

/// The page of the driver traits.
pub const DRIVER_PAGE: HarnessPage = HarnessPage {
    file: "driver-traits.md",
    title: "Driver traits",
    writer: "driver author",
};

/// The pages of the harness surface, in the order of the index.
pub const HARNESS_PAGES: [&HarnessPage; 2] = [&DRIVER_PAGE, &CAPABILITY_PAGE];

const DRIVER_SURFACE: &str = "The traits a harness driver crate implements: `HarnessDriver` describes how to prepare, launch, stop and collect one harness as data, and `TraceReader` turns its output into sessions; the core calls them and runs what they describe.";

/// The traits of the page, with the file of `candler-harness/src` that defines each.
const TRAITS: [(&str, &str); 2] = [("HarnessDriver", "driver.rs"), ("TraceReader", "reader.rs")];

/// The error types whose refusals a driver can cause: its own, and the command builder's.
const DRIVER_ERRORS: [&str; 2] = ["DriverError", "CommandSpecError"];

/// One item of a trait: its path, its signature after the name, and its default when it has one.
#[derive(Debug, PartialEq, Eq)]
pub struct Item {
    /// `Trait::name`.
    pub path: String,
    /// The signature from the parameter list on.
    pub signature: String,
    /// The tail expression of the default body; `None` for an item with no default.
    pub default: Option<String>,
}

/// The source text of `span`.
fn text_of(source: &str, span: proc_macro2::Span) -> String {
    let (start, end) = (span.start(), span.end());
    let mut out = String::new();
    for (at, line) in source.lines().enumerate() {
        let number = at + 1;
        if number < start.line || number > end.line {
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let from = if number == start.line {
            start.column
        } else {
            0
        };
        let to = if number == end.line {
            end.column
        } else {
            chars.len()
        };
        out.extend(&chars[from..to]);
        out.push(' ');
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The items of the trait `name` in `source`, in order: an item with no default body is
/// `required`, and the default of one with a body is its tail expression.
pub fn trait_items(source: &str, name: &str) -> Result<Vec<Item>, String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    let found = file
        .items
        .iter()
        .find_map(|item| {
            if let syn::Item::Trait(found) = item
                && found.ident == name
            {
                return Some(found);
            }
            None
        })
        .ok_or_else(|| format!("no `pub trait {name}`"))?;
    let items: Vec<Item> = found
        .items
        .iter()
        .filter_map(|item| {
            if let syn::TraitItem::Fn(method) = item {
                return Some(method);
            }
            None
        })
        .map(|method| {
            let sig = text_of(source, method.sig.span());
            let signature = sig
                .split_once('(')
                .map(|(_, rest)| format!("({rest}"))
                .unwrap_or_default()
                .replace("( ", "(")
                .replace(", )", ")");
            let default = method.default.as_ref().map(|body| {
                body.stmts
                    .last()
                    .map(|stmt| text_of(source, stmt.span()))
                    .unwrap_or_default()
                    .trim_end_matches(';')
                    .to_owned()
            });
            Item {
                path: format!("{name}::{}", method.sig.ident),
                signature,
                default,
            }
        })
        .collect();
    if items.is_empty() {
        return Err(format!("the trait {name} holds no item"));
    }
    Ok(items)
}

pub fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The Fields tables of the driver page: one row per trait item.
fn driver_fields() -> Result<String, String> {
    let src = repo_root().join("crates/candler-harness/src");
    let mut out = String::new();
    for (name, file) in TRAITS {
        let path = src.join(file);
        let source = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = write!(
            out,
            "### `{name}`\n\n| Path | Signature | Class | Default |\n|---|---|---|---|\n"
        );
        for item in trait_items(&source, name)? {
            let (class, default) = match &item.default {
                None => ("required", "-".to_owned()),
                Some(body) => ("optional", format!("`{}`", cell(body))),
            };
            let _ = writeln!(
                out,
                "| `{}` | `{}` | {class} | {default} |",
                item.path,
                cell(&item.signature)
            );
        }
        out.push('\n');
    }
    Ok(out)
}

/// The Version block of the harness pages: the core line, which the `candler-harness` crate
/// shares.
pub fn version_section(versions: &Versions) -> Result<String, String> {
    let manifest = repo_root().join("crates/candler-harness/Cargo.toml");
    let text = fs::read_to_string(&manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let document: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    let shared = document
        .get("package")
        .and_then(|package| package.get("version"))
        .and_then(|version| version.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true);
    if !shared {
        return Err(
            "`candler-harness` has a version of its own; the page names one line".to_owned(),
        );
    }
    let core = versions.tag("v").ok_or("no core version")?;
    Ok(format!(
        "<!-- generated:version -->\n{OPEN}\nCore line: {core}; the `candler-harness` crate has the same version.\n{CLOSE}\n<!-- /generated:version -->"
    ))
}

/// The page around the generated parts, with the Surface and Notes text carried over from the
/// existing file.
pub fn assemble(
    page: &HarnessPage,
    surface: &str,
    version: &str,
    fields: &str,
    refusals: &[Entry],
    existing: Option<&str>,
) -> String {
    let surface = existing
        .and_then(|text| section(text, "Surface"))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| surface.to_owned());
    let notes = existing
        .and_then(|text| section(text, "Notes"))
        .unwrap_or_default();
    let rows: Vec<&Entry> = refusals.iter().collect();
    let mut out = format!(
        "# {}\n\n## Surface\n\n{surface}\n\n## Version\n\n{version}\n\n## Fields\n\n<!-- generated:fields -->\n{fields}<!-- /generated:fields -->\n\n## Refusals\n\n{}\n\n## Notes\n",
        page.title,
        refusals_block(&rows),
    );
    if !notes.is_empty() {
        out.push('\n');
        out.push_str(&notes);
        out.push('\n');
    }
    out
}

/// The driver-traits page.
pub fn render_driver_traits(versions: &Versions, existing: Option<&str>) -> Result<String, String> {
    let refusals = entries_of_enums(&DRIVER_ERRORS);
    Ok(assemble(
        &DRIVER_PAGE,
        DRIVER_SURFACE,
        &version_section(versions)?,
        &driver_fields()?,
        &refusals,
        existing,
    ))
}
