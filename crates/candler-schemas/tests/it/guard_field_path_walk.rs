//! Guards `FieldPath::relative_to` and `FieldPath::nest`: the prefix rule and the nesting that
//! every reader of a table-valued path shares.

use candler_schemas::primitives::FieldPath;
use serde_json::json;

#[test]
fn relative_to_matches_whole_segments_only() {
    let cases = [
        ("prompt.values", "prompt.values", Some("")),
        ("prompt.values.a", "prompt.values", Some("a")),
        ("prompt.values.a.b", "prompt.values", Some("a.b")),
        ("prompt.valuesx", "prompt.values", None),
        ("prompt.valuesx.a", "prompt.values", None),
        ("prompt", "prompt.values", None),
        ("model.id", "prompt.values", None),
    ];
    for (path, prefix, expected) in cases {
        let path: FieldPath = path.parse().expect("a field path");
        assert_eq!(path.relative_to(prefix), expected, "{path} below {prefix}");
    }
}

#[test]
fn nest_wraps_one_table_per_segment() {
    assert_eq!(FieldPath::nest("a", json!(1)), json!({"a": 1}));
    assert_eq!(FieldPath::nest("a.b", json!(1)), json!({"a": {"b": 1}}));
    assert_eq!(
        FieldPath::nest("a.b.c", json!([1])),
        json!({"a": {"b": {"c": [1]}}})
    );
}
