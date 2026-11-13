#![cfg(test)]
//! The suite file and the evaluator protocol: the 43 expectations of the golden corpus, the
//! graph rules of a suite and the refusal of a list where an object is written.

use candler_schemas::evaluation::{
    CancelParams, Condition, ContextArtifact, ContextRaw, ContextRef, DescribeParams,
    DescribeResult, EvaluateParams, EvaluateResult, EvaluationError, EvaluatorEnvironment,
    EvaluatorRef, Item, KindSpec, LogParams, Needs, Notification, OutcomeParams, PluginEnvironment,
    PluginManifest, ProgressParams, ProtocolVersion, Request, RequestCancelled, RequestSide,
    RunContext, SuiteFile,
};
use candler_schemas::outcome::{Instrument, OutcomeRecord};
use serde::de::DeserializeOwned;

/// The translated suite, written by `tests/vectors/suite/translate_eval_json.py`.
const CONT_TRAIN: &str = include_str!("../vectors/suite/cont_train.toml");

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

/// A `not_measured` outcome record written by hand, with the cause object `cause`.
fn record(cause: &str) -> String {
    format!(
        r#"{{"run":{{"cell":"sha256:b28a6587f9984281103bf4150867bbca481889e3046c60ec7c27f2b31d562a22","replicate":1,"attempt":1}},"expectation_id":"D-E2","role":"descriptive","evaluator":"synthetic-eval@1.0.0","instruments":[],"suite_entry_digest":"sha256:015b60579b1ff8dac24c2e453898bee9888fff7f0c7eba8345db7a87227f5764","context_digest":"sha256:b28b531af1ae51ed826ac1ab4587bc7e2680d8652cf476f0fd7ef86566378fc1","request_id":"01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d","status":"not_measured","evidence":[],"cause":{cause}}}"#
    )
}

/// Reads `valid` as a `T` and refuses `list`, the same members written as a JSON list.
fn object_not_list<T: DeserializeOwned>(name: &str, valid: &str, list: &str) {
    serde_json::from_str::<T>(valid).unwrap_or_else(|e| panic!("{name}: the object form: {e}"));
    assert!(
        serde_json::from_str::<T>(list).is_err(),
        "{name}: the list form is refused"
    );
}

/// Guards: Cond 7 — an `outcome` notification whose record carries the legacy cause is refused,
/// by the reader and by the builder, while the same record with a `reason` cause is accepted and
/// the outcome record itself still reads the legacy cause; every message type of the protocol
/// refuses a list where an object is written. Oracle: a record written by hand, with the
/// `not_measured` status that admits both causes.
#[test]
fn an_outcome_notification_refuses_the_legacy_cause() {
    let legacy = record(r#"{"legacy":"no_replay_target"}"#);
    let reason = record(r#"{"reason":"instrument_error"}"#);
    let wrap = |record: &str| format!(r#"{{"record":{record}}}"#);

    serde_json::from_str::<OutcomeRecord>(&legacy).expect("the record keeps the legacy cause");
    let accepted = serde_json::from_str::<OutcomeParams>(&wrap(&reason)).expect("a reason cause");
    // BORROW: .as_str() reads the id as text
    assert_eq!(accepted.record().expectation_id().as_str(), "D-E2");

    let error = serde_json::from_str::<OutcomeParams>(&wrap(&legacy)).expect_err("legacy");
    assert!(error.to_string().contains("D-E2"), "{error}");
    let record = serde_json::from_str::<OutcomeRecord>(&legacy).expect("a record");
    let built = OutcomeParams::builder().record(record).build();
    // BORROW: .as_str() reads the id as text
    assert!(
        matches!(built, Err(EvaluationError::LegacyCause { ref expectation_id }) if expectation_id.as_str() == "D-E2"),
        "{built:?}"
    );

    let digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    let run = r#"{"cell":"sha256:b28a6587f9984281103bf4150867bbca481889e3046c60ec7c27f2b31d562a22","replicate":1,"attempt":1}"#;
    let context = format!(
        r#"{{"run":{run},"root":"/r","raw":[],"artifacts":[],"snapshots":[],"inputs":[],"metadata":{{}}}}"#
    );
    let context_list = format!(r#"[{run},"/r","/t",[],[],[],[],{{}}]"#);
    let needs = r#"{"trace":false,"artifacts":[],"snapshots":[],"inputs":[],"metadata":[]}"#;
    let needs_list = r"[false,[],[],[],[]]";
    let id = "01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d";
    let describe = r#"{"protocol":{"major":1,"minor":0},"name":"e","version":"1.0.0","instruments":[],"environment":{"packages":{}},"kind":"deterministic","kinds":[]}"#;
    object_not_list::<OutcomeParams>("OutcomeParams", &wrap(&reason), &format!("[{reason}]"));
    object_not_list::<CancelParams>(
        "CancelParams",
        &format!(r#"{{"request_id":"{id}"}}"#),
        &format!(r#"["{id}"]"#),
    );
    object_not_list::<ProgressParams>("ProgressParams", r#"{"done":1}"#, "[1]");
    object_not_list::<LogParams>("LogParams", r#"{"line":"x"}"#, "[1]");
    object_not_list::<EvaluateResult>("EvaluateResult", r#"{"reported":3}"#, "[3]");
    object_not_list::<RunContext>("RunContext", &context, &context_list);
    object_not_list::<EvaluateParams>(
        "EvaluateParams",
        &format!(r#"{{"context":{context},"items":[],"prior":[],"scratch":"/s"}}"#),
        &format!(r#"[{context},[],[],"/s"]"#),
    );
    object_not_list::<Item>(
        "Item",
        r#"{"expectation_id":"A1","kind":"k","params":{},"deadline_ms":5}"#,
        r#"["A1","k",{},5]"#,
    );
    object_not_list::<ContextRaw>(
        "ContextRaw",
        r#"{"stream":"stdout","path":"/p"}"#,
        r#"["stdout","/p"]"#,
    );
    object_not_list::<ContextArtifact>(
        "ContextArtifact",
        &format!(r#"{{"path":"a.txt","abs":"/a","sha256":"{digest}"}}"#),
        &format!(r#"["a.txt","/a","{digest}"]"#),
    );
    object_not_list::<ContextRef>(
        "ContextRef",
        &format!(r#"{{"kind":"k","path":"/p","digest":"{digest}"}}"#),
        &format!(r#"["k","/p","{digest}"]"#),
    );
    object_not_list::<Needs>("Needs", needs, needs_list);
    object_not_list::<KindSpec>(
        "KindSpec",
        &format!(r#"{{"kind":"k","params_schema":{{}},"needs":{needs},"default_deadline_ms":5}}"#),
        &format!(r#"["k",{{}},{needs},5]"#),
    );
    object_not_list::<EvaluatorEnvironment>(
        "EvaluatorEnvironment",
        r#"{"packages":{}}"#,
        &format!(r#"["{digest}",{{}}]"#),
    );
    object_not_list::<DescribeResult>(
        "DescribeResult",
        describe,
        r#"[{"major":1,"minor":0},"e","1.0.0",[],{"packages":{}},"deterministic",[]]"#,
    );
    object_not_list::<ProtocolVersion>("ProtocolVersion", r#"{"major":1,"minor":0}"#, "[1,0]");
    object_not_list::<PluginEnvironment>("PluginEnvironment", r#"{"kind":"none"}"#, r#"["none"]"#);
    object_not_list::<EvaluatorRef>(
        "EvaluatorRef",
        r#"{"name":"e","range":"^1"}"#,
        r#"["e","^1"]"#,
    );
    object_not_list::<Condition>(
        "Condition",
        r#"{"path":"a.b","any_of":[1]}"#,
        r#"["a.b",[1]]"#,
    );
    object_not_list::<Instrument>(
        "Instrument",
        &format!(r#"{{"name":"x","version":"1.0.0","digest":"{digest}","dirty":false}}"#),
        &format!(r#"["x","1.0.0","{digest}",false]"#),
    );
    let cancel = format!(r#"{{"request_id":"{id}"}}"#);
    object_not_list::<Notification>(
        "Notification",
        &format!(r#"{{"method":"cancel","params":{cancel}}}"#),
        &format!(r#"["cancel",{cancel}]"#),
    );
    object_not_list::<Notification>(
        "Notification cancel with params as a list",
        &format!(r#"{{"method":"cancel","params":{cancel}}}"#),
        &format!(r#"{{"method":"cancel","params":["{id}"]}}"#),
    );
    object_not_list::<Notification>(
        "Notification progress with params as a list",
        r#"{"method":"progress","params":{"done":1}}"#,
        r#"{"method":"progress","params":[1]}"#,
    );
    object_not_list::<Notification>(
        "Notification log with params as a list",
        r#"{"method":"log","params":{"line":"x"}}"#,
        r#"{"method":"log","params":[1]}"#,
    );
    object_not_list::<Notification>(
        "Notification outcome with params as a list",
        &format!(r#"{{"method":"outcome","params":{}}}"#, wrap(&reason)),
        &format!(r#"{{"method":"outcome","params":[{reason}]}}"#),
    );

    let params = format!(r#"{{"context":{context},"items":[],"prior":[],"scratch":"/s"}}"#);
    object_not_list::<DescribeParams>("DescribeParams", "{}", "[]");
    object_not_list::<Request>(
        "Request describe",
        r#"{"method":"describe","params":{}}"#,
        r#"["describe",{}]"#,
    );
    object_not_list::<Request>(
        "Request describe with params as a list",
        r#"{"method":"describe","params":{}}"#,
        r#"{"method":"describe","params":[]}"#,
    );
    object_not_list::<Request>(
        "Request evaluate with params as a list",
        &format!(r#"{{"method":"evaluate","params":{params}}}"#),
        &format!(r#"{{"method":"evaluate","params":[{context},[],[],"/s"]}}"#),
    );
    object_not_list::<RequestCancelled>(
        "RequestCancelled",
        r#"{"code":-32800,"message":"cancelled"}"#,
        r#"[-32800,"cancelled"]"#,
    );
    // a request, a result and a cancelled error each read through `RequestSide`
    let sides = [
        r#"{"method":"describe","params":{}}"#,
        r#"{"reported":3}"#,
        r#"{"code":-32800,"message":"cancelled"}"#,
    ];
    for text in sides {
        serde_json::from_str::<RequestSide>(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    }
    assert!(
        serde_json::from_str::<RequestCancelled>(r#"{"code":-32801,"message":"x"}"#).is_err(),
        "another error code is refused"
    );
}
