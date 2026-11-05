#![cfg(test)]
//! RFC 8785 canonical JSON: the digest-bearing text of every hashed JSON value.

use std::path::Path;

use candler_schemas::canonical::{CanonicalError, check, from_text, to_jcs};
use serde_json::{Number, Value};

fn double(bits: u64) -> Value {
    Number::from_f64(f64::from_bits(bits)).map_or(Value::Null, Value::Number)
}

/// Guards: Cond1 (cell digest content) — a number is written as ECMAScript writes it, so digests
/// of JSON values agree across implementations; keys are ordered by UTF-16 code units, strings
/// escape exactly the characters RFC 8785 names. Oracle: RFC 8785 Appendix B (Table 1), §3.2.2
/// and §3.2.3, copied as literals.
#[test]
fn rfc_8785_appendix_b_numbers_the_sample_and_the_sorting_vector() {
    // Appendix B, Table 1: (IEEE 754 bits, JSON representation).
    let table: [(u64, &str); 25] = [
        (0x0000_0000_0000_0000, "0"),
        (0x8000_0000_0000_0000, "0"),
        (0x0000_0000_0000_0001, "5e-324"),
        (0x8000_0000_0000_0001, "-5e-324"),
        (0x7fef_ffff_ffff_ffff, "1.7976931348623157e+308"),
        (0xffef_ffff_ffff_ffff, "-1.7976931348623157e+308"),
        (0x4340_0000_0000_0000, "9007199254740992"),
        (0xc340_0000_0000_0000, "-9007199254740992"),
        (0x4430_0000_0000_0000, "295147905179352830000"),
        (0x44b5_2d02_c7e1_4af5, "9.999999999999997e+22"),
        (0x44b5_2d02_c7e1_4af6, "1e+23"),
        (0x44b5_2d02_c7e1_4af7, "1.0000000000000001e+23"),
        (0x444b_1ae4_d6e2_ef4e, "999999999999999700000"),
        (0x444b_1ae4_d6e2_ef4f, "999999999999999900000"),
        (0x444b_1ae4_d6e2_ef50, "1e+21"),
        (0x3eb0_c6f7_a0b5_ed8c, "9.999999999999997e-7"),
        (0x3eb0_c6f7_a0b5_ed8d, "0.000001"),
        (0x41b3_de43_5555_5553, "333333333.3333332"),
        (0x41b3_de43_5555_5554, "333333333.33333325"),
        (0x41b3_de43_5555_5555, "333333333.3333333"),
        (0x41b3_de43_5555_5556, "333333333.3333334"),
        (0x41b3_de43_5555_5557, "333333333.33333343"),
        (0xbecb_f647_612f_3696, "-0.0000033333333333333333"),
        (0x4314_3ff3_c1cb_0959, "1424953923781206.2"),
        (0x3ff0_0000_0000_0000, "1"),
    ];
    for (bits, expected) in table {
        assert_eq!(
            to_jcs(&double(bits)).as_deref(),
            Ok(expected),
            "{bits:016x}"
        );
    }
    // Appendix B rows 3 and 4 of the notes: NaN and Infinity have no JSON text.
    assert_eq!(Number::from_f64(f64::NAN), None);
    assert_eq!(Number::from_f64(f64::INFINITY), None);

    // §3.2.2 input, §3.2.3 output.
    let sample = r#"{
        "numbers": [333333333.33333329, 1E30, 4.50, 2e-3, 0.000000000000000000000000001],
        "string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/",
        "literals": [null, true, false]
    }"#;
    let expected = r#"{"literals":[null,true,false],"numbers":[333333333.3333333,1e+30,4.5,0.002,1e-27],"string":"€$\u000f\nA'B\"\\\\\"/"}"#;
    assert_eq!(
        from_text(sample).and_then(|v| to_jcs(&v)).as_deref(),
        Ok(expected)
    );

    // §3.2.3 sorting sample: UTF-16 order puts U+1F600 (D83D DE00) before U+FB33.
    let unsorted = r#"{
        "\u20ac": "Euro Sign",
        "\r": "Carriage Return",
        "\ufb33": "Hebrew Letter Dalet With Dagesh",
        "1": "One",
        "\ud83d\ude00": "Emoji: Grinning Face",
        "\u0080": "Control",
        "\u00f6": "Latin Small Letter O With Diaeresis"
    }"#;
    let sorted = from_text(unsorted)
        .and_then(|v| to_jcs(&v))
        .unwrap_or_default();
    let values: Vec<&str> = sorted
        .split("\":\"")
        .skip(1)
        .map(|rest| rest.split('"').next().unwrap_or_default())
        .collect();
    assert_eq!(
        values,
        [
            "Carriage Return",
            "One",
            "Control",
            "Latin Small Letter O With Diaeresis",
            "Euro Sign",
            "Emoji: Grinning Face",
            "Hebrew Letter Dalet With Dagesh",
        ]
    );
}

/// Guards: Cond1 (cell digest content) — every finite double has the ECMAScript text, across
/// powers of 2, subnormals, the 1e21 and 1e-7 switches, the extremes and random mantissas.
/// Oracle: `String(x)` computed by Node (`tests/vectors/jcs/gen_doubles.mjs`, named in the TSV
/// header).
#[test]
fn doubles_are_written_as_node_writes_them() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/jcs/doubles.tsv");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let mut checked = 0_usize;
    for line in text.lines().filter(|line| !line.starts_with('#')) {
        let (hex, expected) = line
            .split_once('\t')
            .unwrap_or_else(|| panic!("line {line:?}"));
        let bits = u64::from_str_radix(hex, 16).unwrap_or_else(|e| panic!("{hex}: {e}"));
        assert_eq!(to_jcs(&double(bits)).as_deref(), Ok(expected), "{hex}");
        checked += 1;
    }
    assert!(checked >= 2000, "the table holds {checked} doubles");
}

/// Guards: Cond1 (cell digest content) — a value with no canonical text is refused rather than
/// rounded or merged: integers beyond ±(2^53 − 1), a lone surrogate escape, a repeated key.
/// Oracle: RFC 8785 §3.2.2.2 and §3.2.2.3 (lone surrogates and numbers beyond the I-JSON range
/// must fail), and the design's rule on integers; each row's positive control is its neighbour.
#[test]
fn values_without_a_canonical_text_are_refused_and_their_neighbours_pass() {
    const MAX: i64 = (1 << 53) - 1;
    // (name, refused text, accepted control)
    let rows: [(&str, &str, &str); 8] = [
        ("integer 2^53", "9007199254740992", "9007199254740991"),
        ("integer -(2^53)", "-9007199254740992", "-9007199254740991"),
        (
            "integer in an array",
            "[1,[9007199254740993]]",
            "[1,[9007199254740991]]",
        ),
        (
            "integer beyond u64",
            "18446744073709551616",
            "1844674407370955",
        ),
        ("lone high surrogate", r#""\ud800""#, "\"😀\""),
        (
            "lone low surrogate in a key",
            r#"{"\udc00":1}"#,
            r#"{"😀":1}"#,
        ),
        (
            "high surrogate before a non-surrogate escape",
            r#""\ud800\u0041""#,
            r#""A""#,
        ),
        (
            "duplicate key",
            r#"{"a":1,"b":2,"a":3}"#,
            r#"{"a":1,"b":2,"c":3}"#,
        ),
    ];
    for (name, bad, good) in rows {
        assert!(from_text(good).is_ok(), "{name}: control {good}");
        assert!(from_text(bad).is_err(), "{name}: {bad}");
    }
    assert!(matches!(
        from_text(r#"{"a":1,"a":2}"#),
        Err(CanonicalError::DuplicateKey { key }) if key == "a"
    ));
    assert!(matches!(
        from_text(r#""\ud800""#),
        Err(CanonicalError::LoneSurrogate { .. })
    ));
    assert!(matches!(
        from_text("9007199254740992"),
        Err(CanonicalError::IntegerRange { .. })
    ));
    // The writer refuses the same integers held as values, and passes the controls.
    for refused in [MAX + 1, -MAX - 1, i64::MAX, i64::MIN] {
        assert!(to_jcs(&Value::from(refused)).is_err(), "{refused}");
        assert!(check(&Value::from(refused)).is_err(), "{refused}");
    }
    assert!(to_jcs(&Value::from(u64::MAX)).is_err());
    for accepted in [MAX, -MAX, 0] {
        assert_eq!(
            to_jcs(&Value::from(accepted)),
            Ok(accepted.to_string()),
            "{accepted}"
        );
    }
}
