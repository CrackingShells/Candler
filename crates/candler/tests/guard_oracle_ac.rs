#![cfg(test)]
//! OR1: a second engine over the raw golden corpus re-derives the A-C reference files of
//! `tests/oracle/`. It reads `serde_json::Value` only, and no Candler crate.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// The `gate` expectations among the A-C ids, copied by hand from the text of `eval.json` at
/// `28be2c15` (sha256 `ca7f12af...`): one id list per role.
const GATE: [&str; 16] = [
    "A1", "A2", "B7", "B8", "D1", "C-N1", "C-N2", "C-N3", "C-X1", "C-X3", "C-X4", "C-XN1", "C-H1",
    "C-H2", "C-M1", "C-M2",
];
/// The `quality` expectations among the A-C ids, copied by hand from `eval.json` at `28be2c15`.
const QUALITY: [&str; 7] = ["A3", "A5", "A6", "B1", "B2", "B3", "B9"];
/// `_COUNTED_STATUSES` of `htn_creator_eval/_outcome.py` at `28be2c15`, line 91.
const COUNTED: [&str; 3] = ["pass", "fail", "blocked"];
/// The Tier D expectations of `eval.json`: the uncompared set.
const UNCOMPARED: [&str; 6] = ["D-E1", "D-E2", "D-E3", "D-E4", "D-E5", "D-E6"];

struct Row {
    cell: String,
    run: u32,
    id: String,
    status: String,
    score: i64,
    max_score: i64,
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    out.sort();
    out
}

fn name_of(path: &Path) -> String {
    // BORROW: `.into_owned()` turns the lossy file-name view into an owned `String`
    path.file_name()
        .expect("file name")
        .to_string_lossy()
        .into_owned()
}

fn int_of(value: &Value, key: &str) -> i64 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("integer `{key}` missing"))
}

fn text_of(value: &Value, key: &str) -> String {
    // BORROW: `.to_owned()` copies the borrowed text into an owned `String`
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("text `{key}` missing"))
        .to_owned()
}

/// Every grading row of every `eval-K-grading.json` under a cell's `with_skill/run-N` folders.
fn read_rows(corpus: &Path) -> Vec<Row> {
    let mut rows = Vec::new();
    for cell in sorted_entries(corpus) {
        if !name_of(&cell).starts_with("20") || !cell.join("experiment.json").is_file() {
            continue;
        }
        for eval in sorted_entries(&cell) {
            let runs = eval.join("with_skill");
            if !name_of(&eval).starts_with("eval-") || !runs.is_dir() {
                continue;
            }
            for run in sorted_entries(&runs) {
                let run_name = name_of(&run);
                let Some(n) = run_name
                    .strip_prefix("run-")
                    .and_then(|s| s.parse::<u32>().ok())
                else {
                    continue;
                };
                for file in sorted_entries(&run) {
                    let file_name = name_of(&file);
                    if !(file_name.starts_with("eval-") && file_name.ends_with("-grading.json")) {
                        continue;
                    }
                    let text = fs::read_to_string(&file).expect("grading file");
                    let doc: Value = serde_json::from_str(&text).expect("grading JSON");
                    let expectations = doc
                        .get("expectations")
                        .and_then(Value::as_array)
                        .expect("expectations");
                    for e in expectations {
                        rows.push(Row {
                            cell: name_of(&cell),
                            run: n,
                            id: text_of(e, "id"),
                            status: text_of(e, "status"),
                            score: int_of(e, "score"),
                            max_score: int_of(e, "max_score"),
                        });
                    }
                }
            }
        }
    }
    rows
}

fn read_text(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("oracle")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn toml_int(doc: &toml::Table, section: &str, key: &str) -> i64 {
    doc.get(section)
        .and_then(|t| t.get(key))
        .and_then(toml::Value::as_integer)
        .unwrap_or_else(|| panic!("counts.toml lacks {section}.{key}"))
}

fn toml_pair(value: Option<&toml::Value>) -> (i64, i64) {
    let items = value.and_then(toml::Value::as_array).expect("[num, den]");
    let mut ints = items.iter().map(|v| v.as_integer().expect("integer"));
    let pair = (ints.next().expect("num"), ints.next().expect("den"));
    assert!(ints.next().is_none(), "a pair has two items");
    pair
}

/// Guards: R3 — the A-C counts, the 209 zero-weight pairs (by cell hex, through `cells.tsv`), every cell's gate and quality sums and
/// the rows of `equivalence.json` that `gen_oracle.py` committed equal a second derivation from
/// the raw grading files. Oracle: the raw corpus read through `serde_json::Value`, with the role
/// ids of `eval.json` and the counted statuses of `_outcome.py` at `28be2c15` as hand literals.
#[test]
#[ignore = "needs CANDLER_GOLDEN_CORPUS, the extracted campaign/20260806-cont-train folder"]
fn oracle_ac_rederived() {
    let root = std::env::var("CANDLER_GOLDEN_CORPUS")
        .expect("CANDLER_GOLDEN_CORPUS must name the extracted campaign/20260806-cont-train");
    let rows = read_rows(Path::new(&root));
    let is_ac = |r: &Row| !UNCOMPARED.contains(&r.id.as_str());
    let ac: Vec<&Row> = rows.iter().filter(|r| is_ac(r)).collect();

    // Counts.
    let counts: toml::Table = read_text("counts.toml").parse().expect("counts.toml");
    let status = |s: &str| ac.iter().filter(|r| r.status == s).count();
    let zero: Vec<&&Row> = ac
        .iter()
        .filter(|r| r.status == "pass" && r.max_score == 0)
        .collect();
    let as_i64 = |n: usize| i64::try_from(n).expect("count fits i64");
    assert_eq!(rows.len(), 10_320);
    assert_eq!(
        counts.get("total").and_then(toml::Value::as_integer),
        Some(as_i64(rows.len()))
    );
    assert_eq!(
        toml_int(&counts, "imported", "pass"),
        as_i64(status("pass"))
    );
    assert_eq!(
        toml_int(&counts, "imported", "zero_weight"),
        as_i64(zero.len())
    );
    assert_eq!(
        toml_int(&counts, "imported", "fail"),
        as_i64(status("fail"))
    );
    assert_eq!(
        toml_int(&counts, "imported", "blocked"),
        as_i64(status("blocked"))
    );
    assert_eq!(
        toml_int(&counts, "imported", "not_measured"),
        as_i64(status("not_measured"))
    );
    assert_eq!(status("not_applicable"), 0);
    assert_eq!(
        toml_int(&counts, "live_expected", "pass"),
        as_i64(status("pass") - zero.len())
    );
    assert_eq!(
        toml_int(&counts, "live_expected", "not_applicable"),
        as_i64(zero.len())
    );
    assert_eq!(
        toml_int(&counts, "live_expected", "fail"),
        as_i64(status("fail"))
    );
    assert_eq!(
        toml_int(&counts, "live_expected", "blocked"),
        as_i64(status("blocked"))
    );
    assert_eq!(
        toml_int(&counts, "live_expected", "not_measured"),
        as_i64(status("not_measured"))
    );
    for id in UNCOMPARED {
        let n = rows.iter().filter(|r| r.id == id).count();
        assert_eq!(toml_int(&counts, "uncompared", id), as_i64(n), "{id}");
    }

    // `cells.tsv` names each of the corpus's cell directories exactly once.
    let mut dirs: Vec<&str> = rows.iter().map(|r| r.cell.as_str()).collect();
    dirs.sort_unstable();
    dirs.dedup();
    assert_eq!(dirs.len(), 12);
    let cells_tsv = read_text("cells.tsv");
    let mut hex_of: BTreeMap<&str, &str> = BTreeMap::new();
    for line in cells_tsv.lines() {
        let (hex, dir) = line.split_once('\t').expect("`<hex>\\t<dir>`");
        assert!(
            hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()),
            "{hex}"
        );
        assert!(
            hex_of.insert(dir, hex).is_none(),
            "directory {dir} listed twice"
        );
    }
    assert_eq!(hex_of.keys().copied().collect::<Vec<_>>(), dirs);
    assert_eq!(
        hex_of.values().collect::<BTreeSet<_>>().len(),
        12,
        "a hex names two directories"
    );
    let hex = |cell: &str| *hex_of.get(cell).expect("cell in cells.tsv");

    // The zero-weight pairs, keyed by cell hex.
    let mut pairs: Vec<String> = zero
        .iter()
        .map(|r| format!("{}\t{}\t{}", hex(&r.cell), r.run, r.id))
        .collect();
    pairs.sort();
    let committed: Vec<String> = read_text("na_pairs.txt")
        .lines()
        .map(String::from)
        .collect();
    assert_eq!(committed, pairs);

    // The per-cell gate and quality sums.
    let mut sums: BTreeMap<&str, [i64; 4]> = BTreeMap::new();
    for r in &ac {
        if !COUNTED.contains(&r.status.as_str()) {
            continue;
        }
        let offset = if GATE.contains(&r.id.as_str()) {
            0
        } else if QUALITY.contains(&r.id.as_str()) {
            2
        } else {
            continue;
        };
        let cell = sums.entry(r.cell.as_str()).or_insert([0; 4]);
        for (slot, add) in cell.iter_mut().skip(offset).zip([r.score, r.max_score]) {
            *slot += add;
        }
    }
    let rates: toml::Table = read_text("rates_by_cell.toml")
        .parse()
        .expect("rates_by_cell.toml");
    let table = rates
        .get("cell")
        .and_then(toml::Value::as_table)
        .expect("[cell.*] tables");
    assert_eq!(table.len(), sums.len());
    for (cell, [gn, gd, qn, qd]) in &sums {
        let t = table
            .get(hex(cell))
            .unwrap_or_else(|| panic!("cell {cell} missing"));
        assert_eq!(toml_pair(t.get("gate")), (*gn, *gd), "gate of {cell}");
        assert_eq!(toml_pair(t.get("quality")), (*qn, *qd), "quality of {cell}");
    }

    // The equivalence rows, as a multiset of (N, expectation); the cell hex is GP2's.
    let eq: Value = serde_json::from_str(&read_text("equivalence.json")).expect("equivalence.json");
    let rules = eq.get("rules").and_then(Value::as_array).expect("rules");
    assert_eq!(rules.len(), 1);
    let listed = rules
        .first()
        .and_then(|r| r.get("rows"))
        .and_then(Value::as_array)
        .expect("rows");
    let mut got: Vec<(u32, String)> = listed
        .iter()
        .map(|row| {
            let run = text_of(row, "run");
            let (hex, tail) = run.rsplit_once("-r").expect("run key `<hex>-r<N>-a1`");
            assert!(
                hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()),
                "{run}"
            );
            let n = tail
                .strip_suffix("-a1")
                .and_then(|s| s.parse().ok())
                .expect("replicate");
            (n, text_of(row, "expectation"))
        })
        .collect();
    got.sort();
    let mut want: Vec<(u32, String)> = zero.iter().map(|r| (r.run, r.id.clone())).collect();
    want.sort();
    assert_eq!(got, want);
}
