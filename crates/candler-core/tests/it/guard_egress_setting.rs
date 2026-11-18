#![cfg(test)]
//! Guards for the egress allowlist `isolation.egress`: the parse table, the refusals at resolve
//! and per cell, and the cell it makes.
//!
//! The expected values are the hostname rule of RFC 1123 §2.1 worked by hand, literals, and the
//! digest of a cell with no egress value, pinned as a literal.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::session::cells_of;
use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::{FileValues, Resolution, ResolveInput};
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::egress::{EgressRefusal, EgressStep, settle};
use candler_core::runner::plan::{Force, PlanCtx, PlanError, PlanMode, plan_runs};
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_schemas::execution::ExperimentFile;
use candler_schemas::execution::paths;
use candler_schemas::primitives::FieldPath;
use candler_schemas::refusal::Refusal;
use candler_schemas::values::{JsonValue, ResolvedValue, ValueSource};
use serde_json::{Value, json};

use crate::support;

fn fp(text: &str) -> FieldPath {
    text.parse().expect("a field path")
}

fn js(value: Value) -> JsonValue {
    JsonValue::new(value).expect("a JSON value")
}

/// The text of a label of `n` copies of `c`.
fn label(c: char, n: usize) -> String {
    std::iter::repeat_n(c, n).collect()
}

/// What a `settle` row expects: the canonical entries, or the code and a text the detail names.
type Expect = Result<Vec<String>, (&'static str, String)>;

/// Guards: R06 D3 — `isolation.egress` is a list of `host:port` entries that resolves to its
/// canonical form (lowercase, sorted, no duplicates) or is refused naming the entry: IP literals,
/// wildcards, schemes, paths, port 0, a port above 65535, a missing port and a name that is no
/// RFC 1123 hostname with a dot are `isolation.egress_invalid`; any entry with
/// `isolation.network = bridge` is `isolation.egress_with_bridge`.
///
/// An empty list is no allowlist and settles to absent, with or without `bridge`.
///
/// Oracle: RFC 1123 §2.1, worked by hand (letters, digits and hyphens; no leading or trailing
/// hyphen; labels of 1 to 63 characters; at most 253 characters; at least one dot), and the sort
/// order of the `host:port` text.
///
/// Bite: an IP literal accepted (a parser that checks characters only); a missing port defaulted
/// to 443.
#[test]
fn egress_parse_table() {
    let ok = |want: &[&str]| -> Expect { Ok(want.iter().map(|text| (*text).to_owned()).collect()) };
    let invalid = |entry: &str| Err(("isolation.egress_invalid", entry.to_owned()));
    let long_host = format!(
        "{}.{}.{}.{}.com:443",
        label('a', 63),
        label('b', 63),
        label('c', 63),
        label('d', 57)
    );
    let too_long = format!(
        "{}.{}.{}.{}.com:443",
        label('a', 63),
        label('b', 63),
        label('c', 63),
        label('d', 58)
    );
    let rows: Vec<(&str, Value, Option<Value>, Expect)> = vec![
        (
            "one entry",
            json!(["pypi.org:443"]),
            None,
            ok(&["pypi.org:443"]),
        ),
        (
            "mixed case is lowercased",
            json!(["PyPI.Org:443"]),
            None,
            ok(&["pypi.org:443"]),
        ),
        (
            "duplicates go, the order is sorted",
            json!([
                "files.pythonhosted.org:443",
                "pypi.org:443",
                "PYPI.ORG:443",
                "files.pythonhosted.org:443"
            ]),
            None,
            ok(&["files.pythonhosted.org:443", "pypi.org:443"]),
        ),
        (
            "one host on two ports stays two entries, sorted by text",
            json!(["a.example.com:8080", "a.example.com:443"]),
            None,
            ok(&["a.example.com:443", "a.example.com:8080"]),
        ),
        (
            "a leading zero of a port goes",
            json!(["a.example.com:0443"]),
            None,
            ok(&["a.example.com:443"]),
        ),
        (
            "the largest port",
            json!(["a.example.com:65535"]),
            None,
            ok(&["a.example.com:65535"]),
        ),
        (
            "digits and hyphens inside labels",
            json!(["a-1.b2-c.io:1"]),
            None,
            ok(&["a-1.b2-c.io:1"]),
        ),
        (
            "a 253 character name",
            json!([long_host.clone()]),
            None,
            ok(&[long_host.as_str()]),
        ),
        ("an empty list is no allowlist", json!([]), None, ok(&[])),
        (
            "the internal network allows a list",
            json!(["a.example.com:443"]),
            Some(json!("internal")),
            ok(&["a.example.com:443"]),
        ),
        (
            "a bridge with no entry",
            json!([]),
            Some(json!("bridge")),
            ok(&[]),
        ),
        (
            "IPv4 literal",
            json!(["93.184.216.34:443"]),
            None,
            invalid("93.184.216.34:443"),
        ),
        (
            "IPv6 literal",
            json!(["2001:db8::1:443"]),
            None,
            invalid("2001:db8::1:443"),
        ),
        (
            "bracketed IPv6 literal",
            json!(["[2001:db8::1]:443"]),
            None,
            invalid("[2001:db8::1]:443"),
        ),
        (
            "wildcard",
            json!(["*.example.com:443"]),
            None,
            invalid("*.example.com:443"),
        ),
        (
            "scheme",
            json!(["https://example.com:443"]),
            None,
            invalid("https://example.com:443"),
        ),
        (
            "path",
            json!(["example.com:443/index"]),
            None,
            invalid("example.com:443/index"),
        ),
        (
            "port 0",
            json!(["example.com:0"]),
            None,
            invalid("example.com:0"),
        ),
        (
            "port above 65535",
            json!(["example.com:65536"]),
            None,
            invalid("example.com:65536"),
        ),
        (
            "port with a sign",
            json!(["example.com:+443"]),
            None,
            invalid("example.com:+443"),
        ),
        (
            "missing port",
            json!(["example.com"]),
            None,
            invalid("example.com"),
        ),
        (
            "empty port",
            json!(["example.com:"]),
            None,
            invalid("example.com:"),
        ),
        (
            "no dot",
            json!(["localhost:80"]),
            None,
            invalid("localhost:80"),
        ),
        (
            "leading hyphen",
            json!(["-a.example.com:443"]),
            None,
            invalid("-a.example.com:443"),
        ),
        (
            "trailing hyphen",
            json!(["a-.example.com:443"]),
            None,
            invalid("a-.example.com:443"),
        ),
        (
            "empty label",
            json!(["a..example.com:443"]),
            None,
            invalid("a..example.com:443"),
        ),
        (
            "trailing dot",
            json!(["example.com.:443"]),
            None,
            invalid("example.com.:443"),
        ),
        (
            "underscore",
            json!(["a_b.example.com:443"]),
            None,
            invalid("a_b.example.com:443"),
        ),
        (
            "a label of 64 characters",
            json!([format!("{}.com:443", label('a', 64))]),
            None,
            invalid(&label('a', 64)),
        ),
        (
            "a 254 character name",
            json!([too_long.clone()]),
            None,
            invalid(&too_long),
        ),
        (
            "an all-digit last label",
            json!(["a.123:443"]),
            None,
            invalid("a.123:443"),
        ),
        (
            "an entry that is no string",
            json!([443]),
            None,
            invalid("443"),
        ),
        (
            "a value that is no list",
            json!("example.com:443"),
            None,
            invalid("example.com:443"),
        ),
        (
            "the bad entry is named among good ones",
            json!(["pypi.org:443", "example.com"]),
            None,
            invalid("`example.com`"),
        ),
        (
            "an entry with bridge",
            json!(["pypi.org:443"]),
            Some(json!("bridge")),
            Err(("isolation.egress_with_bridge", "pypi.org:443".to_owned())),
        ),
    ];
    for (name, egress, network, expect) in rows {
        let got = settle(Some(&egress), network.as_ref());
        match (got, expect) {
            (Ok(None), Ok(want)) => assert!(want.is_empty(), "{name}: settled to absent"),
            (Ok(Some(entries)), Ok(want)) => {
                assert!(!want.is_empty(), "{name}: an empty list settles to absent");
                let text: Vec<String> = entries.iter().map(ToString::to_string).collect();
                assert_eq!(text, want, "{name}");
            }
            (Err(refusal), Err((code, names))) => {
                assert_eq!(Refusal::code(&refusal).as_str(), code, "{name}");
                assert!(
                    refusal.detail().contains(&names),
                    "{name}: {}",
                    refusal.detail()
                );
            }
            (got, want) => panic!("{name}: got {got:?}, wanted {want:?}"),
        }
    }
    assert_eq!(
        settle(None, Some(&json!("bridge"))),
        Ok(None),
        "absent stays absent"
    );
}

async fn resolve(fixed: &[(&str, Value)]) -> Result<Resolution, ResolveError> {
    let cwd = Path::new("/work");
    let config = Arc::new(Config::new(
        cwd.to_owned(),
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    ));
    let values = fixed
        .iter()
        .map(|(path, value)| (fp(path), js(value.clone())))
        .collect();
    Resolver::new(config)
        .register(Box::new(EgressStep::new().expect("a step")))
        .expect("registers")
        .resolve(ResolveInput::default().file(FileValues::new(values)))
        .await
}

/// Guards: R06 D3 — a wrong allowlist is refused when the experiment resolves, before any job
/// starts, with the code of the refusal and the entry in the message; a well-formed list
/// resolves, held as written (canonical form is the cell's, at planning); the step derives
/// nothing, so an empty or absent value resolves as written.
/// Oracle: the codes of D3, and the written value read back unchanged.
/// Bite: the step left unregistered or its refusal routed as `resolve.preflight_failed`; a
/// refusal of every list re-inserted (the well-formed list is refused).
#[tokio::test]
async fn egress_checked_at_resolve() {
    match resolve(&[(
        paths::ISOLATION_EGRESS,
        json!(["pypi.org:443", "10.0.0.1:80"]),
    )])
    .await
    {
        Err(ResolveError::Egress(refusal @ EgressRefusal::Invalid { .. })) => {
            assert_eq!(Refusal::code(&refusal).as_str(), "isolation.egress_invalid");
            assert!(
                refusal.detail().contains("10.0.0.1:80"),
                "{}",
                refusal.detail()
            );
        }
        other => panic!("an IP literal: {:?}", other.map(|_| ())),
    }
    match resolve(&[
        (paths::ISOLATION_EGRESS, json!(["pypi.org:443"])),
        (paths::ISOLATION_NETWORK, json!("bridge")),
    ])
    .await
    {
        Err(ResolveError::Egress(refusal @ EgressRefusal::WithBridge { .. })) => {
            assert_eq!(
                Refusal::code(&refusal).as_str(),
                "isolation.egress_with_bridge"
            );
            assert!(
                refusal.detail().contains("pypi.org:443"),
                "{}",
                refusal.detail()
            );
        }
        other => panic!("bridge: {:?}", other.map(|_| ())),
    }
    let written = json!(["PyPI.org:443"]);
    let resolution = resolve(&[(paths::ISOLATION_EGRESS, written.clone())])
        .await
        .expect("a well-formed list resolves");
    let held = resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == paths::ISOLATION_EGRESS)
        .and_then(ResolvedValue::value)
        .map(|value| value.as_value().clone());
    assert_eq!(
        held,
        Some(written),
        "the step settles it and derives nothing"
    );
    let written = json!([]);
    let resolution = resolve(&[(paths::ISOLATION_EGRESS, written.clone())])
        .await
        .expect("an empty list resolves");
    let held = resolution
        .values
        .iter()
        .find(|value| value.path().as_str() == paths::ISOLATION_EGRESS)
        .and_then(ResolvedValue::value)
        .map(|value| value.as_value().clone());
    assert_eq!(held, Some(written), "the step derives nothing");
    assert!(
        resolution
            .sources
            .iter()
            .all(|entry| !matches!(entry.source(), ValueSource::Derived { .. })),
        "no derived source"
    );
    assert!(resolve(&[]).await.is_ok(), "an absent value resolves");
}

fn experiment_with(factors: &str) -> ExperimentFile {
    let text = format!(
        "schema = \"experiment/1\"\nreplicates = 1\n\n{factors}\n[budget.per_run]\nwall_s = 60\n\n\
         [harness]\nname = \"claude-code\"\n\n[treatment]\nname = \"t\"\n"
    );
    ExperimentFile::from_toml_str(&text).expect("an experiment")
}

/// Plans `file` over `fixed` and returns, per cell, its digest and values.
async fn planned(
    file: &ExperimentFile,
    fixed: &[(&str, Value)],
) -> Result<Vec<(String, Vec<(String, Value)>)>, PlanError> {
    let mut all = vec![
        (
            "harness",
            json!({"name": "claude-code", "version": "2.1.311"}),
        ),
        ("treatment", json!({"name": "t"})),
    ];
    all.extend(fixed.iter().map(|(path, value)| (*path, value.clone())));
    let mut resolution = resolve(&all).await.expect("the fixed values resolve");
    resolution.settings.push(ResolvedValue::recorded(
        fp(paths::REQUEST_TREATMENT),
        js(json!({"name": "t"})),
    ));
    let (dir, store, lock) = support::run_plan::fresh_store();
    let ctx = PlanCtx {
        store,
        experiment_lock: lock,
        secrets_present: BTreeSet::new(),
        driver: Some(Arc::new(ClaudeCodeDriver::new())),
    };
    let outcome = plan_runs(&ctx, file, &resolution, Force::No, PlanMode::Check)?;
    let spec = JobSpec {
        experiment: file.clone(),
        resolution,
        levels: Vec::new(),
        plans: outcome.plans.clone(),
        budget: file.budget().clone(),
        force: false,
        inputs: Vec::new(),
        evaluation_inputs: Vec::new(),
    };
    let cells = cells_of(&spec, &outcome, &BTreeSet::new(), &BTreeMap::new()).expect("cells");
    drop(dir);
    Ok(cells
        .iter()
        .map(|(digest, cell)| {
            let values = cell
                .comparable
                .values()
                .iter()
                .filter_map(|value| {
                    Some((value.path().to_string(), value.value()?.as_value().clone()))
                })
                .collect();
            (digest.to_string(), values)
        })
        .collect())
}

fn egress_of(values: &[(String, Value)]) -> Option<Value> {
    values
        .iter()
        .find(|(path, _)| path == paths::ISOLATION_EGRESS)
        .map(|(_, value)| value.clone())
}

/// The digest of the cell whose egress is `["a.org:443", "pypi.org:443"]`.
const LISTED_DIGEST: &str =
    "sha256:9a1cd9a1ee0c360c0dde88f8b50418136d40a56c301bd0a6a39110787ec5a33f";

/// The digest of the cell of the plain experiment (no egress value).
const BASE_DIGEST: &str = "sha256:ea0e539ae84ca8476df9f8f73b3dbbb90ef2c7f173a4f636fb5e752b785254cf";

/// Guards: R06 D3 — a cell with no egress value keeps its digest, and an empty list is an absent
/// value (no allowlist, no proxy), so it digests as one.
/// Oracle: the digest e7aa7bb computes for this cell, pinned as a literal.
/// Bite: the empty list kept as `Some([])` (it digests apart from absent); a value written into
/// every cell (the plain digest moves).
#[tokio::test]
async fn egress_unset_leaves_the_cell() {
    let file = experiment_with("");
    let plain = planned(&file, &[]).await.expect("plans");
    assert_eq!(plain.len(), 1);
    assert_eq!(plain[0].0, BASE_DIGEST, "an absent value moves no digest");
    assert_eq!(egress_of(&plain[0].1), None);

    let empty = planned(&file, &[(paths::ISOLATION_EGRESS, json!([]))])
        .await
        .expect("plans");
    assert_eq!(
        empty, plain,
        "an empty list is an absent value, digest and values"
    );
}

/// Guards: R06 D3 — a cell whose effective values pair an allowlist with `bridge` is refused when
/// the cells are planned, with `isolation.egress_with_bridge`, even when only a factor level sets
/// the list; a factor level with a bad entry is `isolation.egress_invalid`; a well-formed list a
/// factor level sets is accepted and parses, canonicalises and digests as a cell value.
/// Oracle: the codes of D3; the canonical lists written by hand; the digest of the canonical list
/// pinned as a literal; the fixed `bridge` resolves alone because no list is written there.
/// Bite: the per-cell call dropped from the plan (the cell plans with both); entries left unsorted
/// (the unsorted and the sorted spelling digest apart).
#[tokio::test]
async fn egress_checked_per_cell() {
    let factor = |level: &str| {
        experiment_with(&format!(
            "[[factors]]\npath = \"isolation.egress\"\nlevels = [{level}]\n"
        ))
    };
    for (name, level, code) in [
        (
            "a list with bridge",
            "[\"pypi.org:443\"]",
            "isolation.egress_with_bridge",
        ),
        ("a bad entry", "[\"pypi.org\"]", "isolation.egress_invalid"),
    ] {
        match planned(
            &factor(level),
            &[(paths::ISOLATION_NETWORK, json!("bridge"))],
        )
        .await
        {
            Err(PlanError::Egress(refusal)) => {
                assert_eq!(Refusal::code(&refusal).as_str(), code, "{name}");
            }
            other => panic!("{name}: {:?}", other.map(|cells| cells.len())),
        }
    }
    assert!(
        planned(
            &factor("[]"),
            &[(paths::ISOLATION_NETWORK, json!("bridge"))]
        )
        .await
        .is_ok(),
        "an empty list with bridge plans"
    );
    // A well-formed list a factor level sets plans, as the canonical list: lowercase, sorted, no
    // repeats. Unsorted or repeated entries must not change the digest.
    let unsorted = planned(
        &factor("[\"PyPI.org:443\", \"a.org:443\", \"pypi.org:443\"]"),
        &[],
    )
    .await
    .expect("a well-formed list plans");
    assert_eq!(unsorted.len(), 1);
    assert_eq!(
        egress_of(&unsorted[0].1),
        Some(json!(["a.org:443", "pypi.org:443"])),
        "the cell holds the canonical list"
    );
    let sorted = planned(&factor("[\"a.org:443\", \"pypi.org:443\"]"), &[])
        .await
        .expect("plans");
    assert_eq!(unsorted[0].0, sorted[0].0, "the same list is the same cell");
    assert_eq!(sorted[0].0, LISTED_DIGEST);
    assert_ne!(LISTED_DIGEST, BASE_DIGEST, "a list moves the digest");
    let other = planned(&factor("[\"a.org:443\", \"b.org:443\"]"), &[])
        .await
        .expect("plans");
    assert_ne!(other[0].0, sorted[0].0, "another list is another cell");
}
