#![cfg(test)]
//! The suite file and the evaluator protocol: the 43 expectations of the golden corpus, the
//! graph rules of a suite and the refusal of a list where an object is written.

use candler_schemas::evaluation::{EvaluationError, PluginManifest, SuiteFile};

/// The translated suite, written by `tests/vectors/suite/translate_eval_json.py`.
const CONT_TRAIN: &str = include_str!("vectors/suite/cont_train.toml");

/// The 43 expectation ids of the golden corpus's `eval.json`, in the order of its 13 evals,
/// written out by hand.
const IDS: [&str; 43] = [
    "A1", "A3", "A5", "A2", "A6", "B1", "B2", "B3", "B7", "B8", "B9", "C1", "C2", "C3", "C4", "C5",
    "C6", "C7", "C8", "C9", "D1", "C-S2", "C-S3", "C-S4", "C-N1", "C-N2", "C-N3", "C-X1", "C-X2",
    "C-X3", "C-X4", "C-X5", "C-XN1", "C-H1", "C-H2", "C-M1", "C-M2", "D-E1", "D-E2", "D-E3",
    "D-E4", "D-E5", "D-E6",
];

/// The six expectations of evals 3 and 4, which are graded with the skill only.
const WITH_SKILL_ONLY: [&str; 6] = ["B1", "B2", "B3", "B7", "B8", "B9"];

/// Guards: Real-data acceptance — the translated golden-corpus suite is a valid suite of the 43
/// expectations of `eval.json`, and exactly the six expectations of evals 3 and 4 apply to the
/// `with_skill` configuration only. Oracle: the id list and the configurations of `eval.json`,
/// counted by hand.
#[test]
fn the_golden_corpus_suite_validates() {
    let suite = SuiteFile::from_toml_str(CONT_TRAIN).expect("the translated suite validates");
    let entries = suite.expectations();
    assert_eq!(entries.len(), 43);
    let ids: Vec<&str> = entries
        .iter()
        // BORROW: .as_str() reads the id as text
        .map(|entry| entry.id().as_str())
        .collect();
    assert_eq!(ids, IDS);
    for entry in entries {
        let conditions = entry
            .applies_to()
            .expect("every entry names configurations");
        assert_eq!(conditions.len(), 1, "{}", entry.id());
        // INDEX: the length was asserted to be one on the line above
        let condition = &conditions[0];
        // BORROW: .as_str() reads the path as text
        assert_eq!(condition.path().as_str(), "plan.config");
        let configurations: Vec<String> = condition
            .any_of()
            .iter()
            .map(|value| serde_json::to_string(value).expect("a value is written"))
            .collect();
        // BORROW: .as_str() reads the id as text
        let expected: &[&str] = if WITH_SKILL_ONLY.contains(&entry.id().as_str()) {
            &[r#""with_skill""#]
        } else {
            &[r#""with_skill""#, r#""without_skill""#]
        };
        assert_eq!(configurations, expected, "{}", entry.id());
    }
}

/// A suite entry written as TOML, with `depends_on` as given.
fn entry(id: &str, depends_on: &str) -> String {
    format!(
        "[[expectations]]\nid = \"{id}\"\nrole = \"gate\"\nkind = \"file_exists\"\n\
         depends_on = [{depends_on}]\n[expectations.evaluator]\nname = \"e\"\nrange = \"^1\"\n\
         [expectations.params]\n"
    )
}

/// A suite of entries, each a pair of an id and a `depends_on` list.
fn suite(entries: &[(&str, &str)]) -> String {
    let mut text = String::from("schema = \"suite/1\"\n");
    for (id, depends_on) in entries {
        text.push_str(&entry(id, depends_on));
    }
    text
}

/// The provenance record of the golden corpus's mini-corpus copy.
const PROVENANCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../candler-core/tests/fixtures/mini-corpus/PROVENANCE.json"
));

/// Guards: Real-data acceptance — the suite records the `eval.json` it was translated from, and
/// the recorded sha256 is the one the golden corpus pins for the suite its regrade ran. Oracle:
/// `PROVENANCE.json` of the mini-corpus, `eval_infra."eval.json".sha256`.
#[test]
fn the_suite_source_is_the_pinned_eval_json() {
    let suite: toml::Table = toml::from_str(CONT_TRAIN).expect("the translated suite is TOML");
    // INDEX: the translation script writes a `source` table with a `sha256`
    // BORROW: .as_str() reads the text of a TOML value
    let recorded = suite["source"]["sha256"]
        .as_str()
        .expect("a recorded sha256");
    let provenance: serde_json::Value =
        serde_json::from_str(PROVENANCE).expect("PROVENANCE.json is JSON");
    // INDEX: PROVENANCE.json pins `eval_infra` with an `eval.json` entry that has a `sha256`
    // BORROW: .as_str() reads the text of a JSON value
    let pinned = provenance["eval_infra"]["eval.json"]["sha256"]
        .as_str()
        .expect("a pinned sha256");
    assert_eq!(recorded, pinned);
}

/// Guards: Suite validity — a suite with a repeated id, a dependency on an id no entry has or a
/// dependency cycle is refused with the ids named, a valid graph is accepted, and a suite, an
/// entry or a manifest written as a list is refused. Oracle: four graphs and four list forms
/// written by hand.
#[test]
fn a_suite_graph_is_checked() {
    let valid = suite(&[("a", ""), ("b", "\"a\""), ("c", "\"a\", \"b\"")]);
    let parsed = SuiteFile::from_toml_str(&valid).expect("the valid graph is accepted");
    assert_eq!(parsed.expectations().len(), 3);

    let refusals: [(&str, String, &[&str]); 4] = [
        (
            "a repeated id",
            suite(&[("a", ""), ("b", ""), ("a", "")]),
            &["a"],
        ),
        (
            "an unknown dependency",
            suite(&[("a", ""), ("b", "\"ghost\"")]),
            &["b", "ghost"],
        ),
        (
            "a two-entry cycle",
            suite(&[("a", "\"b\""), ("b", "\"a\""), ("c", "")]),
            &["a", "b"],
        ),
        (
            "a self-dependency",
            suite(&[("a", ""), ("b", "\"b\"")]),
            &["b"],
        ),
    ];
    for (name, text, named) in &refusals {
        let error = SuiteFile::from_toml_str(text).expect_err(name);
        let message = error.to_string();
        for id in *named {
            assert!(message.contains(id), "{name}: {message}");
        }
    }
    // INDEX: `refusals` holds four rows, so indices 0 to 2 exist
    assert!(matches!(
        SuiteFile::from_toml_str(&refusals[0].1),
        Err(EvaluationError::DuplicateId { .. })
    ));
    // INDEX: `refusals` holds four rows, so indices 0 to 2 exist
    assert!(matches!(
        SuiteFile::from_toml_str(&refusals[1].1),
        Err(EvaluationError::UnknownDependency { .. })
    ));
    // INDEX: `refusals` holds four rows, so indices 0 to 2 exist
    assert!(matches!(
        SuiteFile::from_toml_str(&refusals[2].1),
        Err(EvaluationError::Cycle { .. })
    ));

    let entry_json = r#"{"id":"a","role":"gate","evaluator":{"name":"e","range":"^1"},"kind":"k","params":{},"depends_on":[]}"#;
    let manifest_json = r#"{"schema":"plugin-manifest/1","name":"p","version":"1.0.0","protocol":{"major":1,"minor":0},"command":["run"],"environment":{"kind":"none"}}"#;
    let base_suite = format!(r#"{{"schema":"suite/1","expectations":[{entry_json}]}}"#);
    serde_json::from_str::<SuiteFile>(&base_suite).expect("the object form is accepted");
    serde_json::from_str::<PluginManifest>(manifest_json).expect("the object form is accepted");
    let lists = [
        (
            "a suite",
            base_suite.replace(r#"{"schema":"suite/1","expectations":"#, r#"["suite/1","#),
        ),
        (
            "a manifest",
            // BORROW: .to_owned() makes the owned text the row keeps
            r#"["plugin-manifest/1","p","1.0.0"]"#.to_owned(),
        ),
    ];
    // INDEX: `lists` holds two rows, so indices 0 and 1 exist
    // BORROW: .as_str() views the owned text as the &str the reader takes
    assert!(
        serde_json::from_str::<SuiteFile>(lists[0].1.as_str()).is_err(),
        "{}",
        lists[0].0
    );
    // INDEX: `lists` holds two rows, so indices 0 and 1 exist
    // BORROW: .as_str() views the owned text as the &str the reader takes
    assert!(
        serde_json::from_str::<PluginManifest>(lists[1].1.as_str()).is_err(),
        "{}",
        lists[1].0
    );
    let list_entry = r#"["a","gate"]"#;
    assert!(
        serde_json::from_str::<SuiteFile>(&format!(
            r#"{{"schema":"suite/1","expectations":[{list_entry}]}}"#
        ))
        .is_err(),
        "an entry written as a list"
    );
}
