//! The pages, and the renderer that writes the generated sections around the hand text.

use std::fmt::Write;
use std::path::Path;

use super::hand::section;
use super::schema::{self, Block, Class, Row};
use super::versions::{CLOSE, OPEN, Versions};

/// One contract page.
pub struct Page {
    /// The file under `docs/contract/`.
    pub file: &'static str,
    /// The surface's name: the page's title and its index row.
    pub title: &'static str,
    /// Who writes the surface, for the index.
    pub writer: &'static str,
    /// The Surface sentence a new page starts with: what the surface is, who writes it, who reads it.
    pub surface: &'static str,
    /// The schema files, by name, whose properties the Fields section lists.
    pub roots: &'static [&'static str],
    /// Whether the page is the evaluator helper's: no schema of its own, the fields of the outcome
    /// record that Candler does not fill, and the helper's version.
    pub helper: bool,
}

/// The six pages, in the order of the index.
pub const PAGES: [Page; 6] = [
    Page {
        file: "plugin-manifest.md",
        title: "Plug-in manifest",
        writer: "plug-in author",
        surface: "The manifest a plug-in author writes beside a plug-in so that Candler can read its name, version, protocol, command and environment without running anything.",
        roots: &["plugin-manifest"],
        helper: false,
    },
    Page {
        file: "suite-file.md",
        title: "Suite file",
        writer: "suite author",
        surface: "The suite file an evaluation author writes to list the expectations a run is graded against; Candler reads it to choose and run the evaluators.",
        roots: &["suite"],
        helper: false,
    },
    Page {
        file: "evaluator-protocol.md",
        title: "Evaluator protocol",
        writer: "Candler and the evaluator plug-in",
        surface: "The messages Candler and an evaluator plug-in exchange, one JSON object per line: requests from the core, notifications from either side, and the evaluator's responses.",
        roots: &[
            "evaluator-describe-result",
            "evaluator-evaluate-params",
            "evaluator-message",
            "evaluator-notification",
            "evaluator-request-side",
        ],
        helper: false,
    },
    Page {
        file: "outcome-record.md",
        title: "Outcome record",
        writer: "evaluator plug-in; Candler fills the rest",
        surface: "The result of one evaluator for one expectation of one run: an evaluator reports the fields it owns, Candler adds the fields marked `candler` and stores the record.",
        roots: &["outcome-record"],
        helper: false,
    },
    Page {
        file: "experiment-file.md",
        title: "Experiment file",
        writer: "experimenter",
        surface: "The experiment file an experimenter writes to fix the values, factors, replicates, budget, harness and treatment of a set of runs; Candler reads it to plan and run the cells.",
        roots: &["experiment"],
        helper: false,
    },
    Page {
        file: "evaluator-helper.md",
        title: "Evaluator helper",
        writer: "evaluator kind author",
        surface: "The Python package `candler-evaluator`, which an author of an evaluator kind uses to speak the protocol; a kind returns the fields of an outcome record that Candler does not fill.",
        roots: &[],
        helper: true,
    },
];

/// The page's wire tags, from the `schema` const of its roots; `none` when no root has one.
pub fn wire_tags(dir: &Path, page: &Page) -> Result<String, String> {
    let mut tags = Vec::new();
    for root in page.roots {
        if let Some(tag) = schema::wire_tag(&schema::load(dir, root)?) {
            tags.push(format!("`{tag}`"));
        }
    }
    Ok(if tags.is_empty() {
        "none".to_owned()
    } else {
        tags.join(", ")
    })
}

/// The Version section: the wire tag, then the version numbers inside one block.
fn version_section(dir: &Path, page: &Page, versions: &Versions) -> Result<String, String> {
    let tag = wire_tags(dir, page)?;
    let (label, line) = if page.helper {
        ("Package", "candler-evaluator@")
    } else {
        ("Wire line", "schemas@")
    };
    let span = versions
        .tag(line)
        .ok_or_else(|| format!("no version for `{line}`"))?;
    Ok(format!(
        "<!-- generated:version -->\nWire tag: {tag}.\n\n{OPEN}\n{label}: {span}.\n{CLOSE}\n<!-- /generated:version -->"
    ))
}

/// The cell text of a row, with `|` escaped.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// The rows of a block as a table.
fn table(rows: &[Row]) -> String {
    let mut out = String::from("| Path | Type | Class | Default |\n|---|---|---|---|\n");
    for row in rows {
        let default = if row.default == "-" {
            "-".to_owned()
        } else {
            format!("`{}`", cell(&row.default))
        };
        let _ = writeln!(
            out,
            "| `{}` | `{}` | {} | {} |",
            cell(&row.path),
            cell(&row.ty.join(" | ")),
            row.class.as_str(),
            default
        );
    }
    out
}

/// The blocks of one root as headed tables, the headings `level` hashes deep.
pub fn blocks_markdown(blocks: &[Block], level: usize) -> String {
    let mut out = String::new();
    for block in blocks {
        if let Some(title) = &block.title {
            let _ = write!(out, "{} `{title}`\n\n", "#".repeat(level));
        }
        out.push_str(&table(&block.rows));
        out.push('\n');
    }
    out
}

/// Drops the rows of a candler-class field and of everything under one.
pub fn without_candler(blocks: Vec<Block>) -> Vec<Block> {
    blocks
        .into_iter()
        .map(|block| {
            let owned: Vec<String> = block
                .rows
                .iter()
                .filter(|row| row.class == Class::Candler)
                .map(|row| row.path.clone())
                .collect();
            let rows = block
                .rows
                .into_iter()
                .filter(|row| {
                    !owned.iter().any(|path| {
                        row.path == *path
                            || row.path.starts_with(&format!("{path}."))
                            || row.path.starts_with(&format!("{path}[]"))
                    })
                })
                .collect();
            Block {
                title: block.title,
                rows,
            }
        })
        .collect()
}

/// The generated Fields tables of one schema root, with no page around them.
pub fn root_fields(dir: &Path, root: &str) -> Result<Vec<Block>, String> {
    schema::fields(&schema::load(dir, root)?)
}

fn fields_section(dir: &Path, page: &Page) -> Result<String, String> {
    let mut body = String::new();
    if page.helper {
        let blocks = without_candler(root_fields(dir, "outcome-record")?);
        body.push_str(&blocks_markdown(&blocks, 3));
    } else if let [only] = page.roots {
        body.push_str(&blocks_markdown(&root_fields(dir, only)?, 3));
    } else {
        for root in page.roots {
            let _ = write!(body, "### `{root}`\n\n");
            body.push_str(&blocks_markdown(&root_fields(dir, root)?, 4));
        }
    }
    Ok(format!(
        "<!-- generated:fields -->\n{body}<!-- /generated:fields -->"
    ))
}

/// The whole page: the generated Version and Fields, the Refusals placeholder, and the Surface and
/// Notes text carried over from the existing file (a new page starts with its own sentence).
pub fn render_page(
    dir: &Path,
    page: &Page,
    versions: &Versions,
    existing: Option<&str>,
) -> Result<String, String> {
    let surface = existing
        .and_then(|text| section(text, "Surface"))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| page.surface.to_owned());
    let notes = existing
        .and_then(|text| section(text, "Notes"))
        .unwrap_or_default();
    let mut out = format!(
        "# {}\n\n## Surface\n\n{surface}\n\n## Version\n\n{}\n\n## Fields\n\n{}\n\n## Refusals\n\n<!-- generated:refusals -->\n\n## Notes\n",
        page.title,
        version_section(dir, page, versions)?,
        fields_section(dir, page)?,
    );
    if !notes.is_empty() {
        out.push('\n');
        out.push_str(&notes);
        out.push('\n');
    }
    Ok(out)
}
