#![cfg(test)]
//! Guards for the secrets step: one check per declared secret, and never a value.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use candler_core::resolve::config::{Config, UserConfig};
use candler_core::resolve::layers::ResolveInput;
use candler_core::resolve::preflight::{ResolveError, Resolver};
use candler_core::resolve::steps::secrets::register_secret_step;
use candler_schemas::primitives::SecretName;

fn secret(text: &str) -> SecretName {
    text.parse().expect("a secret name")
}

fn resolver(cwd: &Path, declared: &[&str], present: &[&str]) -> Resolver {
    let config = Config::new(
        cwd.to_owned(), // BORROW: the config owns its working directory
        None,
        UserConfig::default(),
        Some(cwd.join("state")),
    );
    let declared: Vec<SecretName> = declared.iter().map(|name| secret(name)).collect();
    let present: BTreeSet<SecretName> = present.iter().map(|name| secret(name)).collect();
    register_secret_step(Resolver::new(Arc::new(config)), declared, present)
        .expect("the step registers")
}

/// Guards: O1 — the secrets step turns presence into one check per declared secret: of three
/// declared secrets with one present, the resolution fails with a failed check for each of the two
/// missing ones, naming each and holding no value; with all three present it passes and reports a
/// passing check for each. Oracle: hand-written expectations (the three names, the one present; the present one is PATH, whose value in the process is the planted text no explanation may hold).
/// Bite: a missing secret passing; a value in the explanation.
#[tokio::test]
async fn o1_the_secrets_step_derives_presence_and_fails_each_missing_secret() {
    let dir = tempfile::tempdir().expect("a directory");
    let declared = ["ALPHA_TOKEN", "PATH", "GAMMA_TOKEN"];

    let partial = resolver(dir.path(), &declared, &["PATH"]);
    let error = partial
        .resolve(ResolveInput::default())
        .await
        .expect_err("two secrets are missing");
    let ResolveError::Preflight { reports, errors } = error else {
        panic!("a preflight failure was expected");
    };
    assert!(errors.is_empty(), "the step returns no error");
    assert_eq!(reports.len(), 1, "one step reported");
    let failed: Vec<String> = reports
        .iter()
        .flat_map(|report| report.checks.iter())
        .filter(|check| !check.passed)
        .map(|check| check.name.name().to_owned()) // BORROW: the list owns the names
        .collect();
    assert_eq!(failed, ["secret_ALPHA_TOKEN", "secret_GAMMA_TOKEN"]);
    let all: Vec<_> = reports.iter().flat_map(|r| r.checks.iter()).collect();
    assert_eq!(all.len(), 3, "one check per declared secret");
    for check in all {
        let name = check.name.name().trim_start_matches("secret_");
        assert!(
            check.explanation.contains(name),
            "the explanation names the secret: {}",
            check.explanation
        );
    }

    let planted = std::env::var("PATH").expect("the process has a PATH");
    for check in reports.iter().flat_map(|report| report.checks.iter()) {
        assert!(
            !check.explanation.contains(planted.as_str()),
            "an explanation holds a value: {}",
            check.explanation
        );
    }

    let full = resolver(dir.path(), &declared, &declared);
    let resolution = full
        .resolve(ResolveInput::default())
        .await
        .expect("every secret is present");
    let report = resolution.preflight.first().expect("the step reported");
    assert_eq!(report.checks.len(), 3);
    assert!(report.checks.iter().all(|check| check.passed));
    assert!(report.derived.is_empty(), "the step derives no value");
}
