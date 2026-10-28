//! The outcome record: the result every evaluator persists for one expectation of one run.
//!
//! An [`OutcomeRecord`] is valid however it was made. It is built through
//! [`OutcomeRecordBuilder`], whose [`build`](OutcomeRecordBuilder::build) runs
//! [`OutcomeRecord::validate`], or deserialised through a raw form that is checked the same way.
//! Its members are built through validating constructors for the same reason.
//!
//! An optional field is omitted when absent and never written as `null`; the reader refuses an
//! explicit `null` for every optional field. Unknown fields are ignored on input.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU64;

use schemars::{JsonSchema, Schema};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::json;

use crate::map_only::MapOnly;
use crate::primitives::{
    Digest, EvidenceRef, ExpectationId, FormatError, InstrumentDigest, NAME_PATTERN, NameVersion,
    ReasonCode, RelPath, RequestId, RunKey, SEMVER_PATTERN,
};

/// Why an outcome record or one of its members is not valid.
///
/// Each variant names what is wrong, and most carry the offending values.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum OutcomeError {
    /// A builder was finished without a required field.
    #[error("field is unset (`{field}` is required)")]
    MissingField {
        /// The field that was not set.
        field: &'static str,
    },
    /// A status that requires a cause has none.
    #[error("cause is missing (status {status} requires one)")]
    CauseMissing {
        /// The record's status.
        status: Status,
    },
    /// A `pass` or `fail` carries a cause other than the legacy cause.
    #[error("cause {cause} is not allowed (status {status} admits only the legacy cause)")]
    CauseNotAllowed {
        /// The record's status.
        status: Status,
        /// The kind of the cause that was found.
        cause: &'static str,
    },
    /// A status that has no weight carries one.
    #[error("weight is not allowed (status {status} carries none)")]
    WeightNotAllowed {
        /// The record's status.
        status: Status,
    },
    /// A weight that the rule of its status refuses.
    #[error("weight does not fit its status ({rule}, got {num}/{den})")]
    WeightMismatch {
        /// The record's status.
        status: Status,
        /// The rule the status sets, such as `a pass needs num = den`.
        rule: &'static str,
        /// The numerator.
        num: u64,
        /// The denominator.
        den: u64,
    },
    /// A `gate` or `quality` record that lists a dirty instrument.
    #[error("instrument is dirty (role {role} admits only clean instruments, got {name})")]
    DirtyInstrument {
        /// The record's role.
        role: Role,
        /// The name of the dirty instrument.
        name: String,
    },
    /// A `gate` or `quality` record that carries a probability.
    #[error("probability is not allowed (role {role} carries none)")]
    ProbabilityNotAllowed {
        /// The record's role.
        role: Role,
    },
    /// A `0/0` weight on a `pass` without the legacy cause.
    #[error("weight 0/0 needs the legacy cause (got {cause})")]
    ZeroWeightWithoutLegacy {
        /// The kind of the cause that was found, or `none`.
        cause: &'static str,
    },
    /// A weight whose numerator exceeds its denominator.
    #[error("weight numerator exceeds its denominator (got {num}/{den})")]
    WeightAboveOne {
        /// The numerator.
        num: u64,
        /// The denominator.
        den: u64,
    },
    /// A value fraction with a zero denominator.
    #[error("fraction denominator is zero (got {num}/0)")]
    FractionDenominator {
        /// The numerator.
        num: u64,
    },
    /// A number value that is `NaN` or infinite.
    #[error("number is not finite (got {value})")]
    NumberNotFinite {
        /// The offending number.
        value: f64,
    },
    /// An instrument that names a git commit and is marked dirty.
    #[error("instrument digest names a commit of a dirty tree (got {digest})")]
    InstrumentDirty {
        /// The instrument's digest.
        digest: String,
    },
    /// Evidence with neither a reference nor a note.
    #[error("evidence is empty (it needs a ref or a note)")]
    EvidenceEmpty,
    /// A free-text field with no characters.
    #[error("text is empty (`{field}` needs at least one character)")]
    TextEmpty {
        /// The field that is empty.
        field: &'static str,
    },
    /// A probability outside `[0, 1]`, or `NaN`.
    #[error("probability is outside [0, 1] (got {p})")]
    ProbabilityRange {
        /// The offending probability.
        p: f64,
    },
    /// A text member that does not have the shape of its format.
    #[error(transparent)]
    Format(#[from] FormatError),
}

/// Reads an optional field that, when present, must not be `null`.
///
/// Used with `#[serde(default)]`, so an absent field is `None` and an explicit `null` is an error
/// of the inner type.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A finite real number: neither `NaN` nor infinite. Negative zero is finite.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "f64", into = "f64")]
#[non_exhaustive]
pub struct Finite(f64);

impl Finite {
    /// Builds a finite number.
    ///
    /// # Errors
    /// Returns [`OutcomeError::NumberNotFinite`] if `number` is `NaN` or infinite.
    pub const fn new(number: f64) -> Result<Self, OutcomeError> {
        if number.is_finite() {
            Ok(Self(number))
        } else {
            Err(OutcomeError::NumberNotFinite { value: number })
        }
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Finite {
    type Error = OutcomeError;

    fn try_from(number: f64) -> Result<Self, Self::Error> {
        Self::new(number)
    }
}

impl From<Finite> for f64 {
    fn from(number: Finite) -> Self {
        number.0
    }
}

/// The five-state result of an outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::exhaustive_enums)] // EXHAUSTIVE: the five-state outcome algebra is closed
pub enum Status {
    /// The expectation holds.
    Pass,
    /// The expectation was measured and does not hold.
    Fail,
    /// The expectation could not be measured because something the measurement needs is missing; it counts as a failure and is attributed to its cause.
    Blocked,
    /// The evaluator could not measure the expectation; it is excluded from rates and reported with its cause.
    NotMeasured,
    /// The expectation does not apply to the run; it is excluded from rates and reported with its cause.
    NotApplicable,
}

impl Status {
    /// The wire spelling of the status.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Blocked => "blocked",
            Self::NotMeasured => "not_measured",
            Self::NotApplicable => "not_applicable",
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // BORROW: Status::as_str is the wire spelling of the status
        f.write_str(self.as_str())
    }
}

/// The part an expectation plays in grading, copied from its suite entry when it was graded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::exhaustive_enums)] // EXHAUSTIVE: the three roles are a closed grading vocabulary
pub enum Role {
    /// A requirement the run must meet.
    Gate,
    /// A measure of how well the run did.
    Quality,
    /// A measurement reported without a verdict on the run.
    Descriptive,
}

impl Role {
    /// The wire spelling of the role.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gate => "gate",
            Self::Quality => "quality",
            Self::Descriptive => "descriptive",
        }
    }

    /// Reports whether the role is graded: `gate` and `quality` records carry no probability and
    /// no dirty instrument.
    const fn is_graded(self) -> bool {
        match self {
            Self::Gate | Self::Quality => true,
            Self::Descriptive => false,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // BORROW: Role::as_str is the wire spelling of the role
        f.write_str(self.as_str())
    }
}

/// A part of a whole, written as an object with the integers `num` and `den`: `num` out of `den`, with `den` at least 1.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Fraction")]
struct RawFraction {
    /// The count of parts that were found; an integer of at least 0.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    num: u64,
    /// The count of parts that were looked at; an integer of at least 1.
    #[schemars(range(min = 1))]
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    den: u64,
}

impl TryFrom<MapOnly<RawFraction>> for Fraction {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawFraction>) -> Result<Self, Self::Error> {
        Self::new(raw.0.num, raw.0.den)
    }
}

/// A part of a whole, written as an object with the integers `num` and `den`: `num` out of `den`, with `den` at least 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawFraction>")]
#[non_exhaustive]
pub struct Fraction {
    num: u64,
    den: NonZeroU64,
}

impl Fraction {
    /// Builds a fraction.
    ///
    /// # Errors
    /// Returns [`OutcomeError::FractionDenominator`] if `den` is 0.
    pub const fn new(num: u64, den: u64) -> Result<Self, OutcomeError> {
        match NonZeroU64::new(den) {
            Some(den) => Ok(Self { num, den }),
            None => Err(OutcomeError::FractionDenominator { num }),
        }
    }

    /// The count of parts that were found.
    #[must_use]
    pub const fn num(&self) -> u64 {
        self.num
    }

    /// The count of parts that were looked at, at least 1.
    #[must_use]
    pub const fn den(&self) -> u64 {
        self.den.get()
    }
}

/// What an evaluator measured: an object whose `kind` is `number`, `count`, `fraction`, `text` or `counts`, with the payload under the key named after the kind, except that `fraction` carries `num` and `den` directly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[schemars(deny_unknown_fields)]
#[non_exhaustive]
pub enum Value {
    /// A finite real number, under the key `number`.
    #[non_exhaustive]
    Number {
        /// The measured number, a finite JSON number.
        number: Finite,
    },
    /// A whole number of things, under the key `count`.
    #[non_exhaustive]
    Count {
        /// The count, an integer of at least 0.
        #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
        count: u64,
    },
    /// A count out of a total, as the keys `num` and `den` beside `kind`.
    Fraction(Fraction),
    /// A piece of text, under the key `text`.
    #[non_exhaustive]
    Text {
        /// The text, a JSON string that may be empty.
        text: String,
    },
    /// Counts by name, under the key `counts`.
    #[non_exhaustive]
    Counts {
        /// An object that maps each name, a string, to its count, an integer of at least 0.
        #[schemars(extend("$comment" = "Each count fits in 64 bits; the Rust reader enforces it."))]
        counts: BTreeMap<String, u64>,
    },
}

impl Value {
    /// A number value.
    ///
    /// # Errors
    /// Returns [`OutcomeError::NumberNotFinite`] if `number` is `NaN` or infinite.
    pub const fn number(number: f64) -> Result<Self, OutcomeError> {
        if number.is_finite() {
            Ok(Self::Number {
                number: Finite(number),
            })
        } else {
            Err(OutcomeError::NumberNotFinite { value: number })
        }
    }

    /// A count value.
    #[must_use]
    pub const fn count(count: u64) -> Self {
        Self::Count { count }
    }

    /// A fraction value.
    ///
    /// # Errors
    /// Returns [`OutcomeError::FractionDenominator`] if `den` is 0.
    pub const fn fraction(num: u64, den: u64) -> Result<Self, OutcomeError> {
        match NonZeroU64::new(den) {
            Some(den) => Ok(Self::Fraction(Fraction { num, den })),
            None => Err(OutcomeError::FractionDenominator { num }),
        }
    }

    /// A text value.
    #[must_use]
    pub const fn text(text: String) -> Self {
        Self::Text { text }
    }

    /// A value of named counts.
    #[must_use]
    pub const fn counts(counts: BTreeMap<String, u64>) -> Self {
        Self::Counts { counts }
    }
}

/// The contribution of an outcome to a rate, written as an object with the integers `num` and `den`: `num` out of `den`, with `num` at most `den`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Weight", transform = weight_schema_transform)]
struct RawWeight {
    /// The numerator, an integer from 0 up to `den`.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    num: u64,
    /// The denominator, an integer of at least 1, or 0 in the zero weight `0/0`, which is carried only with a legacy cause on a `pass`.
    #[schemars(extend("$comment" = "Fits in 64 bits; the Rust reader enforces it."))]
    den: u64,
}

impl TryFrom<MapOnly<RawWeight>> for Weight {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawWeight>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        Self::new(raw.num, raw.den)
    }
}

/// The contribution of an outcome to a rate, written as an object with the integers `num` and `den`: `num` out of `den`, with `num` at most `den`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawWeight>")]
#[non_exhaustive]
pub struct Weight {
    num: u64,
    den: u64,
}

impl Weight {
    /// The default weight of a `pass`: 1/1.
    pub const PASS: Self = Self { num: 1, den: 1 };
    /// The default weight of a `fail` or a `blocked`: 0/1.
    pub const FAIL: Self = Self { num: 0, den: 1 };

    /// Builds a weight.
    ///
    /// `0/0` is a weight; it is carried only with a legacy cause on a `pass`.
    ///
    /// # Errors
    /// Returns [`OutcomeError::WeightAboveOne`] if `num` exceeds `den`.
    pub const fn new(num: u64, den: u64) -> Result<Self, OutcomeError> {
        if num > den {
            Err(OutcomeError::WeightAboveOne { num, den })
        } else {
            Ok(Self { num, den })
        }
    }

    /// The numerator.
    #[must_use]
    pub const fn num(&self) -> u64 {
        self.num
    }

    /// The denominator.
    #[must_use]
    pub const fn den(&self) -> u64 {
        self.den
    }

    /// Reports whether the weight is `0/0`.
    #[must_use]
    pub const fn is_zero(&self) -> bool {
        self.den == 0
    }
}

/// The exact tool that made a measurement.
#[derive(Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Instrument", transform = instrument_schema_transform)]
struct RawInstrument {
    /// The instrument's name, a non-empty string with neither `@` nor white space.
    #[schemars(regex(pattern = NAME_PATTERN))]
    name: String,
    /// The instrument's version, a `SemVer` 2.0 string.
    #[schemars(regex(pattern = SEMVER_PATTERN))]
    version: String,
    /// What identifies the instrument's content: a `sha256:` content digest, or a `git:` commit hash of a clean tree.
    digest: InstrumentDigest,
    /// Whether the instrument differs from what its digest names, a boolean; always false for a `git:` digest.
    dirty: bool,
}

impl TryFrom<MapOnly<RawInstrument>> for Instrument {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawInstrument>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        let identity = format!("{}@{}", raw.name, raw.version).parse::<NameVersion>()?;
        Self::new(identity, raw.digest, raw.dirty)
    }
}

impl From<Instrument> for RawInstrument {
    fn from(instrument: Instrument) -> Self {
        Self {
            name: instrument.identity.name().into(),
            version: instrument.identity.version().to_string(),
            digest: instrument.digest,
            dirty: instrument.dirty,
        }
    }
}

/// The exact tool that made a measurement.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawInstrument>", into = "RawInstrument")]
#[non_exhaustive]
pub struct Instrument {
    identity: NameVersion,
    digest: InstrumentDigest,
    dirty: bool,
}

impl Instrument {
    /// Builds an instrument from its name and version, its digest and its dirty flag.
    ///
    /// # Errors
    /// Returns [`OutcomeError::InstrumentDirty`] if `digest` names a git commit and `dirty` is set.
    pub fn new(
        identity: NameVersion,
        digest: InstrumentDigest,
        dirty: bool,
    ) -> Result<Self, OutcomeError> {
        if dirty && digest.is_git() {
            return Err(OutcomeError::InstrumentDirty {
                digest: digest.into(),
            });
        }
        Ok(Self {
            identity,
            digest,
            dirty,
        })
    }

    /// The instrument's name.
    #[must_use]
    pub const fn name<'a>(&'a self) -> &'a str {
        self.identity.name()
    }

    /// The instrument's `SemVer` version.
    #[must_use]
    pub const fn version<'a>(&'a self) -> &'a semver::Version {
        self.identity.version()
    }

    /// The digest that identifies the instrument's content.
    #[must_use]
    pub const fn digest<'a>(&'a self) -> &'a InstrumentDigest {
        &self.digest
    }

    /// Whether the instrument differs from what its digest names.
    #[must_use]
    pub const fn dirty(&self) -> bool {
        self.dirty
    }
}

/// A pointer to, or a remark about, what an outcome rests on; at least one of `ref` and `note` is present.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Evidence", transform = evidence_schema_transform)]
struct RawEvidence {
    /// An evidence reference string: `event:<session-id>#<seq>`, or `path:<run-relative path>` with an optional line anchor.
    #[serde(default, rename = "ref", deserialize_with = "present")]
    reference: Option<EvidenceRef>,
    /// What the evidence says, a non-empty string.
    #[serde(default, deserialize_with = "present")]
    #[schemars(length(min = 1))]
    note: Option<String>,
}

impl TryFrom<MapOnly<RawEvidence>> for Evidence {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawEvidence>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        Self::new(raw.reference, raw.note)
    }
}

/// A pointer to, or a remark about, what an outcome rests on; at least one of `ref` and `note` is present.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawEvidence>")]
#[non_exhaustive]
pub struct Evidence {
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    reference: Option<EvidenceRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

impl Evidence {
    /// Builds evidence from a reference, a note, or both.
    ///
    /// # Errors
    /// Returns [`OutcomeError::EvidenceEmpty`] if both are absent.
    /// Returns [`OutcomeError::TextEmpty`] if the note is empty.
    pub fn new(reference: Option<EvidenceRef>, note: Option<String>) -> Result<Self, OutcomeError> {
        if reference.is_none() && note.is_none() {
            return Err(OutcomeError::EvidenceEmpty);
        }
        if note.as_ref().is_some_and(String::is_empty) {
            return Err(OutcomeError::TextEmpty { field: "note" });
        }
        Ok(Self { reference, note })
    }

    /// The reference to the evidence, if there is one.
    #[must_use]
    pub const fn reference<'a>(&'a self) -> Option<&'a EvidenceRef> {
        self.reference.as_ref()
    }

    /// The note, if there is one.
    #[must_use]
    pub fn note<'a>(&'a self) -> Option<&'a str> {
        // BORROW: explicit .as_deref() instead of Deref coercion
        self.note.as_deref()
    }
}

/// Why the outcome has its status: an object with exactly one of the keys `expectation`, `artifact`, `reason` and `legacy`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Cause {
    /// The identifier of the expectation whose outcome this one depends on, under the key `expectation`.
    Expectation(ExpectationId),
    /// The run-relative POSIX path of the artifact the run did not produce, under the key `artifact`.
    Artifact(RelPath),
    /// A reason code string, under the key `reason`; the codes `not_applicable_to_arm`, `instrument_error` and `deadline` have meanings assigned by the Candler core, and an evaluator's own codes do not reuse them.
    Reason(ReasonCode),
    /// A reason code string, under the key `legacy`, for a record whose status, weight or cause the other causes cannot state exactly, such as a zero weight `0/0` on a `pass`; the code says which case. Evaluators never emit it. It is the only cause a `pass` or `fail` may carry, and a zero weight `0/0` requires it.
    #[deprecated(note = "evaluators never emit it")]
    Legacy(ReasonCode),
}

impl Cause {
    /// The key the cause is written under: `expectation`, `artifact`, `reason` or `legacy`.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Expectation(_) => "expectation",
            Self::Artifact(_) => "artifact",
            Self::Reason(_) => "reason",
            #[allow(deprecated)]
            Self::Legacy(_) => LEGACY_KEY,
        }
    }

    /// Reports whether the cause is the legacy cause.
    #[must_use]
    pub const fn is_legacy(&self) -> bool {
        match self {
            Self::Expectation(_) | Self::Artifact(_) | Self::Reason(_) => false,
            #[allow(deprecated)]
            Self::Legacy(_) => true,
        }
    }
}

/// A calibrated probability that the expectation holds, written as an object with the number `p` and the string `calibration_ref`.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "Probability")]
struct RawProbability {
    /// The probability, a number from 0 to 1.
    #[schemars(range(min = 0.0, max = 1.0))]
    p: f64,
    /// A non-empty string that names the calibration backing the probability.
    #[schemars(length(min = 1))]
    calibration_ref: String,
}

impl TryFrom<MapOnly<RawProbability>> for Probability {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawProbability>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        Self::new(raw.p, raw.calibration_ref)
    }
}

/// A calibrated probability that the expectation holds, written as an object with the number `p` and the string `calibration_ref`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawProbability>")]
#[non_exhaustive]
pub struct Probability {
    p: f64,
    calibration_ref: String,
}

impl Probability {
    /// Builds a probability.
    ///
    /// # Errors
    /// Returns [`OutcomeError::ProbabilityRange`] if `p` is outside `[0, 1]` or `NaN`.
    /// Returns [`OutcomeError::TextEmpty`] if `calibration_ref` is empty.
    pub fn new(p: f64, calibration_ref: String) -> Result<Self, OutcomeError> {
        if !(0.0..=1.0).contains(&p) {
            return Err(OutcomeError::ProbabilityRange { p });
        }
        if calibration_ref.is_empty() {
            return Err(OutcomeError::TextEmpty {
                field: "calibration_ref",
            });
        }
        Ok(Self { p, calibration_ref })
    }

    /// The probability, from 0 to 1.
    #[must_use]
    pub const fn p(&self) -> f64 {
        self.p
    }

    /// The reference to the calibration that backs the probability.
    #[must_use]
    pub const fn calibration_ref<'a>(&'a self) -> &'a str {
        // BORROW: explicit .as_str() instead of Deref coercion
        self.calibration_ref.as_str()
    }
}

/// The result of one evaluator for one expectation of one run, written as an object; `status` decides which `cause` and `weight` the object may carry, and `role` decides whether it may carry a `probability` and a dirty instrument.
#[derive(Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields, rename = "OutcomeRecord", transform = outcome_schema_transform)]
struct RawOutcomeRecord {
    /// The key of the run that was evaluated: an object with its cell digest, replicate and attempt.
    run: RunKey,
    /// The identifier of the expectation that was evaluated; it names an entry of the suite.
    expectation_id: ExpectationId,
    /// The role of the expectation, copied from its suite entry when it was graded: `gate`, `quality` or `descriptive`.
    role: Role,
    /// The evaluator that produced the outcome, as `name@version` with a `SemVer` version.
    evaluator: NameVersion,
    /// An array of the instruments the evaluator measured with; empty when there are none, and with no dirty instrument when the role is `gate` or `quality`.
    instruments: Vec<Instrument>,
    /// A `sha256:` digest of the canonical JSON of the suite entry's `kind`, `params`, `role`, `applies_to` and `depends_on`.
    suite_entry_digest: Digest,
    /// A `sha256:` digest of the canonical JSON of the artifacts, snapshots, inputs and metadata the evaluator received.
    context_digest: Digest,
    /// The identifier of the request that produced the outcome: a `UUIDv7` in lowercase hyphenated form.
    request_id: RequestId,
    /// The result: `pass`, `fail`, `blocked`, `not_measured` or `not_applicable`.
    status: Status,
    /// What was measured, as a value object, if the evaluator reports one.
    #[serde(default, deserialize_with = "present")]
    value: Option<Value>,
    /// The contribution to a rate; absent means the default of the status: `pass` 1/1, `fail` 0/1, `blocked` 0/1. A `pass` has `num` equal to `den`, a `fail` has `num` below `den` and a `blocked` has `num` 0. Absent for `not_measured` and `not_applicable`.
    #[serde(default, deserialize_with = "present")]
    weight: Option<Weight>,
    /// A calibrated probability that the expectation holds, as an object; deterministic evaluators never set it, and it is absent when the role is `gate` or `quality`.
    #[serde(default, deserialize_with = "present")]
    probability: Option<Probability>,
    /// An array of the pieces of evidence the outcome rests on; empty when there are none.
    evidence: Vec<Evidence>,
    /// Why the outcome has its status; required for `blocked`, `not_measured` and `not_applicable`, absent for `pass` and `fail` unless it is the legacy cause.
    #[serde(default, deserialize_with = "present")]
    cause: Option<Cause>,
}

impl TryFrom<MapOnly<RawOutcomeRecord>> for OutcomeRecord {
    type Error = OutcomeError;

    fn try_from(raw: MapOnly<RawOutcomeRecord>) -> Result<Self, Self::Error> {
        let raw = raw.0;
        let record = Self {
            run: raw.run,
            expectation_id: raw.expectation_id,
            role: raw.role,
            evaluator: raw.evaluator,
            instruments: raw.instruments,
            suite_entry_digest: raw.suite_entry_digest,
            context_digest: raw.context_digest,
            request_id: raw.request_id,
            status: raw.status,
            value: raw.value,
            weight: raw.weight,
            probability: raw.probability,
            evidence: raw.evidence,
            cause: raw.cause,
        };
        record.validate()?;
        Ok(record)
    }
}

/// The result of one evaluator for one expectation of one run, written as an object; `status` decides which `cause` and `weight` the object may carry, and `role` decides whether it may carry a `probability` and a dirty instrument.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "MapOnly<RawOutcomeRecord>")]
#[non_exhaustive]
pub struct OutcomeRecord {
    run: RunKey,
    expectation_id: ExpectationId,
    role: Role,
    evaluator: NameVersion,
    instruments: Vec<Instrument>,
    suite_entry_digest: Digest,
    context_digest: Digest,
    request_id: RequestId,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    weight: Option<Weight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    probability: Option<Probability>,
    evidence: Vec<Evidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cause: Option<Cause>,
}

impl OutcomeRecord {
    /// Starts a builder.
    #[must_use]
    pub fn builder() -> OutcomeRecordBuilder {
        OutcomeRecordBuilder::default()
    }

    /// Checks the rules that relate a record's fields to each other.
    ///
    /// A cause is required on `blocked`, `not_measured` and `not_applicable`. On `pass` and `fail`
    /// it is absent or the legacy cause. A weight is absent on `not_measured` and
    /// `not_applicable`; on a `pass` it has `num = den`, on a `fail` `num < den` and on a
    /// `blocked` `num = 0` and `den` of at least 1; a `0/0` weight is carried only with the
    /// legacy cause on a `pass`. A `gate` or `quality` record has no probability and no dirty
    /// instrument.
    ///
    /// # Errors
    /// Returns [`OutcomeError::CauseMissing`] if a status that requires a cause has none.
    /// Returns [`OutcomeError::CauseNotAllowed`] if a `pass` or `fail` carries a cause other than
    /// the legacy cause.
    /// Returns [`OutcomeError::WeightNotAllowed`] if a `not_measured` or `not_applicable` has a
    /// weight.
    /// Returns [`OutcomeError::WeightMismatch`] if the weight breaks the rule of the status.
    /// Returns [`OutcomeError::ZeroWeightWithoutLegacy`] if a `0/0` weight has no legacy cause.
    /// Returns [`OutcomeError::ProbabilityNotAllowed`] if a `gate` or `quality` record has a
    /// probability.
    /// Returns [`OutcomeError::DirtyInstrument`] if a `gate` or `quality` record lists a dirty
    /// instrument.
    /// Returns [`OutcomeError::NumberNotFinite`] if the value is a number that is not finite.
    pub fn validate(&self) -> Result<(), OutcomeError> {
        self.validate_cause()?;
        self.validate_weight()?;
        if self.role.is_graded() {
            let role = self.role;
            if self.probability.is_some() {
                return Err(OutcomeError::ProbabilityNotAllowed { role });
            }
            if let Some(dirty) = self
                .instruments
                .iter()
                .find(|instrument| instrument.dirty())
            {
                return Err(OutcomeError::DirtyInstrument {
                    role,
                    name: dirty.name().into(),
                });
            }
        }
        if let Some(Value::Number { number, .. }) = &self.value
            && !number.get().is_finite()
        {
            return Err(OutcomeError::NumberNotFinite {
                value: number.get(),
            });
        }
        Ok(())
    }

    /// Checks the cause against the status.
    const fn validate_cause(&self) -> Result<(), OutcomeError> {
        let status = self.status;
        match (status, &self.cause) {
            (Status::Pass | Status::Fail, Some(cause)) if !cause.is_legacy() => {
                Err(OutcomeError::CauseNotAllowed {
                    status,
                    cause: cause.kind(),
                })
            }
            (Status::Blocked | Status::NotMeasured | Status::NotApplicable, None) => {
                Err(OutcomeError::CauseMissing { status })
            }
            (
                Status::Pass
                | Status::Fail
                | Status::Blocked
                | Status::NotMeasured
                | Status::NotApplicable,
                _,
            ) => Ok(()),
        }
    }

    /// Checks the weight against the status and the cause.
    fn validate_weight(&self) -> Result<(), OutcomeError> {
        let status = self.status;
        let Some(weight) = self.weight else {
            return Ok(());
        };
        let (num, den) = (weight.num(), weight.den());
        let broken = match status {
            Status::NotMeasured | Status::NotApplicable => {
                return Err(OutcomeError::WeightNotAllowed { status });
            }
            Status::Pass if num != den => Some("a pass needs num = den"),
            Status::Fail if num >= den => Some("a fail needs num < den"),
            Status::Blocked if num != 0 || den == 0 => {
                Some("a blocked needs num = 0 and den of at least 1")
            }
            Status::Pass | Status::Fail | Status::Blocked => None,
        };
        if let Some(rule) = broken {
            return Err(OutcomeError::WeightMismatch {
                status,
                rule,
                num,
                den,
            });
        }
        let legacy = self.cause.as_ref().is_some_and(Cause::is_legacy);
        if weight.is_zero() && !legacy {
            return Err(OutcomeError::ZeroWeightWithoutLegacy {
                cause: self.cause.as_ref().map_or("none", Cause::kind),
            });
        }
        Ok(())
    }

    /// The weight the outcome counts with in a rate: its own, or the default of its status.
    ///
    /// The defaults are 1/1 for `pass` and 0/1 for `fail` and `blocked`. `not_measured` and
    /// `not_applicable` never enter a rate and have none.
    #[must_use]
    pub const fn effective_weight(&self) -> Option<Weight> {
        if let Some(weight) = self.weight {
            return Some(weight);
        }
        match self.status {
            Status::Pass => Some(Weight::PASS),
            Status::Fail | Status::Blocked => Some(Weight::FAIL),
            Status::NotMeasured | Status::NotApplicable => None,
        }
    }

    /// The run that was evaluated.
    #[must_use]
    pub const fn run<'a>(&'a self) -> &'a RunKey {
        &self.run
    }

    /// The expectation that was evaluated.
    #[must_use]
    pub const fn expectation_id<'a>(&'a self) -> &'a ExpectationId {
        &self.expectation_id
    }

    /// The role of the expectation.
    #[must_use]
    pub const fn role(&self) -> Role {
        self.role
    }

    /// The evaluator that produced the outcome.
    #[must_use]
    pub const fn evaluator<'a>(&'a self) -> &'a NameVersion {
        &self.evaluator
    }

    /// The instruments the evaluator measured with.
    #[must_use]
    pub const fn instruments<'a>(&'a self) -> &'a [Instrument] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.instruments.as_slice()
    }

    /// The digest of the suite entry the evaluator was given.
    #[must_use]
    pub const fn suite_entry_digest<'a>(&'a self) -> &'a Digest {
        &self.suite_entry_digest
    }

    /// The digest of the context the evaluator received.
    #[must_use]
    pub const fn context_digest<'a>(&'a self) -> &'a Digest {
        &self.context_digest
    }

    /// The request that produced the outcome.
    #[must_use]
    pub const fn request_id<'a>(&'a self) -> &'a RequestId {
        &self.request_id
    }

    /// The status.
    #[must_use]
    pub const fn status(&self) -> Status {
        self.status
    }

    /// What was measured, if the evaluator reports a value.
    #[must_use]
    pub const fn value<'a>(&'a self) -> Option<&'a Value> {
        self.value.as_ref()
    }

    /// The weight the record states, if it states one.
    #[must_use]
    pub const fn weight(&self) -> Option<Weight> {
        self.weight
    }

    /// The probability, if the record states one.
    #[must_use]
    pub const fn probability<'a>(&'a self) -> Option<&'a Probability> {
        self.probability.as_ref()
    }

    /// What the outcome rests on.
    #[must_use]
    pub const fn evidence<'a>(&'a self) -> &'a [Evidence] {
        // BORROW: explicit .as_slice() instead of Deref coercion
        self.evidence.as_slice()
    }

    /// Why the outcome has its status, if the record states a cause.
    #[must_use]
    pub const fn cause<'a>(&'a self) -> Option<&'a Cause> {
        self.cause.as_ref()
    }
}

/// Builds an [`OutcomeRecord`] and validates it.
///
/// `run`, `expectation_id`, `role`, `evaluator`, `suite_entry_digest`, `context_digest`,
/// `request_id` and `status` are required. The other fields are absent or empty until set.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct OutcomeRecordBuilder {
    run: Option<RunKey>,
    expectation_id: Option<ExpectationId>,
    role: Option<Role>,
    evaluator: Option<NameVersion>,
    instruments: Vec<Instrument>,
    suite_entry_digest: Option<Digest>,
    context_digest: Option<Digest>,
    request_id: Option<RequestId>,
    status: Option<Status>,
    value: Option<Value>,
    weight: Option<Weight>,
    probability: Option<Probability>,
    evidence: Vec<Evidence>,
    cause: Option<Cause>,
}

impl OutcomeRecordBuilder {
    /// Sets the run.
    #[must_use]
    pub fn run(mut self, run: RunKey) -> Self {
        self.run = Some(run);
        self
    }

    /// Sets the expectation.
    #[must_use]
    pub fn expectation_id(mut self, expectation_id: ExpectationId) -> Self {
        self.expectation_id = Some(expectation_id);
        self
    }

    /// Sets the role.
    #[must_use]
    pub const fn role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Sets the evaluator.
    #[must_use]
    pub fn evaluator(mut self, evaluator: NameVersion) -> Self {
        self.evaluator = Some(evaluator);
        self
    }

    /// Adds an instrument.
    #[must_use]
    pub fn instrument(mut self, instrument: Instrument) -> Self {
        self.instruments.push(instrument);
        self
    }

    /// Sets the digest of the suite entry.
    #[must_use]
    pub fn suite_entry_digest(mut self, digest: Digest) -> Self {
        self.suite_entry_digest = Some(digest);
        self
    }

    /// Sets the digest of the context.
    #[must_use]
    pub fn context_digest(mut self, digest: Digest) -> Self {
        self.context_digest = Some(digest);
        self
    }

    /// Sets the request id.
    #[must_use]
    pub fn request_id(mut self, request_id: RequestId) -> Self {
        self.request_id = Some(request_id);
        self
    }

    /// Sets the status.
    #[must_use]
    pub const fn status(mut self, status: Status) -> Self {
        self.status = Some(status);
        self
    }

    /// Sets the value.
    #[must_use]
    pub fn value(mut self, value: Value) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the weight.
    #[must_use]
    pub const fn weight(mut self, weight: Weight) -> Self {
        self.weight = Some(weight);
        self
    }

    /// Sets the probability.
    #[must_use]
    pub fn probability(mut self, probability: Probability) -> Self {
        self.probability = Some(probability);
        self
    }

    /// Adds a piece of evidence.
    #[must_use]
    pub fn evidence(mut self, evidence: Evidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// Sets the cause.
    #[must_use]
    pub fn cause(mut self, cause: Cause) -> Self {
        self.cause = Some(cause);
        self
    }

    /// Finishes the record and validates it.
    ///
    /// # Errors
    /// Returns [`OutcomeError::MissingField`] if a required field was not set.
    /// Returns [`OutcomeError::CauseMissing`] if a status that requires a cause has none.
    /// Returns [`OutcomeError::CauseNotAllowed`] if a `pass` or `fail` carries a cause other than
    /// the legacy cause.
    /// Returns [`OutcomeError::WeightNotAllowed`] if a `not_measured` or `not_applicable` has a
    /// weight.
    /// Returns [`OutcomeError::WeightMismatch`] if the weight breaks the rule of the status.
    /// Returns [`OutcomeError::ZeroWeightWithoutLegacy`] if a `0/0` weight has no legacy cause.
    /// Returns [`OutcomeError::ProbabilityNotAllowed`] if a `gate` or `quality` record has a
    /// probability.
    /// Returns [`OutcomeError::DirtyInstrument`] if a `gate` or `quality` record lists a dirty
    /// instrument.
    /// Returns [`OutcomeError::NumberNotFinite`] if the value is a number that is not finite.
    pub fn build(self) -> Result<OutcomeRecord, OutcomeError> {
        let missing = |field| OutcomeError::MissingField { field };
        let record = OutcomeRecord {
            run: self.run.ok_or_else(|| missing("run"))?,
            expectation_id: self
                .expectation_id
                .ok_or_else(|| missing("expectation_id"))?,
            role: self.role.ok_or_else(|| missing("role"))?,
            evaluator: self.evaluator.ok_or_else(|| missing("evaluator"))?,
            instruments: self.instruments,
            suite_entry_digest: self
                .suite_entry_digest
                .ok_or_else(|| missing("suite_entry_digest"))?,
            context_digest: self
                .context_digest
                .ok_or_else(|| missing("context_digest"))?,
            request_id: self.request_id.ok_or_else(|| missing("request_id"))?,
            status: self.status.ok_or_else(|| missing("status"))?,
            value: self.value,
            weight: self.weight,
            probability: self.probability,
            evidence: self.evidence,
            cause: self.cause,
        };
        record.validate()?;
        Ok(record)
    }
}

/// What the instrument schema says about the version numbers.
const VERSION_COMMENT: &str =
    "Each of the version's three numbers fits in 64 bits; the Rust reader enforces it.";

/// The key under which the legacy cause is written.
const LEGACY_KEY: &str = "legacy";

/// What the record schema says about the weight rules that it cannot state.
const WEIGHT_COMMENT: &str = concat!(
    "A pass has a weight with num equal to den and a fail has num below den; ",
    "the schema does not compare the two, and the Rust reader enforces both. ",
    "The schema states the blocked rule, num 0."
);

/// The statuses that require a cause.
const CAUSE_REQUIRED: [Status; 3] = [Status::Blocked, Status::NotMeasured, Status::NotApplicable];
/// The statuses whose cause, if any, is the legacy cause.
const CAUSE_LEGACY_ONLY: [Status; 2] = [Status::Pass, Status::Fail];
/// The statuses that carry no weight.
const WEIGHT_ABSENT: [Status; 2] = [Status::NotMeasured, Status::NotApplicable];
/// The roles whose records carry no probability and no dirty instrument.
const GRADED: [Role; 2] = [Role::Gate, Role::Quality];

/// The schema of a record whose `status` is one of `statuses`.
fn status_is(statuses: &[Status]) -> serde_json::Value {
    // BORROW: Status::as_str is the wire spelling of the status
    let names: Vec<&str> = statuses.iter().map(|status| status.as_str()).collect();
    json!({"properties": {"status": {"enum": names}}, "required": ["status"]})
}

/// The schema of a record whose `role` is one of `roles`.
fn role_is(roles: &[Role]) -> serde_json::Value {
    // BORROW: Role::as_str is the wire spelling of the role
    let names: Vec<&str> = roles.iter().map(|role| role.as_str()).collect();
    json!({"properties": {"role": {"enum": names}}, "required": ["role"]})
}

/// Adds an `allOf` clause to `schema`.
fn push_clause(schema: &mut Schema, clause: serde_json::Value) {
    if let Some(object) = schema.as_object_mut() {
        let clauses = object
            .entry("allOf")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        if let serde_json::Value::Array(clauses) = clauses {
            clauses.push(clause);
        }
    }
}

/// Adds the rules of [`OutcomeRecord::validate`] that a schema can state, as `if`/`then` clauses:
/// the cause by status, the weight by status, the legacy cause that a `0/0` weight requires, and
/// the probability and instruments of the graded roles.
fn outcome_schema_transform(schema: &mut Schema) {
    let legacy = json!({"properties": {"cause": {"required": [LEGACY_KEY]}}});
    push_clause(
        schema,
        json!({
            "description": "A cause is required for blocked, not_measured and not_applicable.",
            "if": status_is(&CAUSE_REQUIRED),
            "then": {"required": ["cause"]},
        }),
    );
    push_clause(
        schema,
        json!({
            "description": "A pass or fail carries no cause other than the legacy cause.",
            "if": status_is(&CAUSE_LEGACY_ONLY),
            "then": legacy,
        }),
    );
    push_clause(
        schema,
        json!({
            "description": "A not_measured or not_applicable carries no weight.",
            "if": status_is(&WEIGHT_ABSENT),
            "then": {"not": {"required": ["weight"]}},
        }),
    );
    push_clause(
        schema,
        json!({
            "description": "The weight of a blocked has num 0 and den of at least 1.",
            "if": status_is(&[Status::Blocked]),
            "then": {"properties": {"weight": {
                "properties": {"num": {"const": 0}, "den": {"minimum": 1}},
                "required": ["num", "den"],
            }}},
        }),
    );
    let zero_denominator = json!({"properties": {"den": {"const": 0}}, "required": ["den"]});
    push_clause(
        schema,
        json!({
            "description": "A weight of 0/0 requires the legacy cause.",
            "if": {
                "properties": {"weight": zero_denominator},
                "required": ["weight"],
            },
            "then": {"required": ["cause"], "properties": {"cause": {"required": [LEGACY_KEY]}}},
        }),
    );
    push_clause(
        schema,
        json!({
            "description": "A gate or quality record carries no probability.",
            "if": role_is(&GRADED),
            "then": {"not": {"required": ["probability"]}},
        }),
    );
    push_clause(
        schema,
        json!({
            "description": "A gate or quality record lists no dirty instrument.",
            "if": role_is(&GRADED),
            "then": {"properties": {"instruments": {"items": {
                "properties": {"dirty": {"const": false}},
            }}}},
        }),
    );
    schema.insert(
        // BORROW: .to_owned() makes the owned key `Schema::insert` takes
        "$comment".to_owned(),
        WEIGHT_COMMENT.into(),
    );
}

/// States that `num` is at most `den`, which a schema cannot compare.
fn weight_schema_transform(schema: &mut Schema) {
    schema.insert(
        // BORROW: .to_owned() makes the owned key `Schema::insert` takes
        "$comment".to_owned(),
        "The numerator is at most the denominator; the Rust reader enforces it.".into(),
    );
}

/// Requires a clean tree for a git digest and bounds the version numbers.
fn instrument_schema_transform(schema: &mut Schema) {
    // BORROW: .to_owned() makes the owned key `Schema::insert` takes
    schema.insert("$comment".to_owned(), VERSION_COMMENT.into());
    push_clause(
        schema,
        json!({
            "description": "A git digest names the commit of a clean tree.",
            "if": {"properties": {"digest": {"pattern": "^git:"}}, "required": ["digest"]},
            "then": {"properties": {"dirty": {"const": false}}},
        }),
    );
}

/// Requires at least one of `ref` and `note`.
fn evidence_schema_transform(schema: &mut Schema) {
    schema.insert(
        // BORROW: .to_owned() makes the owned key `Schema::insert` takes
        "anyOf".to_owned(),
        json!([{"required": ["ref"]}, {"required": ["note"]}]),
    );
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::generate::GeneratedSchema;

    const HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    const REQUEST: &str = "01926b3a-7c3e-7a1b-8c4d-5e6f7a8b9c0d";

    fn digest() -> Digest {
        format!("sha256:{HEX}").parse().expect("a digest")
    }

    #[allow(deprecated)]
    fn legacy(code: &str) -> Cause {
        Cause::Legacy(code.parse().expect("a reason code"))
    }

    fn reason(code: &str) -> Cause {
        Cause::Reason(code.parse().expect("a reason code"))
    }

    fn weight(num: u64, den: u64) -> Weight {
        Weight::new(num, den).expect("a weight")
    }

    fn builder(status: Status) -> OutcomeRecordBuilder {
        OutcomeRecord::builder()
            .run(RunKey::new(digest(), 1, 1).expect("a run key"))
            .expectation_id("A1".parse().expect("an id"))
            .role(Role::Gate)
            .evaluator("htn-creator-eval@0.4.2".parse().expect("a name@version"))
            .suite_entry_digest(digest())
            .context_digest(digest())
            .request_id(REQUEST.parse().expect("a request id"))
            .status(status)
    }

    fn dirty_instrument() -> Instrument {
        let identity = "local_checker@2.0.0".parse().expect("name@version");
        let digest = format!("sha256:{HEX}").parse().expect("a digest");
        Instrument::new(identity, digest, true).expect("a dirty content digest")
    }

    fn base_json(status: &str) -> serde_json::Value {
        json!({
            "run": {"cell": format!("sha256:{HEX}"), "replicate": 1, "attempt": 1},
            "expectation_id": "A1",
            "role": "gate",
            "evaluator": "htn-creator-eval@0.4.2",
            "instruments": [],
            "suite_entry_digest": format!("sha256:{HEX}"),
            "context_digest": format!("sha256:{HEX}"),
            "request_id": REQUEST,
            "status": status,
            "evidence": [],
        })
    }

    /// An instrument as JSON; `kind` is the digest prefix, `sha256:` or `git:`.
    fn instrument_json(name: &str, version: &str, kind: &str, dirty: bool) -> serde_json::Value {
        let digest = if kind == "git:" {
            format!("git:{}", "ab".repeat(20))
        } else {
            format!("sha256:{HEX}")
        };
        json!({"name": name, "version": version, "digest": digest, "dirty": dirty})
    }

    fn read(value: serde_json::Value) -> Result<OutcomeRecord, serde_json::Error> {
        serde_json::from_value(value)
    }

    /// The number 2^64, which no `u64` holds.
    fn past_u64() -> serde_json::Value {
        serde_json::from_str("18446744073709551616").expect("a JSON number")
    }

    /// The text of a JSON string.
    fn text(value: &serde_json::Value) -> &str {
        // BORROW: serde_json::Value::as_str reads a JSON string as text
        value.as_str().expect("a JSON string")
    }

    #[test]
    fn outcome_pass_and_fail_need_no_cause_and_take_only_the_legacy_one() {
        for status in [Status::Pass, Status::Fail] {
            builder(status).build().expect("no cause is valid");
            builder(status)
                .cause(legacy("zero_weight"))
                .build()
                .expect("the legacy cause is valid");
            for cause in [
                reason("deadline"),
                Cause::Artifact("out/a.json".parse().expect("a path")),
                Cause::Expectation("B2".parse().expect("an id")),
            ] {
                let kind = cause.kind();
                assert_eq!(
                    builder(status).cause(cause).build(),
                    Err(OutcomeError::CauseNotAllowed {
                        status,
                        cause: kind
                    })
                );
            }
        }
    }

    #[test]
    fn outcome_blocked_not_measured_and_not_applicable_need_a_cause() {
        for status in [Status::Blocked, Status::NotMeasured, Status::NotApplicable] {
            assert_eq!(
                builder(status).build(),
                Err(OutcomeError::CauseMissing { status })
            );
            builder(status)
                .cause(reason("instrument_error"))
                .build()
                .expect("a reason is a cause");
            builder(status)
                .cause(legacy("no_replay_target"))
                .build()
                .expect("the legacy cause is a cause");
        }
    }

    #[test]
    fn outcome_not_measured_and_not_applicable_carry_no_weight() {
        for status in [Status::NotMeasured, Status::NotApplicable] {
            assert_eq!(
                builder(status)
                    .cause(reason("deadline"))
                    .weight(weight(0, 1))
                    .build(),
                Err(OutcomeError::WeightNotAllowed { status })
            );
        }
    }

    #[test]
    fn outcome_weight_follows_the_rule_of_the_status() {
        let mismatch = |status, rule, num, den| {
            Err(OutcomeError::WeightMismatch {
                status,
                rule,
                num,
                den,
            })
        };
        let pass = |num, den| builder(Status::Pass).weight(weight(num, den)).build();
        pass(1, 1).expect("a pass weighs num = den");
        pass(35, 35).expect("a pass weighs num = den");
        assert_eq!(
            pass(1, 2),
            mismatch(Status::Pass, "a pass needs num = den", 1, 2)
        );
        assert_eq!(
            pass(0, 3),
            mismatch(Status::Pass, "a pass needs num = den", 0, 3)
        );
        let fail = |num, den| builder(Status::Fail).weight(weight(num, den)).build();
        fail(0, 1).expect("a fail weighs num < den");
        fail(1, 2).expect("a fail weighs num < den");
        assert_eq!(
            fail(3, 3),
            mismatch(Status::Fail, "a fail needs num < den", 3, 3)
        );
        assert_eq!(
            fail(2, 2),
            mismatch(Status::Fail, "a fail needs num < den", 2, 2)
        );
        assert_eq!(
            fail(0, 0),
            mismatch(Status::Fail, "a fail needs num < den", 0, 0)
        );
        let blocked = |num, den| {
            builder(Status::Blocked)
                .cause(reason("deadline"))
                .weight(weight(num, den))
                .build()
        };
        blocked(0, 1).expect("a blocked weighs num = 0");
        blocked(0, 5).expect("a blocked weighs num = 0");
        assert_eq!(
            blocked(1, 2),
            mismatch(
                Status::Blocked,
                "a blocked needs num = 0 and den of at least 1",
                1,
                2
            )
        );
    }

    #[test]
    fn outcome_zero_weight_is_carried_only_with_the_legacy_cause_on_a_pass() {
        let zero = weight(0, 0);
        assert_eq!(
            builder(Status::Pass).weight(zero).build(),
            Err(OutcomeError::ZeroWeightWithoutLegacy { cause: "none" })
        );
        builder(Status::Pass)
            .weight(zero)
            .cause(legacy("zero_weight"))
            .build()
            .expect("a legacy cause carries 0/0 on a pass");
        for cause in [reason("deadline"), legacy("zero_weight")] {
            assert_eq!(
                builder(Status::Blocked).weight(zero).cause(cause).build(),
                Err(OutcomeError::WeightMismatch {
                    status: Status::Blocked,
                    rule: "a blocked needs num = 0 and den of at least 1",
                    num: 0,
                    den: 0
                })
            );
        }
    }

    #[test]
    fn outcome_effective_weight_is_the_stated_weight_or_the_status_default() {
        let pass = builder(Status::Pass).build().expect("valid");
        assert_eq!(pass.effective_weight(), Some(weight(1, 1)));
        let fail = builder(Status::Fail).build().expect("valid");
        assert_eq!(fail.effective_weight(), Some(weight(0, 1)));
        let blocked = builder(Status::Blocked)
            .cause(reason("deadline"))
            .build()
            .expect("valid");
        assert_eq!(blocked.effective_weight(), Some(weight(0, 1)));
        for status in [Status::NotMeasured, Status::NotApplicable] {
            let outside = builder(status)
                .cause(reason("deadline"))
                .build()
                .expect("valid");
            assert_eq!(outside.effective_weight(), None);
        }
        let stated = builder(Status::Fail)
            .weight(weight(1, 2))
            .build()
            .expect("valid");
        assert_eq!(stated.effective_weight(), Some(weight(1, 2)));
        assert_eq!(stated.weight(), Some(weight(1, 2)));
        assert_eq!(pass.weight(), None);
    }

    #[test]
    fn outcome_graded_roles_carry_no_probability_and_no_dirty_instrument() {
        let probability = || Probability::new(0.5, "cal".into()).expect("a probability");
        for role in [Role::Gate, Role::Quality] {
            assert_eq!(
                builder(Status::Pass)
                    .role(role)
                    .probability(probability())
                    .build(),
                Err(OutcomeError::ProbabilityNotAllowed { role })
            );
            assert_eq!(
                builder(Status::Pass)
                    .role(role)
                    .instrument(dirty_instrument())
                    .build(),
                Err(OutcomeError::DirtyInstrument {
                    role,
                    name: "local_checker".into()
                })
            );
        }
        builder(Status::Pass)
            .role(Role::Descriptive)
            .probability(probability())
            .instrument(dirty_instrument())
            .build()
            .expect("a descriptive record may carry both");
        let clean = "local_checker@2.0.0".parse().expect("name@version");
        let digest = format!("sha256:{HEX}").parse().expect("a digest");
        let clean = Instrument::new(clean, digest, false).expect("a clean instrument");
        builder(Status::Pass)
            .role(Role::Gate)
            .instrument(clean)
            .build()
            .expect("a clean instrument is allowed");
    }

    #[test]
    fn outcome_builder_names_the_field_it_lacks() {
        assert_eq!(
            OutcomeRecord::builder().build(),
            Err(OutcomeError::MissingField { field: "run" })
        );
        let mut partial = builder(Status::Pass);
        partial.status = None;
        assert_eq!(
            partial.build(),
            Err(OutcomeError::MissingField { field: "status" })
        );
    }

    #[test]
    fn outcome_members_refuse_what_is_invalid() {
        assert_eq!(
            Fraction::new(3, 0),
            Err(OutcomeError::FractionDenominator { num: 3 })
        );
        assert_eq!(Fraction::new(3, 2).map(|f| (f.num(), f.den())), Ok((3, 2)));
        assert_eq!(
            Weight::new(3, 2),
            Err(OutcomeError::WeightAboveOne { num: 3, den: 2 })
        );
        assert_eq!(
            Weight::new(1, 0),
            Err(OutcomeError::WeightAboveOne { num: 1, den: 0 })
        );
        assert!(weight(0, 0).is_zero());
        assert!(!weight(2, 2).is_zero());
        assert!(matches!(
            Value::number(f64::NAN),
            Err(OutcomeError::NumberNotFinite { .. })
        ));
        assert!(Value::number(f64::INFINITY).is_err());
        assert!(Value::number(f64::NEG_INFINITY).is_err());
        assert!(Value::number(-1.5).is_ok());
        assert!(Value::number(-0.0).is_ok());
        assert!(Value::number(f64::MIN_POSITIVE / 2.0).is_ok());
        assert_eq!(
            Value::fraction(1, 0),
            Err(OutcomeError::FractionDenominator { num: 1 })
        );
        assert_eq!(Evidence::new(None, None), Err(OutcomeError::EvidenceEmpty));
        assert_eq!(
            Evidence::new(None, Some(String::new())),
            Err(OutcomeError::TextEmpty { field: "note" })
        );
        for p in [-0.1, 1.1, f64::NAN] {
            assert!(matches!(
                Probability::new(p, "cal".into()),
                Err(OutcomeError::ProbabilityRange { .. })
            ));
        }
        assert_eq!(
            Probability::new(0.5, String::new()),
            Err(OutcomeError::TextEmpty {
                field: "calibration_ref"
            })
        );
        assert!(Probability::new(0.0, "cal".into()).is_ok());
        assert!(Probability::new(1.0, "cal".into()).is_ok());
    }

    #[test]
    fn outcome_finite_numbers_are_finite_by_construction_and_checked_by_validate() {
        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                Finite::new(number),
                Err(OutcomeError::NumberNotFinite { .. })
            ));
            assert!(Finite::try_from(number).is_err());
            let wire: Result<Finite, _> = serde_json::from_value(json!(number));
            assert!(wire.is_err());
        }
        assert_eq!(Finite::new(-0.0).map(Finite::get), Ok(-0.0));
        // The field is private to this module, so only here can a non-finite number be made.
        let unchecked = Value::Number {
            number: Finite(f64::NAN),
        };
        let built = builder(Status::Pass).value(unchecked).build();
        assert!(matches!(
            built,
            Err(OutcomeError::NumberNotFinite { value }) if value.is_nan()
        ));
    }

    #[test]
    fn outcome_instrument_with_a_git_digest_must_be_clean() {
        let identity: NameVersion = "htn_creator_scripts@0.4.2".parse().expect("name@version");
        let git: InstrumentDigest = format!("git:{}", "ab".repeat(20)).parse().expect("git");
        let sha: InstrumentDigest = format!("sha256:{HEX}").parse().expect("sha");
        assert!(Instrument::new(identity.clone(), git.clone(), false).is_ok());
        assert!(matches!(
            Instrument::new(identity.clone(), git, true),
            Err(OutcomeError::InstrumentDirty { .. })
        ));
        let dirty = Instrument::new(identity, sha, true).expect("a dirty content digest");
        assert!(dirty.dirty());
        assert_eq!(dirty.name(), "htn_creator_scripts");
        assert_eq!(dirty.version().to_string(), "0.4.2");
        assert!(!dirty.digest().is_git());
    }

    /// One value of every variant of [`OutcomeError`], with the position of the variant.
    fn every_error() -> Vec<(usize, OutcomeError)> {
        let status = Status::Blocked;
        let role = Role::Gate;
        let format = "x".parse::<Digest>().expect_err("not a digest");
        let all = vec![
            OutcomeError::MissingField { field: "run" },
            OutcomeError::CauseMissing { status },
            OutcomeError::CauseNotAllowed {
                status: Status::Pass,
                cause: "reason",
            },
            OutcomeError::WeightNotAllowed { status },
            OutcomeError::WeightMismatch {
                status: Status::Pass,
                rule: "a pass needs num = den",
                num: 1,
                den: 2,
            },
            OutcomeError::DirtyInstrument {
                role,
                name: "checker".into(),
            },
            OutcomeError::ProbabilityNotAllowed { role },
            OutcomeError::ZeroWeightWithoutLegacy { cause: "none" },
            OutcomeError::WeightAboveOne { num: 3, den: 2 },
            OutcomeError::FractionDenominator { num: 3 },
            OutcomeError::NumberNotFinite { value: f64::NAN },
            OutcomeError::InstrumentDirty {
                digest: "git:abc".into(),
            },
            OutcomeError::EvidenceEmpty,
            OutcomeError::TextEmpty { field: "note" },
            OutcomeError::ProbabilityRange { p: 1.5 },
            OutcomeError::Format(format),
        ];
        all.into_iter()
            .map(|error| {
                // Naming every variant, with no wildcard, makes a new variant a compile error.
                let position = match &error {
                    OutcomeError::MissingField { .. } => 0,
                    OutcomeError::CauseMissing { .. } => 1,
                    OutcomeError::CauseNotAllowed { .. } => 2,
                    OutcomeError::WeightNotAllowed { .. } => 3,
                    OutcomeError::WeightMismatch { .. } => 4,
                    OutcomeError::DirtyInstrument { .. } => 5,
                    OutcomeError::ProbabilityNotAllowed { .. } => 6,
                    OutcomeError::ZeroWeightWithoutLegacy { .. } => 7,
                    OutcomeError::WeightAboveOne { .. } => 8,
                    OutcomeError::FractionDenominator { .. } => 9,
                    OutcomeError::NumberNotFinite { .. } => 10,
                    OutcomeError::InstrumentDirty { .. } => 11,
                    OutcomeError::EvidenceEmpty => 12,
                    OutcomeError::TextEmpty { .. } => 13,
                    OutcomeError::ProbabilityRange { .. } => 14,
                    OutcomeError::Format(_) => 15,
                };
                (position, error)
            })
            .collect()
    }

    #[test]
    fn outcome_error_messages_follow_the_wording_rule_for_every_variant() {
        let errors = every_error();
        let positions: Vec<usize> = errors.iter().map(|(position, _)| *position).collect();
        assert_eq!(positions, (0..positions.len()).collect::<Vec<_>>());
        for (_, error) in &errors {
            let message = error.to_string();
            assert!(message.contains(" ("), "no context in {message:?}");
            assert!(message.ends_with(')'), "no closing context in {message:?}");
            assert!(!message.ends_with('.'), "trailing period in {message:?}");
            assert_eq!(message, message.trim(), "stray white space in {message:?}");
        }
        assert_eq!(
            OutcomeError::WeightAboveOne { num: 3, den: 2 }.to_string(),
            "weight numerator exceeds its denominator (got 3/2)"
        );
        assert_eq!(
            OutcomeError::WeightMismatch {
                status: Status::Fail,
                rule: "a fail needs num < den",
                num: 3,
                den: 3
            }
            .to_string(),
            "weight does not fit its status (a fail needs num < den, got 3/3)"
        );
    }

    #[test]
    fn outcome_serialisation_omits_absent_options_and_never_writes_null() {
        let record = builder(Status::Pass).build().expect("valid");
        let written = serde_json::to_value(&record).expect("serialises");
        assert_eq!(written, base_json("pass"));
        assert!(
            !serde_json::to_string(&record)
                .expect("text")
                .contains("null")
        );
        let full = builder(Status::Blocked)
            .role(Role::Descriptive)
            .cause(reason("deadline"))
            .value(Value::count(3))
            .weight(weight(0, 2))
            .probability(Probability::new(0.25, "cal-1".into()).expect("a probability"))
            .evidence(
                Evidence::new(Some("path:out/a.py#L1-2".parse().expect("ref")), None)
                    .expect("evidence"),
            )
            .evidence(Evidence::new(None, Some("seen".into())).expect("evidence"))
            .build()
            .expect("valid");
        let text = serde_json::to_string(&full).expect("text");
        assert!(!text.contains("null"));
        let back: OutcomeRecord = serde_json::from_str(&text).expect("reads what it wrote");
        assert_eq!(back, full);
    }

    #[test]
    fn outcome_reader_refuses_null_for_every_optional_field() {
        let mut blocked = base_json("blocked");
        blocked["role"] = json!("descriptive");
        blocked["cause"] = json!({"reason": "deadline"});
        blocked["value"] = json!({"kind": "count", "count": 1});
        blocked["weight"] = json!({"num": 0, "den": 1});
        blocked["probability"] = json!({"p": 0.5, "calibration_ref": "c"});
        blocked["evidence"] = json!([{"ref": "event:s#0", "note": "n"}]);
        read(blocked.clone()).expect("the full record reads");
        for field in ["value", "weight", "probability", "cause"] {
            let mut record = blocked.clone();
            record[field] = json!(null);
            assert!(read(record).is_err(), "a null {field} must be refused");
        }
        for field in ["ref", "note"] {
            let mut record = blocked.clone();
            record["evidence"][0][field] = json!(null);
            assert!(read(record).is_err(), "a null {field} must be refused");
        }
        let mut required = base_json("pass");
        required["role"] = json!(null);
        assert!(read(required).is_err());
    }

    #[test]
    fn outcome_reader_runs_the_cross_field_rules() {
        let mut record = base_json("pass");
        record["cause"] = json!({"reason": "deadline"});
        assert!(read(record).is_err());
        assert!(read(base_json("blocked")).is_err());
        let mut record = base_json("not_measured");
        record["cause"] = json!({"reason": "deadline"});
        record["weight"] = json!({"num": 0, "den": 0});
        assert!(read(record).is_err());
        let mut record = base_json("pass");
        record["weight"] = json!({"num": 0, "den": 0});
        assert!(read(record.clone()).is_err());
        record["cause"] = json!({"legacy": "zero_weight"});
        read(record).expect("0/0 with the legacy cause reads");
        let mut record = base_json("fail");
        record["weight"] = json!({"num": 2, "den": 2});
        assert!(read(record).is_err());
        let mut record = base_json("pass");
        record["probability"] = json!({"p": 0.5, "calibration_ref": "c"});
        assert!(read(record.clone()).is_err());
        record["role"] = json!("descriptive");
        read(record).expect("a descriptive record may carry a probability");
        let mut record = base_json("pass");
        record["instruments"] = json!([instrument_json("i", "1.0.0", "sha256:", true)]);
        assert!(read(record.clone()).is_err());
        record["role"] = json!("descriptive");
        read(record).expect("a descriptive record may list a dirty instrument");
    }

    #[test]
    fn outcome_reader_ignores_unknown_fields_and_requires_collections() {
        let mut record = base_json("pass");
        record["future"] = json!(1);
        record["run"]["future"] = json!(1);
        record["evidence"] = json!([{"note": "n", "future": true}]);
        let mut instrument = instrument_json("i", "1.0.0", "sha256:", false);
        instrument["future"] = json!(1);
        record["instruments"] = json!([instrument]);
        read(record).expect("unknown fields are ignored");
        for field in ["instruments", "evidence"] {
            let mut record = base_json("pass");
            if let Some(object) = record.as_object_mut() {
                object.remove(field);
            }
            assert!(read(record).is_err(), "{field} is required");
        }
    }

    #[test]
    fn outcome_reader_reads_objects_only() {
        let mut record = base_json("fail");
        record["weight"] = json!([1, 2]);
        assert!(read(record).is_err(), "a weight written as a list");
        let mut record = base_json("pass");
        record["probability"] = json!(["0.5", "c"]);
        record["role"] = json!("descriptive");
        assert!(read(record).is_err(), "a probability written as a list");
        let mut record = base_json("pass");
        record["evidence"] = json!([["event:s#0", "n"]]);
        assert!(read(record).is_err(), "evidence written as a list");
        let mut record = base_json("pass");
        record["instruments"] = json!([["i", "1.0.0", format!("sha256:{HEX}"), false]]);
        assert!(read(record).is_err(), "an instrument written as a list");
        let mut record = base_json("pass");
        record["value"] = json!(["fraction", 1, 2]);
        assert!(read(record).is_err(), "a value written as a list");
        let fraction: Result<Value, _> = serde_json::from_value(json!(["fraction", 1, 2]));
        assert!(fraction.is_err());
        let fraction: Result<Fraction, _> = serde_json::from_value(json!([1, 2]));
        assert!(fraction.is_err(), "a fraction written as a list");
        let whole = json!([
            {"cell": format!("sha256:{HEX}"), "replicate": 1, "attempt": 1},
            "A1", "gate", "htn-creator-eval@0.4.2", [], format!("sha256:{HEX}"),
            format!("sha256:{HEX}"), REQUEST, "pass", [], null, null, null, null
        ]);
        assert!(read(whole).is_err(), "a record written as a list");
    }

    #[test]
    fn outcome_reader_refuses_malformed_members() {
        let cases = [
            ("status", json!("not-measured")),
            ("role", json!("Gate")),
            ("expectation_id", json!("")),
            ("evaluator", json!("htn-creator-eval")),
            ("suite_entry_digest", json!("sha256:xyz")),
            ("request_id", json!("not-a-uuid")),
            (
                "run",
                json!({"cell": "sha256:xyz", "replicate": 1, "attempt": 1}),
            ),
            ("value", json!({"kind": "fraction", "num": 1, "den": 0})),
            ("value", json!({"kind": "number"})),
            ("value", json!({"kind": "unknown", "unknown": 1})),
            ("value", json!({"kind": "count", "count": 1.5})),
            ("value", json!({"kind": "count", "count": -1})),
            ("value", json!({"kind": "count", "count": past_u64()})),
            ("weight", json!({"num": 3, "den": 2})),
            ("weight", json!({"num": 1})),
            ("probability", json!({"p": 1.5, "calibration_ref": "c"})),
            ("probability", json!({"p": 0.5, "calibration_ref": ""})),
            ("evidence", json!([{}])),
            ("evidence", json!([{"ref": "event:s"}])),
            (
                "instruments",
                json!([instrument_json("a@b", "1.0.0", "sha256:", false)]),
            ),
            (
                "instruments",
                json!([instrument_json("a", "1.0", "sha256:", false)]),
            ),
            (
                "instruments",
                json!([instrument_json("a", "1.0.0", "git:", true)]),
            ),
        ];
        for (field, bad) in cases {
            let mut record = base_json("pass");
            record[field] = bad.clone();
            assert!(read(record).is_err(), "{field} = {bad} must be refused");
        }
        for bad in [
            json!({"reason": "deadline", "artifact": "a"}),
            json!({}),
            json!({"reason": "Deadline"}),
            json!({"artifact": "../a"}),
            json!({"unknown": "x"}),
        ] {
            let mut record = base_json("blocked");
            record["cause"] = bad.clone();
            assert!(read(record).is_err(), "cause {bad} must be refused");
        }
    }

    #[test]
    fn outcome_a_fraction_with_a_zero_denominator_is_refused_in_the_project_wording() {
        let error = serde_json::from_value::<Fraction>(json!({"num": 1, "den": 0}))
            .expect_err("a zero denominator");
        assert!(
            error
                .to_string()
                .contains("fraction denominator is zero (got 1/0)"),
            "{error}"
        );
        let error =
            serde_json::from_value::<Value>(json!({"kind": "fraction", "num": 1, "den": 0}))
                .expect_err("a zero denominator");
        assert!(
            error.to_string().contains("fraction denominator is zero"),
            "{error}"
        );
    }

    #[test]
    fn outcome_values_read_and_write_their_documented_shapes() {
        let shapes = [
            json!({"kind": "number", "number": 1.46}),
            json!({"kind": "count", "count": 14}),
            json!({"kind": "fraction", "num": 0, "den": 14}),
            json!({"kind": "text", "text": "ok"}),
            json!({"kind": "counts", "counts": {"measured": 3}}),
        ];
        for shape in shapes {
            let value: Value = serde_json::from_value(shape.clone()).expect("reads");
            assert_eq!(serde_json::to_value(&value).expect("writes"), shape);
        }
    }

    #[test]
    fn outcome_cause_kind_names_the_wire_key() {
        assert_eq!(reason("deadline").kind(), "reason");
        assert_eq!(legacy("x").kind(), "legacy");
        assert!(legacy("x").is_legacy());
        assert!(!reason("x").is_legacy());
        assert_eq!(Status::NotApplicable.to_string(), "not_applicable");
        assert_eq!(Role::Descriptive.to_string(), "descriptive");
    }

    fn schema_text() -> serde_json::Value {
        GeneratedSchema::of::<OutcomeRecord>("outcome-record")
            .schema
            .to_value()
    }

    fn validator() -> jsonschema::Validator {
        jsonschema::validator_for(&schema_text()).expect("the schema compiles")
    }

    /// Collects the values of every key called `key` in a JSON document.
    fn collect<'a>(
        value: &'a serde_json::Value,
        key: &str,
        found: &mut Vec<&'a serde_json::Value>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (name, inner) in map {
                    // BORROW: explicit .as_str() to match the key against the argument
                    if name.as_str() == key {
                        found.push(inner);
                    }
                    collect(inner, key, found);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    collect(item, key, found);
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }

    #[test]
    fn outcome_schema_is_strict_and_states_the_by_status_rules() {
        let schema = validator();
        let valid = |value: &serde_json::Value| schema.is_valid(value);
        assert!(valid(&base_json("pass")));
        let with = |status: &str, edits: &[(&str, serde_json::Value)]| {
            let mut record = base_json(status);
            for (key, value) in edits {
                record[*key] = value.clone();
            }
            record
        };
        let reason = json!({"reason": "deadline"});
        let legacy = json!({"legacy": "zero_weight"});
        let zero = json!({"num": 0, "den": 0});
        let half = json!({"num": 1, "den": 2});
        for status in ["blocked", "not_measured", "not_applicable"] {
            assert!(!valid(&with(status, &[])), "{status} needs a cause");
            assert!(valid(&with(status, &[("cause", reason.clone())])));
            assert!(valid(&with(status, &[("cause", legacy.clone())])));
        }
        for status in ["pass", "fail"] {
            assert!(valid(&with(status, &[("cause", legacy.clone())])));
            assert!(!valid(&with(status, &[("cause", reason.clone())])));
        }
        for status in ["not_measured", "not_applicable"] {
            let edits = [("cause", reason.clone()), ("weight", half.clone())];
            assert!(!valid(&with(status, &edits)), "{status} has no weight");
        }
        // A blocked weighs num 0; the schema does not compare num with den for a pass or a fail.
        let blocked = |weight: &serde_json::Value| {
            with(
                "blocked",
                &[("cause", reason.clone()), ("weight", weight.clone())],
            )
        };
        assert!(valid(&blocked(&json!({"num": 0, "den": 3}))));
        assert!(!valid(&blocked(&half)));
        assert!(valid(&with("pass", &[("weight", half.clone())])));
        assert!(valid(&with(
            "fail",
            &[("weight", json!({"num": 2, "den": 2}))]
        )));
        assert!(!valid(&with("pass", &[("weight", zero.clone())])));
        assert!(valid(&with(
            "pass",
            &[("weight", zero.clone()), ("cause", legacy)]
        )));
        assert!(!valid(&blocked(&zero)));
        assert!(!valid(&with("pass", &[("future", json!(1))])));
        assert!(!valid(&with("pass", &[("value", json!(null))])));
    }

    #[test]
    fn outcome_schema_states_the_rules_of_the_graded_roles() {
        let schema = validator();
        let probability = json!({"p": 0.5, "calibration_ref": "c"});
        let dirty = json!([instrument_json("i", "1.0.0", "sha256:", true)]);
        let clean = json!([instrument_json("i", "1.0.0", "sha256:", false)]);
        for role in ["gate", "quality", "descriptive"] {
            let mut record = base_json("pass");
            record["role"] = json!(role);
            assert!(schema.is_valid(&record));
            record["instruments"] = clean.clone();
            assert!(schema.is_valid(&record));
            let graded = role != "descriptive";
            let mut with_probability = record.clone();
            with_probability["probability"] = probability.clone();
            assert_eq!(schema.is_valid(&with_probability), !graded, "{role}");
            let mut with_dirty = record.clone();
            with_dirty["instruments"] = dirty.clone();
            assert_eq!(schema.is_valid(&with_dirty), !graded, "{role}");
        }
    }

    #[test]
    fn outcome_schema_marks_exactly_the_legacy_cause_deprecated() {
        let schema = schema_text();
        let mut deprecated = Vec::new();
        collect(&schema, "deprecated", &mut deprecated);
        assert_eq!(deprecated, [&json!(true)]);
        let legacy = &schema["$defs"]["Cause"]["oneOf"];
        let marked: Vec<&serde_json::Value> = legacy
            .as_array()
            .expect("the cause is a oneOf")
            .iter()
            .filter(|variant| variant["deprecated"] == json!(true))
            .collect();
        assert_eq!(marked.len(), 1);
        assert_eq!(
            marked.first().map(|variant| &variant["required"]),
            Some(&json!(["legacy"]))
        );
    }

    #[test]
    fn outcome_schema_names_what_it_cannot_state() {
        let schema = schema_text();
        let defs = &schema["$defs"];
        assert!(text(&defs["Weight"]["$comment"]).contains("numerator is at most the denominator"));
        assert!(text(&schema["$comment"]).contains("A pass has a weight with num equal to den"));
        let limit = "fits in 64 bits; the rust reader enforces it";
        let fits = |comment: &serde_json::Value| text(comment).to_lowercase().contains(limit);
        for comment in [
            &defs["Weight"]["properties"]["num"]["$comment"],
            &defs["Weight"]["properties"]["den"]["$comment"],
            &defs["Instrument"]["$comment"],
        ] {
            assert!(fits(comment), "{comment}");
        }
        let variants = defs["Value"]["oneOf"].as_array().expect("a oneOf");
        let find = |kind: &str| {
            variants
                .iter()
                .find(|variant| variant["properties"]["kind"]["const"] == json!(kind))
                .expect("a variant of that kind")
        };
        assert!(fits(&find("count")["properties"]["count"]["$comment"]));
        assert!(fits(&find("counts")["properties"]["counts"]["$comment"]));
        assert!(fits(&find("fraction")["properties"]["num"]["$comment"]));
        assert!(fits(&find("fraction")["properties"]["den"]["$comment"]));
    }

    #[test]
    fn outcome_the_schema_cannot_order_num_and_den_or_bound_an_integer_and_the_reader_does() {
        let schema = validator();
        let mut record = base_json("fail");
        record["weight"] = json!({"num": 1, "den": 2});
        assert!(schema.is_valid(&record) && read(record).is_ok());
        for (status, weight) in [
            ("pass", json!({"num": 1, "den": 2})),
            ("fail", json!({"num": 2, "den": 2})),
        ] {
            let mut record = base_json(status);
            record["weight"] = weight;
            assert!(schema.is_valid(&record), "the schema compares num with den");
            assert!(read(record).is_err());
        }
        let mut record = base_json("pass");
        record["value"] = json!({"kind": "count", "count": past_u64()});
        assert!(schema.is_valid(&record), "the schema bounds count");
        assert!(read(record).is_err());
    }

    #[test]
    fn outcome_schema_has_no_null_no_line_break_and_closes_every_object() {
        let schema = schema_text();
        let mut descriptions = Vec::new();
        collect(&schema, "description", &mut descriptions);
        assert!(descriptions.len() > 10);
        for description in descriptions {
            let text = text(description);
            assert!(!text.contains('\n'), "line break in {text:?}");
            assert!(!text.contains("fields below"), "{text:?}");
        }
        assert!(
            !serde_json::to_string(&schema)
                .expect("text")
                .contains("null")
        );
        let mut closed = Vec::new();
        collect(&schema, "additionalProperties", &mut closed);
        // Every object with fixed properties is closed; the one open map is `counts`.
        let falses = closed
            .iter()
            .filter(|value| ***value == json!(false))
            .count();
        assert_eq!(falses + 1, closed.len());
        assert!(falses >= 10, "only {falses} closed objects");
    }
}
