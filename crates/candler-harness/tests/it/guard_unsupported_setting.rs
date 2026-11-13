//! The refusal of a setting a harness cannot honour: its code, side, pointer and text.
#![cfg(test)]

use std::str::FromStr as _;

use candler_harness::driver::DriverError;
use candler_schemas::primitives::FieldPath;
use candler_schemas::refusal::{Refusal, Side};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Guards: R01 §Refusals — a driver that cannot honour a setting refuses it with one shared
/// code on the experiment side, carries the setting's pointer into the experiment file so the
/// refusal record names the place, and its text names the setting, the harness and the version.
/// Oracle: the pointers are written by hand from `file_field` (`budget`) and `unknown_path_pointer`
/// (`[fixed]`); the code is the one the coordination ruling of 2026-11-12 names.
#[test]
fn an_unsupported_setting_is_one_coded_refusal_at_its_pointer() -> Res<()> {
    let version = semver::Version::new(9, 8, 7);
    for (setting, pointer) in [
        ("budget.per_run.turns", "/budget/per_run/turns"),
        ("model.effort", "/fixed/model.effort"),
    ] {
        let error = DriverError::unsupported(
            &FieldPath::from_str(setting)?,
            "Fakeharness",
            &version,
            "no such flag",
        );
        assert_eq!(
            Refusal::code(&error).as_str(),
            "experiment.setting_unsupported"
        );
        assert_eq!(Refusal::side(&error), Side::Experiment);
        assert_eq!(Refusal::path(&error).as_deref(), Some(pointer));
        assert_eq!(
            error.to_string(),
            format!("Fakeharness 9.8.7 cannot honour {pointer}: no such flag")
        );
    }
    Ok(())
}
