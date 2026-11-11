//! The index page: the pages and their surfaces, the field classes and the compatibility table.

use std::fmt::Write;
use std::path::Path;

use super::pages::{PAGES, wire_tags};
use super::refusals::other_refusals;
use super::versions::{CLOSE, OPEN, Versions};

/// The reserved page of the driver contract. The index lists it; nothing here reads, writes or
/// checks the file.
pub const DRIVER_PAGE: &str = "driver-traits.md";

/// The whole index, generated.
pub fn render_index(dir: &Path, versions: &Versions) -> Result<String, String> {
    let mut rows = String::new();
    for page in &PAGES {
        let _ = writeln!(
            rows,
            "| [{file}]({file}) | {title} | {writer} | {tag} |",
            file = page.file,
            title = page.title,
            writer = page.writer,
            tag = wire_tags(dir, page)?,
        );
    }
    let _ = writeln!(
        rows,
        "| [{DRIVER_PAGE}]({DRIVER_PAGE}) | Driver traits | driver author, by hand | none |"
    );
    let core = versions.tag("v").ok_or("no core version")?;
    let schemas = versions.tag("schemas@").ok_or("no schemas version")?;
    let evaluator = versions
        .tag("candler-evaluator@")
        .ok_or("no evaluator version")?;
    let other = other_refusals()?;
    Ok(format!(
        "# Contract pages

One page per surface an author writes or a plug-in speaks. Each page holds the sections Surface, Version, Fields, Refusals and Notes, in that order. Version and Fields are generated from the committed schema files in `crates/candler-schemas/schema/`.

## Pages

| Page | Surface | Written by | Wire tag |
|---|---|---|---|
{rows}
`{DRIVER_PAGE}` is written by hand and is not generated.

## Field classes

Every row of a Fields table has one class. A `required` followed by a variant in parentheses, such as `required (value.kind = fraction)`, is required only when the object takes that variant.

| Class | Meaning |
|---|---|
| `required` | An author sets it, and the object's `required` lists it. |
| `optional` | An author may set it, and `required` does not list it. |
| `candler` | Candler writes it; an author never sets it. |
| `free` | Stored and never read; it needs no schema of its own. |

## Compatibility

{OPEN}
| Line | Version | Manifest |
|---|---|---|
| core | {core} | `Cargo.toml` |
| wire schemas | {schemas} | `crates/candler-schemas/Cargo.toml` |
| evaluator helper | {evaluator} | `python/candler-evaluator/pyproject.toml` |
{CLOSE}

## Other refusals

Surfaces that name no page of their own: configuration, resolving, running, isolation, the store, jobs, evaluation and importing. The path is an RFC 6901 pointer into the document at fault, or a file.

{other}
"
    ))
}
