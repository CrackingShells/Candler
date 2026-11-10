#![cfg(test)]
//! The experiment file of the R03 `cont-train` matrix: its translated TOML validates, its cells are
//! the product of its factors in written order, and a file whose paths overlap is refused.

use std::path::Path;

use candler_schemas::execution::{ExecutionError, ExperimentFile, varying_paths};
use candler_schemas::primitives::Digest;
use candler_schemas::values::{JsonValue, ResolvedValue};
use serde_json::{Value, json};

/// The translated plan, read as text.
fn cont_train() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/experiment/cont_train.toml");
    std::fs::read_to_string(path).expect("cont_train.toml is committed")
}

/// The JSON value a cell holds at `path`, if it holds one.
fn value_at<'a>(cell: &'a [ResolvedValue], path: &str) -> Option<&'a Value> {
    cell.iter()
        // BORROW: .as_str() compares the path text
        .find(|value| value.path().as_str() == path)
        .and_then(|value| value.value())
        .map(JsonValue::as_value)
}

/// The model and the skill version of a cell.
fn axes(cell: &[ResolvedValue]) -> (Value, Value) {
    (
        value_at(cell, "plan.model").cloned().expect("model"),
        value_at(cell, "plan.skill_version")
            .cloned()
            .expect("skill version"),
    )
}

/// Guards: Real-data acceptance — the cells of the `cont-train` experiment are the product of its two factors and differ in nothing else. Oracle: the axes of `cont-train-version-matrix.yaml` (3 models, 4 skill versions, 3 × 4 = 12 cells), counted and ordered by hand.
#[test]
fn cont_train_has_twelve_cells_in_written_order() {
    // BORROW: .as_str() lends the plan text to the parser
    let file = ExperimentFile::from_toml_str(cont_train().as_str()).expect("the plan validates");
    let cells = file.cells();
    assert_eq!(cells.len(), 12, "3 models x 4 skill versions");

    let models = ["claude-haiku-4-5", "claude-sonnet-4-6", "claude-sonnet-5"];
    let skills = [
        "htn-creator@0.2.6",
        "htn-creator@0.2.5",
        "htn-creator@0.2.4",
        "htn-creator@v0.2.3",
    ];
    // The first factor is the slowest: all four skills of a model come before the next model.
    for (index, cell) in cells.iter().enumerate() {
        // INDEX: the model and skill positions are below 3 and 4 because the index is below 12
        let want = (json!(models[index / 4]), json!(skills[index % 4]));
        assert_eq!(axes(cell), want, "cell {index}");
        assert_eq!(cell.len(), 7, "four fixed values, two factors, one cap");
        assert_eq!(value_at(cell, "plan.config"), Some(&json!("with_skill")));
        assert_eq!(value_at(cell, "budget.per_run.wall_s"), Some(&json!(3600)));
    }
    assert_eq!(file.replicates(), 20);

    let varying: Vec<String> = varying_paths(&cells)
        .iter()
        // BORROW: .as_str().to_owned() makes owned path text for the comparison
        .map(|path| path.as_str().to_owned())
        .collect();
    assert_eq!(varying, ["plan.model", "plan.skill_version"]);
}

/// Guards: Real-data acceptance — a fixed value is the same whether the file nests its table or quotes its dotted key. Oracle: the two spellings of `plan.config` written by hand in the test.
#[test]
fn nested_and_quoted_fixed_spellings_agree() {
    let top = "replicates = 1\nschema = \"experiment/1\"\n";
    let tail = "[budget.per_run]\nwall_s = 5\n[harness]\nname = \"h\"\n[treatment]\nname = \"t\"\n";
    let quoted = ExperimentFile::from_toml_str(
        // BORROW: .as_str() lends the built document to the parser
        format!("{top}[fixed]\n\"plan.config\" = \"with_skill\"\n{tail}").as_str(),
    )
    .expect("quoted spelling");
    let nested = ExperimentFile::from_toml_str(
        // BORROW: .as_str() lends the built document to the parser
        format!("{top}[fixed.plan]\nconfig = \"with_skill\"\n{tail}").as_str(),
    )
    .expect("nested spelling");
    assert_eq!(quoted.cells(), nested.cells());
    assert_eq!(
        // INDEX: the plan expands to at least one cell
        value_at(&quoted.cells()[0], "plan.config"),
        Some(&json!("with_skill"))
    );
}

/// Guards: Real-data acceptance — a file whose factor paths overlap a fixed value or one another, or that leaves a cell empty, is refused, and the plain base is accepted. Oracle: the refusals follow from the rule "a path that is another's prefix is the same field", worked by hand on four documents.
#[test]
fn overlapping_or_empty_experiments_are_refused() {
    let top = "replicates = 1\nschema = \"experiment/1\"\n";
    let tail = "[budget.per_run]\nwall_s = 5\n[harness]\nname = \"h\"\n[treatment]\nname = \"t\"\n";
    // BORROW: .as_str() lends the built document to the parser
    let read = |head: &str| ExperimentFile::from_toml_str(format!("{top}{head}{tail}").as_str());

    let base = "[fixed]\n\"plan.a\" = 1\n[[factors]]\npath = \"plan.b\"\nlevels = [1, 2]\n";
    assert_eq!(read(base).expect("base").cells().len(), 2);

    let fixed_overlap = "[fixed]\n\"plan.a\" = 1\n[[factors]]\npath = \"plan\"\nlevels = [1, 2]\n";
    assert!(matches!(
        read(fixed_overlap),
        Err(ExecutionError::Document { .. })
    ));

    let factors_overlap = "[[factors]]\npath = \"plan.a\"\nlevels = [1]\n[[factors]]\npath = \"plan.a\"\nlevels = [2]\n";
    assert!(matches!(
        read(factors_overlap),
        Err(ExecutionError::Document { .. })
    ));

    let no_levels = "[[factors]]\npath = \"plan.a\"\nlevels = []\n";
    assert!(matches!(
        read(no_levels),
        Err(ExecutionError::Document { .. })
    ));

    let fill_overlap =
        "[fixed]\n\"plan.a.x\" = 1\n[[factors]]\npath = \"plan.a\"\nlevels = [{ x = 1 }]\n";
    assert!(matches!(
        read(fill_overlap),
        Err(ExecutionError::Document { .. })
    ));
}

/// Guards: Experiment file — a file with no new field and no table under `prompt.values` lists the cells it listed before, so no cell digest and no resume check moves. Oracle: the cell digests recorded from the b017d31 code before this change, in `vectors/experiment/legacy_cells.toml`.
#[test]
fn unchanged_cells_keep_their_digests() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/experiment");
    let text = std::fs::read_to_string(dir.join("legacy_cells.toml")).expect("committed");
    // BORROW: .as_str() lends the vector text to the parser
    let doc: toml::Table = toml::from_str(text.as_str()).expect("a TOML vector file");
    let cases = doc
        .get("case")
        .and_then(toml::Value::as_array)
        .expect("cases");
    assert!(cases.len() >= 4, "every recorded text is present");
    for case in cases {
        let case = case.as_table().expect("a case table");
        let name = case
            .get("name")
            .and_then(toml::Value::as_str)
            .expect("a name");
        let experiment = if let Some(text) = case.get("text").and_then(toml::Value::as_str) {
            text.to_owned()
        } else {
            let file = case
                .get("file")
                .and_then(toml::Value::as_str)
                .expect("a text or a file");
            std::fs::read_to_string(dir.join(file)).expect("the named file is committed")
        };
        // BORROW: .as_str() lends the experiment text to the parser
        let file = ExperimentFile::from_toml_str(experiment.as_str()).expect(name);
        let got: Vec<String> = file
            .cells()
            .iter()
            .map(|cell| {
                let json = serde_json::to_value(cell).expect("a cell is JSON");
                Digest::of_canonical(&json)
                    .expect("a cell has a canonical form")
                    // BORROW: .as_str().to_owned() makes owned digest text
                    .as_str()
                    .to_owned()
            })
            .collect();
        let want: Vec<String> = case
            .get("cells")
            .and_then(toml::Value::as_array)
            .expect("recorded cells")
            .iter()
            .map(|digest| digest.as_str().expect("digest text").to_owned())
            .collect();
        assert_eq!(got, want, "{name}: the cells moved");
    }
}
