//! Guards: every agent setting is reached by a configuration path or is listed in `NO_PATH` with
//! where it comes from, `NO_PATH` lists nothing a path reaches, and every driver answers every
//! setting a path reaches with words.

use std::mem::discriminant;

use candler_core::resolve::paths::{CONFIGURATION_PATHS, NO_PATH, Reach};
use candler_harness::spec::AgentSetting;

use crate::drivers::Registry;

fn reached(setting: AgentSetting) -> bool {
    CONFIGURATION_PATHS.iter().any(|row| match row.reach {
        Reach::Agent(keys) => keys
            .iter()
            .any(|key| discriminant(key) == discriminant(&setting)),
        Reach::CandlerOnly => false,
    })
}

#[test]
fn every_agent_setting_is_reached_by_a_path_or_has_no_path() {
    let stray: Vec<AgentSetting> = AgentSetting::ALL
        .into_iter()
        .filter(|setting| {
            !reached(*setting)
                && !NO_PATH
                    .iter()
                    .any(|(listed, _)| discriminant(listed) == discriminant(setting))
        })
        .collect();
    assert!(stray.is_empty(), "reached by no path: {stray:?}");
    let stale: Vec<AgentSetting> = NO_PATH
        .iter()
        .map(|(listed, _)| *listed)
        .filter(|listed| reached(*listed))
        .collect();
    assert!(stale.is_empty(), "listed in NO_PATH and reached: {stale:?}");
}

#[test]
fn every_driver_answers_every_path_that_reaches_the_agent() {
    let registry = Registry::builtin();
    for name in registry.names() {
        let Ok(driver) = registry.driver(&name) else {
            panic!("{name} is registered");
        };
        for row in &CONFIGURATION_PATHS {
            let Reach::Agent(keys) = row.reach else {
                continue;
            };
            for key in keys {
                assert!(
                    !driver.answer(*key).text().is_empty(),
                    "{name}: {} has no answer for {key:?}",
                    row.path
                );
            }
        }
    }
}

/// Guards: the harnesses the binary registers are the harnesses `candler_harness::spec::HARNESS_IDS`
/// lists, which the keys of a cell's `hooks` and `instructions.global` are checked against.
/// Oracle: the list is written in `candler-harness`; the registry builds its names from its drivers.
/// Bite: register a driver the list does not name, or list an id no driver has.
#[test]
fn the_registered_harnesses_are_the_listed_ones() {
    let mut registered = Registry::builtin().names();
    registered.sort();
    let mut listed: Vec<String> = candler_harness::spec::HARNESS_IDS
        .into_iter()
        .map(str::to_owned)
        .collect();
    listed.sort();
    assert_eq!(registered, listed);
}
