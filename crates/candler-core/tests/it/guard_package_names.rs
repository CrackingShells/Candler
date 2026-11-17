//! Guards: an import package name the experiment records must be a field-path segment, because
//! it becomes `provenance.packages.<name>.*`. A leading `_` is no segment, so it is refused up
//! front rather than left out of the record.
#![cfg(test)]

use candler_core::isolation::image::package_name_ok;

#[test]
fn a_package_name_is_a_field_path_segment() {
    let cases = [
        ("numpy", true),
        ("a", true),
        ("scikit_learn", true),
        ("pkg2", true),
        ("a_", true),
        ("_x", false),
        ("_", false),
        ("2pkg", false),
        ("Numpy", false),
        ("a-b", false),
        ("a.b", false),
        ("", false),
    ];
    for (name, expected) in cases {
        assert_eq!(package_name_ok(name), expected, "{name:?}");
    }
}
