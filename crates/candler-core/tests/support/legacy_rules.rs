//! The import rules of the mini-corpus: the harness, treatment, snapshot, marker and instrument names that the core takes from its caller.
#![allow(dead_code, reason = "each guard binary uses a subset of the helpers")]

use std::str::FromStr as _;

use candler_core::import::rules::{
    HarnessRule, ImportRules, MarkerCode, MarkerRule, SnapshotRule, TreatmentRule,
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
            FieldPath::from_str("resolved_skill_version").unwrap_or_else(|e| panic!("{e}")),
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
        .build()
        .unwrap_or_else(|e| panic!("the rules of the mini-corpus are valid: {e}"))
}
