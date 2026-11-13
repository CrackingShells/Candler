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

/// An experiment document: the head holds the `[fixed]` and factor tables, and the tail the required ones.
fn document(head: &str) -> String {
    format!(
        "schema = \"experiment/1\"\nreplicates = 1\n{head}\n[budget.per_run]\nwall_s = 5\n[harness]\nname = \"h\"\n[treatment]\nname = \"t\"\n"
    )
}

/// The cells of the document `head` completes.
fn cells_of(head: &str) -> Vec<Vec<ResolvedValue>> {
    // BORROW: .as_str() lends the built document to the parser
    ExperimentFile::from_toml_str(document(head).as_str())
        .expect("a valid experiment")
        .cells()
}

/// The paths of a cell that lie at or below `root`.
fn paths_under(cell: &[ResolvedValue], root: &str) -> Vec<String> {
    cell.iter()
        // BORROW: .as_str() compares the path text
        .map(|value| value.path().as_str())
        .filter(|path| *path == root || path.starts_with(format!("{root}.").as_str()))
        .map(str::to_owned)
        .collect()
}

/// Guards: Experiment file — a table written at or below `prompt.values` reaches the cell as one object, whether the file nests it or quotes dotted keys, so a template reads `values.a.b` whole. Oracle: the cell written by hand from the TOML: `prompt.values` holds `{"a": 1, "b": {"c": "x"}}` and no leaf path of it exists.
#[test]
fn table_value_fixed_folds_into_one_object() {
    let want = json!({"a": 1, "b": {"c": "x"}});
    let nested = cells_of("[fixed.prompt.values]\na = 1\n[fixed.prompt.values.b]\nc = \"x\"\n");
    let quoted = cells_of("[fixed]\n\"prompt.values.a\" = 1\n\"prompt.values.b.c\" = \"x\"\n");
    let whole = cells_of("[fixed]\n\"prompt.values\" = { a = 1, b = { c = \"x\" } }\n");
    for cells in [&nested, &quoted, &whole] {
        assert_eq!(cells.len(), 1);
        // INDEX: the experiment has one cell
        let cell = &cells[0];
        assert_eq!(value_at(cell, "prompt.values"), Some(&want));
        assert_eq!(paths_under(cell, "prompt.values"), ["prompt.values"]);
    }
    assert_eq!(nested, quoted);
    assert_eq!(nested, whole);
}

/// Guards: Experiment file — a table at any path that is not declared table-valued still flattens into one value per leaf. Oracle: the cell written by hand: `plan.values.a` and `plan.values.b` are two values and `plan.values` is none.
#[test]
fn table_value_undeclared_paths_still_flatten() {
    let cells = cells_of("[fixed.plan.values]\na = 1\nb = 2\n[fixed.prompt]\nother = { c = 3 }\n");
    // INDEX: the experiment has one cell
    let cell = &cells[0];
    assert_eq!(value_at(cell, "plan.values.a"), Some(&json!(1)));
    assert_eq!(value_at(cell, "plan.values.b"), Some(&json!(2)));
    assert_eq!(value_at(cell, "plan.values"), None);
    assert_eq!(value_at(cell, "prompt.other.c"), Some(&json!(3)));
    assert_eq!(value_at(cell, "prompt.other"), None);
}

/// Guards: Experiment file — the levels of a factor at or below `prompt.values` fold per cell into one object, merged with the `[fixed]` part, in the order the levels are written. Oracle: the four cells written by hand, the first factor varying slowest.
#[test]
fn table_value_factor_levels_fold_per_cell() {
    let head = "[fixed.prompt.values]\nk = \"fixed\"\n\
                [[factors]]\npath = \"prompt.values.a\"\nlevels = [1, 2]\n\
                [[factors]]\npath = \"prompt.values.b\"\nlevels = [{ x = true }, { x = false }]\n";
    let cells = cells_of(head);
    let got: Vec<Option<&Value>> = cells
        .iter()
        .map(|cell| value_at(cell, "prompt.values"))
        .collect();
    let want = [
        json!({"k": "fixed", "a": 1, "b": {"x": true}}),
        json!({"k": "fixed", "a": 1, "b": {"x": false}}),
        json!({"k": "fixed", "a": 2, "b": {"x": true}}),
        json!({"k": "fixed", "a": 2, "b": {"x": false}}),
    ];
    assert_eq!(got, want.iter().map(Some).collect::<Vec<_>>());
    for cell in &cells {
        assert_eq!(paths_under(cell, "prompt.values"), ["prompt.values"]);
    }

    let at_path = cells_of(
        "[[factors]]\npath = \"prompt.values\"\nlevels = [{ a = 1 }, { c = { b = 2 } }]\n",
    );
    let got: Vec<Option<&Value>> = at_path
        .iter()
        .map(|cell| value_at(cell, "prompt.values"))
        .collect();
    assert_eq!(got, [Some(&json!({"a": 1})), Some(&json!({"c": {"b": 2}}))]);
}

/// Guards: Experiment file — an empty table at `prompt.values` is refused, and so is a table whose leaf a fixed value or a factor also gives, while disjoint leaves share one object. Oracle: the rule "a path that is another's prefix is the same field", worked by hand on the documents.
#[test]
fn table_value_empty_and_overlapping_tables_are_refused() {
    // BORROW: .as_str() lends the built document to the parser
    let read = |head: &str| ExperimentFile::from_toml_str(document(head).as_str());
    assert!(matches!(
        read("[fixed.prompt.values]\n"),
        Err(ExecutionError::EmptyTable { .. })
    ));
    assert!(matches!(
        read("[fixed]\n\"prompt.values\" = {}\n"),
        Err(ExecutionError::EmptyTable { .. })
    ));
    assert!(matches!(
        read("[fixed]\n\"prompt.values.a\" = 1\n[fixed.prompt.values]\na = 2\n"),
        Err(ExecutionError::Duplicate { .. })
    ));
    assert!(matches!(
        read(
            "[fixed.prompt.values]\na = 1\n[[factors]]\npath = \"prompt.values\"\nlevels = [{ a = { x = 1 } }]\n"
        ),
        Err(ExecutionError::Document { .. })
    ));
    assert!(matches!(
        read(
            "[fixed.prompt.values]\na = 1\n[[factors]]\npath = \"prompt.values.a\"\nlevels = [2]\n"
        ),
        Err(ExecutionError::Document { .. })
    ));
    for disjoint in [
        "[fixed.prompt.values]\na = 1\n[[factors]]\npath = \"prompt.values.b\"\nlevels = [2]\n",
        "[fixed.prompt.values]\na = 1\n[[factors]]\npath = \"prompt.values\"\nlevels = [{ b = 1 }]\n",
    ] {
        assert!(read(disjoint).is_ok(), "{disjoint}");
    }
    // Disjoint leaves share one object.
    let merged = cells_of(
        "[fixed.prompt.values]\na = 1\n[[factors]]\npath = \"prompt.values\"\nlevels = [{ b = 1 }]\n",
    );
    // INDEX: the experiment has one cell
    assert_eq!(
        value_at(&merged[0], "prompt.values"),
        Some(&json!({"a": 1, "b": 1}))
    );
}
