#![cfg(test)]
//! Guards for the contract pages under `docs/contract/`: the generated sections equal what the
//! committed schema files and manifests give, the five sections hold in order, and the versions the
//! pages quote equal the manifests.
//!
//! Setting `CANDLER_BLESS_CONTRACT=1` writes the pages and the index instead of comparing them.

use crate::drivers;
use crate::support::contract_pages;

use std::collections::BTreeMap;
use std::fs;

use contract_pages::capabilities::render_capabilities;
use contract_pages::frame::{MAX_NOTE_WORDS, SECTIONS, check_frame, check_notes};
use contract_pages::hand::section;
use contract_pages::harness::{HARNESS_PAGES, render_driver_traits};
use contract_pages::index::render_index;
use contract_pages::pages::{PAGES, blocks_markdown, render_page, root_fields};
use contract_pages::refusals::{
    Home, all_entries, binary_entries, catalogue_entries, codes_named_in, home_of, known_codes,
    listed_codes,
};
use contract_pages::versions::{Versions, check_blocks, tokens, tokens_outside_blocks};
use contract_pages::{blessing, docs_dir, repo_root, schema_dir, write_atomically};

fn versions() -> Versions {
    Versions::read(&repo_root()).expect("the manifests are readable")
}

/// Compares a file with its generated text, or writes it when blessing.
fn settle(name: &str, generated: &str) {
    let path = docs_dir().join(name);
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() == Some(generated) {
        return;
    }
    if blessing() {
        write_atomically(&path, generated).expect("the page is written");
        return;
    }
    match current {
        None => {
            panic!("docs/contract/{name} does not exist; regenerate with CANDLER_BLESS_CONTRACT=1")
        }
        Some(current) => {
            let at = current
                .lines()
                .zip(generated.lines())
                .position(|(a, b)| a != b)
                .unwrap_or(current.lines().count().min(generated.lines().count()));
            panic!(
                "docs/contract/{name} differs from the generated page at line {}; regenerate with CANDLER_BLESS_CONTRACT=1",
                at + 1
            );
        }
    }
}

fn existing(name: &str) -> Option<String> {
    fs::read_to_string(docs_dir().join(name)).ok()
}

/// The files of every page that holds the five sections: the schema pages, then the harness pages.
fn page_files() -> Vec<&'static str> {
    PAGES
        .iter()
        .map(|page| page.file)
        .chain(HARNESS_PAGES.iter().map(|page| page.file))
        .collect()
}

/// Guards: D1 — regenerating the six schema pages, the two harness pages and the index from the
/// committed schema files, the manifests, the trait definitions and the binary's driver registry
/// changes no byte of them; the hand text (Surface, Notes) is carried over.
/// Oracle: the committed files under `docs/contract/`.
/// Bite: a property added to a copy of a committed schema file without regenerating; a cell of a
/// Fields table edited by hand; a driver declaring a capability it did not (`OpenCode` declaring
/// `session`); a method added to a trait.
#[test]
fn d1_regenerating_the_pages_changes_nothing() {
    let versions = versions();
    for page in &PAGES {
        let current = existing(page.file);
        let generated = render_page(&schema_dir(), page, &versions, current.as_deref())
            .expect("the page is rendered");
        settle(page.file, &generated);
    }
    let registry = drivers::Registry::builtin().expect("the drivers are built");
    let traits = render_driver_traits(&versions, existing("driver-traits.md").as_deref())
        .expect("the driver traits page is rendered");
    settle("driver-traits.md", &traits);
    let capabilities = render_capabilities(
        &versions,
        &registry,
        existing("harness-capabilities.md").as_deref(),
    )
    .expect("the capability page is rendered");
    settle("harness-capabilities.md", &capabilities);
    let index = render_index(&schema_dir(), &versions).expect("the index is rendered");
    settle("index.md", &index);
}

/// The class column of a root's generated Fields tables, by path, read back from the markdown.
fn classes_of(root: &str) -> BTreeMap<String, Vec<String>> {
    let blocks = root_fields(&schema_dir(), root).expect("the root is walked");
    let markdown = blocks_markdown(&blocks, 4);
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in markdown.lines().filter(|line| line.starts_with("| `")) {
        let cells: Vec<&str> = line.trim_matches('|').split(" | ").collect();
        let path = cells.first().expect("a path cell").trim().trim_matches('`');
        // A `required` field of some variants only names them after the class word.
        let class = cells
            .get(2)
            .expect("a class cell")
            .split(" (")
            .next()
            .unwrap_or_default()
            .trim();
        out.entry(path.to_owned())
            .or_default()
            .push(class.to_owned());
    }
    out
}

/// Guards: D1 — the class of every property of the nine schema files, as the generated Fields table
/// gives it, equals the hand table's.
/// Oracle: `crates/candler-schemas/tests/vectors/field_classes.toml`, classified by hand.
/// Bite: a class edited in the hand table; the class rule changed to ignore `x-candler-class`.
#[test]
fn d1_fields_agree_with_the_hand_classes() {
    let text = fs::read_to_string(
        repo_root().join("crates/candler-schemas/tests/vectors/field_classes.toml"),
    )
    .expect("the hand table is readable");
    let table: toml::Table = text.parse().expect("the hand table is toml");
    let mut checked = 0;
    for (file, rows) in &table {
        assert!(
            PAGES.iter().any(|page| page.roots.contains(&file.as_str())),
            "`{file}` belongs to no page"
        );
        let generated = classes_of(file);
        for (path, class) in rows.as_table().expect("a table of rows") {
            let want = class.as_str().expect("a class word");
            let got = generated
                .get(path)
                .unwrap_or_else(|| panic!("{file}: the generated table has no row `{path}`"));
            assert!(
                got.iter().all(|got| got == want),
                "{file}: `{path}` is {got:?} in the generated table and {want} by hand"
            );
            checked += 1;
        }
    }
    assert!(checked > 300, "the hand table has {checked} rows");
}

/// A page with the five sections, the given Notes text under the last.
fn frame_page(notes: &str) -> String {
    let mut out = String::from("# Page\n");
    for section in SECTIONS {
        out.push_str("\n## ");
        out.push_str(section);
        out.push_str("\n\ntext\n");
    }
    out.truncate(out.rfind("text\n").expect("the last section has text"));
    out.push_str(notes);
    out
}

/// Guards: D2 — each real page has the five `##` sections in order and no other, and its Notes are
/// stamped and within 120 words; the checker refuses a page with an extra section, sections out of
/// order, a missing section, a Notes section of 121 words, an unstamped note or a stamp with an
/// unknown line. The index has no five sections and is not checked here.
/// Oracle: the section names and limits written by hand in the rows below.
/// Bite: an extra `##` section in a page; a note without a stamp; a Notes section of 121 words;
/// two sections swapped.
#[test]
fn d2_each_page_has_the_five_sections_in_order() {
    let stamp = "[checked: schemas@0.6.0]";
    let words = |n: usize| format!("- {} {stamp}\n", "word ".repeat(n).trim_end());
    assert_eq!(MAX_NOTE_WORDS, 120);

    let good = frame_page(&words(120));
    check_frame(&good).expect("a page with 120 words passes");
    check_frame(&frame_page("")).expect("an empty Notes passes");
    assert!(check_frame(&frame_page(&words(121))).is_err(), "121 words");
    assert!(
        check_frame(&frame_page("- a note with no stamp\n")).is_err(),
        "unstamped"
    );
    assert!(
        check_frame(&frame_page("- a note [checked: foo@1.0.0]\n")).is_err(),
        "a stamp of an unknown line"
    );
    assert!(
        check_frame(&frame_page("a loose line\n")).is_err(),
        "not a bullet"
    );
    check_frame(&frame_page(
        "- a note [checked: v@0.6.0]\n- another [checked: candler-evaluator@0.1.0]\n",
    ))
    .expect("each of the three lines stamps");
    assert!(
        check_frame(&format!("{good}\n## Extra\n")).is_err(),
        "an extra section"
    );
    assert!(
        check_frame(
            &good
                .replace("## Fields", "## Tmp")
                .replace("## Refusals", "## Fields")
                .replace("## Tmp", "## Refusals")
        )
        .is_err(),
        "sections out of order"
    );
    assert!(
        check_frame(&good.replace("## Refusals\n\ntext\n", "")).is_err(),
        "a missing section"
    );
    check_frame(&good.replace("## Fields\n\ntext\n", "## Fields\n\n### One root\n\ntext\n"))
        .expect("a `###` heading is allowed");
    check_notes("").expect("no notes");

    for file in page_files() {
        let text = existing(file).unwrap_or_else(|| panic!("{file} does not exist"));
        if let Err(e) = check_frame(&text) {
            panic!("{file}: {e}");
        }
    }
}

/// Guards: D4 — every version number the pages quote sits inside a `<!-- versions -->` block and
/// equals its manifest's: the core `v` the workspace `version` of `Cargo.toml`, `schemas@` that of
/// `crates/candler-schemas/Cargo.toml`, `candler-evaluator@` that of the helper's `pyproject.toml`;
/// the index's table holds all three.
/// Oracle: the three manifests.
/// Bite: a stale version in the index table; a version number outside a block.
#[test]
fn d4_compatibility_table_equals_the_manifests() {
    let versions = versions();
    let index = existing("index.md").expect("docs/contract/index.md exists");
    check_blocks(&index, &versions).expect("the index's table equals the manifests");
    let lines: Vec<String> = tokens(&index)
        .expect("tokens")
        .into_iter()
        .map(|(line, _)| line)
        .collect();
    for line in ["v", "schemas@", "candler-evaluator@"] {
        assert!(
            lines.iter().any(|found| found == line),
            "the table has no `{line}` row"
        );
    }
    for file in page_files() {
        let text = existing(file).unwrap_or_else(|| panic!("{file} does not exist"));
        check_blocks(&text, &versions).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert!(
            tokens_outside_blocks(&text).expect("blocks").is_empty(),
            "{file}: a version number sits outside a versions block"
        );
    }
}

/// Guards: D4 — a stale version in a block is refused, a block with no stale version passes, and a
/// version outside a block is found.
/// Oracle: the versions written by hand below.
/// Bite: the comparison made to ignore the line; a stale `schemas@` left in the table.
#[test]
fn d4_a_stale_version_is_refused() {
    let versions = Versions {
        core: "1.2.3".to_owned(),
        schemas: "4.5.6".to_owned(),
        evaluator: "7.8.9".to_owned(),
    };
    let table = |core: &str, schemas: &str| {
        format!(
            "<!-- versions -->\n| core | `v{core}` |\n| wire | `schemas@{schemas}` |\n| helper | `candler-evaluator@7.8.9` |\n<!-- /versions -->\n"
        )
    };
    check_blocks(&table("1.2.3", "4.5.6"), &versions).expect("equal versions pass");
    assert!(
        check_blocks(&table("1.2.3", "4.5.5"), &versions).is_err(),
        "a stale schemas@"
    );
    assert!(
        check_blocks(&table("1.2.4", "4.5.6"), &versions).is_err(),
        "a stale core"
    );
    assert!(
        check_blocks("<!-- versions -->\n`v1.2.3`\n", &versions).is_err(),
        "an open block"
    );
    let outside = format!("`v9.9.9`\n{}", table("1.2.3", "4.5.6"));
    assert_eq!(
        tokens_outside_blocks(&outside).expect("blocks"),
        vec![("v".to_owned(), "9.9.9".to_owned())]
    );
}

/// The codes a page names: those in the first cell of a row of its Refusals, and those in
/// backticks in its Notes. A sentence of a row may name a setting such as `store.root`, which is no
/// code.
fn named_codes(file: &str) -> Vec<String> {
    let text = existing(file).unwrap_or_else(|| panic!("{file} does not exist"));
    let mut codes = listed_codes(&text);
    codes.extend(codes_named_in(&section(&text, "Notes").unwrap_or_default()));
    codes
}

/// Guards: D3 — every code in the first cell of a row of a page's Refusals, or of the index's list
/// of other refusals, and every code in backticks in a Note, is a row of the code's catalogue or of the registry's `[helper]`
/// section; the finder reads a backticked `<surface>.<reason>` and nothing else.
/// Oracle: `candler_core::refusal::catalog()` and `docs/contract/codes.toml`.
/// Bite: a code deleted from the catalogue and left in a page; a code in a Note that no row has.
#[test]
fn d3_every_code_on_a_page_is_in_the_catalog() {
    assert_eq!(
        codes_named_in("`experiment.no_such_code` is `direct_url.json` in `a b.c` and `plugin.`"),
        vec!["experiment.no_such_code".to_owned()],
        "the finder reads a backticked code of a named surface and nothing else"
    );
    let known = known_codes().expect("the codes are read");
    let mut files = page_files();
    files.push("index.md");
    for file in files {
        for code in named_codes(file) {
            assert!(
                known.contains(&code),
                "{file} names `{code}`, which is no row of the catalogue or of [helper]"
            );
        }
    }
}

/// Guards: D3 — every surface of the catalogue is named by the surface table; every code of a side
/// other than Candler's, and every `[helper]` code, is listed in the Refusals of the page the table
/// gives its surface, or in the index's other refusals.
/// Oracle: `candler_core::refusal::catalog()` and `docs/contract/codes.toml`; the surface table,
/// written by hand in `support/contract_pages/refusals.rs`.
/// Bite: an author code with neither page nor index entry; a surface the table does not name.
#[test]
fn d3_every_author_code_has_a_page() {
    let mut surfaced = catalogue_entries();
    surfaced.extend(binary_entries().expect("the binary's rows are read"));
    for row in surfaced {
        assert!(
            home_of(row.surface()).is_some(),
            "the surface `{}` of `{}` is not named by the surface table",
            row.surface(),
            row.code
        );
    }
    let mut listed: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in page_files() {
        let text = existing(file).unwrap_or_else(|| panic!("{file} does not exist"));
        listed.insert(file.to_owned(), listed_codes(&text));
    }
    let index = existing("index.md").expect("docs/contract/index.md exists");
    listed.insert("index.md".to_owned(), listed_codes(&index));
    let mut checked = 0;
    for row in all_entries().expect("the rows are read") {
        let home = home_of(row.surface())
            .unwrap_or_else(|| panic!("the surface `{}` is not named", row.surface()));
        let file = match home {
            Home::Page(file) => file,
            Home::Index => "index.md",
        };
        assert!(
            listed
                .get(file)
                .is_some_and(|codes| codes.contains(&row.code)),
            "`{}` is not listed in the Refusals of {file}",
            row.code
        );
        checked += 1;
    }
    assert!(checked > 150, "{checked} codes are checked");
}
