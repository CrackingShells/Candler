//! The import rules of the mini-corpus: the harness, treatment, snapshot, marker and instrument names that the core takes from its caller.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::str::FromStr as _;

use candler_core::import::ImportError;
use candler_core::import::rules::{
    HarnessRule, ImportRules, ImportRulesBuilder, MarkerCode, MarkerRule, SnapshotRule,
    TreatmentRule,
};
use candler_schemas::primitives::{FieldPath, Name, NameToken, ReasonCode};
use regex::Regex;

/// The rules that read the mini-corpus, and the golden corpus it was cut from.
///
/// # Panics
///
/// Panics if a literal below is not the primitive it names.
#[must_use]
pub fn rules() -> ImportRules {
    try_rules("resolved_skill_version")
        .unwrap_or_else(|e| panic!("the rules of the mini-corpus are valid: {e}"))
}

/// The rules of the mini-corpus with `resolved` as the resolved-version key.
///
/// # Errors
///
/// Returns the builder's error when the rules do not hold, such as a dotted `resolved`.
///
/// # Panics
///
/// Panics if a literal below is not the primitive it names.
pub fn try_rules(resolved: &str) -> Result<ImportRules, ImportError> {
    builder(resolved)
        .entry_params(entry_params(&["scale", "text", "tier"]))
        .build()
}

/// The names `keys`, as the rules' entry params.
///
/// # Panics
///
/// Panics if a key is not a name.
#[must_use]
pub fn entry_params(keys: &[&str]) -> Vec<Name> {
    keys.iter()
        .map(|key| Name::from_str(key).unwrap_or_else(|e| panic!("{key}: {e}")))
        .collect()
}

/// The rules of the mini-corpus but for the entry params, which the caller sets.
///
/// # Panics
///
/// Panics if a literal below is not the primitive it names.
#[must_use]
pub fn builder(resolved: &str) -> ImportRulesBuilder {
    let name = |text: &str| Name::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let pattern = |text: &str| Regex::new(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    ImportRules::builder()
        .format(NameToken::from_str("htn-results@1").unwrap_or_else(|e| panic!("{e}")))
        .harness(HarnessRule::new(
            name("claude-code"),
            "/claude_code_version".into(),
        ))
        .treatment(TreatmentRule::new(
            "/plan/skill_version".into(),
            FieldPath::from_str(resolved).unwrap_or_else(|e| panic!("{e}")),
        ))
        .instrument(name("htn-creator-eval"))
        .snapshot(SnapshotRule::new(name("catalog"), "/catalog/sha256".into()))
        .marker(MarkerRule::new(
            pattern(r"TIER_D_REPLAY:[^\n]*reason=(?P<code>[A-Za-z0-9_]+)"),
            MarkerCode::Capture("code".into()),
        ))
        .marker(MarkerRule::new(
            pattern(r"declares zero 'mappings' entries"),
            MarkerCode::Fixed(
                ReasonCode::from_str("zero_mappings").unwrap_or_else(|e| panic!("{e}")),
            ),
        ))
        .suite_kind(name("legacy-check"))
        .metadata(FieldPath::from_str("plan.model").unwrap_or_else(|e| panic!("{e}")))
}
