//! Guards: a session id stands in a command line only as a plain path segment, so `.` and `..`
//! are refused with the same variant as any other unsafe id.
#![cfg(test)]

use candler_harness::spec::{CommandSpec, CommandSpecError, SESSION_ID_PLACEHOLDER};
use candler_schemas::trace::SessionId;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

#[test]
fn a_dot_segment_is_not_a_session_id() -> Res<()> {
    let spec = CommandSpec::builder()
        .argv(vec![String::from("run"), SESSION_ID_PLACEHOLDER.to_owned()])
        .build()?;
    for (id, ok) in [
        ("abc-1.2_x", true),
        ("-x", true),
        (".", false),
        ("..", false),
        ("a/b", false),
    ] {
        let result = spec.with_session_id(&id.parse::<SessionId>()?);
        match (ok, result) {
            (true, Ok(_)) => {}
            (false, Err(CommandSpecError::UnsafeSessionId { id: refused })) => {
                assert_eq!(refused, id);
            }
            (_, other) => return Err(format!("{id:?}: unexpected {other:?}").into()),
        }
    }
    Ok(())
}
