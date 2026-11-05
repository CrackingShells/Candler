#![cfg(test)]
//! The tagged enums of the wire are read from objects and from nothing else.

use candler_schemas::process::{ExitStatus, ProcessStart};
use candler_schemas::values::ValueSource;
use serde::de::DeserializeOwned;

fn read<T: DeserializeOwned>(text: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(text)
}

/// Guards: I12, I15 — a value source, a process start time and an exit status are written as
/// objects, so a list that happens to start with a kind name does not read as a record of that
/// kind. Oracle: the hand-written list and object texts below, each object being the positive
/// control of its list.
#[test]
fn tagged_enums_refuse_the_list_form_and_accept_the_object_form() {
    assert_eq!(
        read::<ExitStatus>(r#"{"kind":"code","code":3}"#).expect("object form"),
        ExitStatus::Code { code: 3 }
    );
    assert!(
        read::<ExitStatus>(r#"["code",3]"#).is_err(),
        "exit status as a list"
    );
    assert!(
        read::<ExitStatus>(r#"["signal",9]"#).is_err(),
        "signal as a list"
    );

    assert_eq!(
        read::<ProcessStart>(r#"{"kind":"unavailable"}"#).expect("object form"),
        ProcessStart::Unavailable
    );
    assert!(
        read::<ProcessStart>(r#"["unavailable"]"#).is_err(),
        "unavailable start as a list"
    );
    assert!(
        read::<ProcessStart>(r#"["darwin",1,2]"#).is_err(),
        "darwin start as a list"
    );
    assert!(
        read::<ProcessStart>(r#"["linux","3f2a9c1e-5b7d-4e08-9a6c-0123456789ab",7]"#).is_err(),
        "linux start as a list"
    );

    assert_eq!(
        read::<ValueSource>(r#"{"kind":"core_default"}"#).expect("object form"),
        ValueSource::CoreDefault
    );
    assert!(
        read::<ValueSource>(r#"["core_default"]"#).is_err(),
        "core default as a list"
    );
    assert!(
        read::<ValueSource>(r#"["derived","probe@1"]"#).is_err(),
        "derived source as a list"
    );
}
