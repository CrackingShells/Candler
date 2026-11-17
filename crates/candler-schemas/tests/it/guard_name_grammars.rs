//! Guards the name grammars `primitives` owns: each table pins the output of the helper it
//! replaced (`segment`, `reason_of`, `plain_session_id`, `is_env_name`, `is_key`, the
//! `driver.rs` closure) byte for byte, except where a narrowing was ruled.

use candler_schemas::primitives::{
    FieldPath, ReasonCode, is_env_var_name, plain_arg, plain_segment,
};

#[test]
fn segment_of_folds_a_name_to_a_field_path_segment() {
    let cases = [
        ("inputs", "inputs"),
        ("1", "x1"),
        ("x1", "x1"),
        ("A", "a"),
        ("a", "a"),
        ("Z9", "z9"),
        ("Foo-Bar", "foo_bar"),
        ("a.b c", "a_b_c"),
        ("_a", "x_a"),
        ("", "x"),
        ("\u{e9}", "x_"),
    ];
    for (name, expected) in cases {
        assert_eq!(FieldPath::segment_of(name), expected, "segment of {name:?}");
    }
}

#[test]
fn segment_of_collides_where_the_fold_does() {
    assert_eq!(FieldPath::segment_of("1"), FieldPath::segment_of("x1"));
    assert_eq!(FieldPath::segment_of("A"), FieldPath::segment_of("a"));
    assert_ne!(FieldPath::segment_of("a-b"), FieldPath::segment_of("a.c"));
}

#[test]
fn from_lossy_folds_an_error_type_to_a_reason_code() {
    let cases = [
        ("provider.no-route", Some("provider_no_route")),
        ("Aborted", Some("aborted")),
        ("a", Some("a")),
        ("ApiError9", Some("apierror9")),
        ("1x", None),
        ("_x", None),
        ("", None),
        ("-", None),
        ("\u{e9}a", None),
    ];
    for (kind, expected) in cases {
        let got = ReasonCode::from_lossy(kind);
        assert_eq!(got.as_ref().map(ReasonCode::as_str), expected, "{kind:?}");
    }
}

#[test]
fn is_env_var_name_is_posix() {
    let cases = [
        ("A", true),
        ("_", true),
        ("a1", true),
        ("A_b9", true),
        ("_1", true),
        ("1a", false),
        ("", false),
        ("a-b", false),
        ("a b", false),
        ("a=b", false),
        ("\u{e9}", false),
    ];
    for (text, expected) in cases {
        assert_eq!(is_env_var_name(text), expected, "{text:?}");
    }
}

#[test]
fn plain_segment_and_plain_arg_differ_by_the_leading_dash() {
    // (text, plain_segment, plain_arg)
    let cases = [
        ("name", true, true),
        ("a.b", true, true),
        ("x-", true, true),
        ("_x", true, true),
        ("-x", true, false),
        ("-", true, false),
        (".", false, false),
        ("..", false, false),
        ("", false, false),
        ("a/b", false, false),
        ("a b", false, false),
        ("\u{e9}", false, false),
    ];
    for (text, segment, arg) in cases {
        assert_eq!(plain_segment(text), segment, "plain_segment {text:?}");
        assert_eq!(plain_arg(text), arg, "plain_arg {text:?}");
    }
}
