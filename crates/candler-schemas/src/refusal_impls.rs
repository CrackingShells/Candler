//! The [`Refusal`](crate::refusal::Refusal) impls of this crate's errors, and the pointers of the
//! variants that carry one.
//!
//! Each table is one `refusal_table!` call: a variant without a row does not compile. The `when`
//! and `at` sentences are written for a coding agent, because the contract pages' tables of
//! refusals are generated from them. `docs/contract/codes.toml` registers the codes, and a test
//! checks the two against each other.

use crate::canonical::CanonicalError;
use crate::catalog::CatalogError;
use crate::evaluation::{EvaluationError, VersionMismatch};
use crate::execution::ExecutionError;
use crate::outcome::OutcomeError;
use crate::primitives::{FormatError, SchemaTagError};
use crate::refusal::{Refusal, Side, pointer_of};
use crate::refusal_table;
use crate::values::ValuesError;

/// The pointer of a variant that sits in a document: a read document, or the file's `inputs`, `replicates`
/// and `budget` fields. A field path of a resolved value is not a place in the file, so it has no pointer.
fn execution_path(error: &ExecutionError) -> Option<String> {
    match error {
        ExecutionError::Toml { path, .. }
        | ExecutionError::Document { path, .. }
        | ExecutionError::UnknownField { path, .. } => non_root(path.as_str()),
        ExecutionError::InputDeclaration { .. } => Some(pointer_of(["inputs"])),
        ExecutionError::Zero { field, .. } | ExecutionError::NotPositive { field } => {
            file_field(field)
        }
        ExecutionError::Format(inner) => Refusal::path(inner),
        ExecutionError::Values(inner) => Refusal::path(inner),
        ExecutionError::Canonical(inner) => Refusal::path(inner),
        ExecutionError::GitCommit { .. }
        | ExecutionError::Unrepresentable { .. }
        | ExecutionError::MissingField { .. }
        | ExecutionError::NoLevels { .. }
        | ExecutionError::EmptyTable { .. }
        | ExecutionError::PathOverlap { .. }
        | ExecutionError::Unsorted { .. }
        | ExecutionError::EmptyList { .. }
        | ExecutionError::Duplicate { .. }
        | ExecutionError::ReservedRoot { .. }
        | ExecutionError::ReservedValue { .. }
        | ExecutionError::AbsolutePath { .. }
        | ExecutionError::SourcesMismatch { .. }
        | ExecutionError::KnownKind { .. }
        | ExecutionError::PathUnderFile { .. }
        | ExecutionError::DirNotEmpty { .. } => None,
    }
}

/// The pointer of a field the experiment file holds, written as a dotted path: `replicates` or a
/// field under `budget`. Any other name, such as `pid`, is not a field of the file.
fn file_field(field: &str) -> Option<String> {
    match field.split('.').next() {
        Some("replicates" | "budget") => Some(pointer_of(field.split('.'))),
        _ => None,
    }
}

refusal_table! {
    ExecutionError, path = execution_path;
    GitCommit { .. } => row(
        "manifest.git_commit_invalid",
        Side::Candler,
        "A git commit text is not 40 or 64 lowercase hex digits.",
        "-",
    ),
    Format(inner) => delegate(inner, Side::Experiment),
    Values(inner) => delegate(inner, Side::Experiment),
    Canonical(inner) => delegate(inner, Side::Experiment),
    Toml { .. } => row(
        "experiment.toml_invalid",
        Side::Experiment,
        "The experiment file is not valid TOML.",
        "-",
    ),
    Unrepresentable { .. } => row(
        "experiment.value_unrepresentable",
        Side::Experiment,
        "A TOML value is a date, a time or a number that JSON cannot state.",
        "-",
    ),
    Document { .. } => row(
        "experiment.document_invalid",
        Side::Experiment,
        "The TOML is not an experiment file: a field is missing, has the wrong type or breaks the format of its own.",
        "The field at fault, or its parent table when a field is missing; none at the root.",
    ),
    UnknownField { .. } => row(
        "experiment.unknown_field",
        Side::Experiment,
        "The experiment file holds a key its type does not have, such as a misspelt option.",
        "The unknown key itself.",
    ),
    MissingField { .. } => row(
        "manifest.field_unset",
        Side::Candler,
        "A builder of a run record was asked to build before a required field was set.",
        "-",
    ),
    Zero { .. } => row(
        "experiment.count_zero",
        Side::Experiment,
        "A count, duration or process id that must be at least 1 is 0.",
        "The field when it is `replicates` or one under `budget`; none otherwise.",
    ),
    NotPositive { .. } => row(
        "experiment.cost_not_positive",
        Side::Experiment,
        "A cost cap is not a finite number above 0.",
        "-",
    ),
    NoLevels { .. } => row(
        "experiment.factor_without_levels",
        Side::Experiment,
        "A factor has no level, so the experiment would have no cell.",
        "-",
    ),
    EmptyTable { .. } => row(
        "experiment.table_empty",
        Side::Experiment,
        "A table among the fixed values or the levels holds no value, so it fills no path.",
        "-",
    ),
    PathOverlap { .. } => row(
        "experiment.path_overlap",
        Side::Experiment,
        "Two paths set by the fixed values and the factors are the same, or one lies below the other.",
        "-",
    ),
    Unsorted { .. } => row(
        "manifest.list_unsorted",
        Side::Candler,
        "A list of a run record is not in the order its owner requires.",
        "-",
    ),
    EmptyList { .. } => row(
        "manifest.list_empty",
        Side::Candler,
        "A list of a run record that needs an entry holds none.",
        "-",
    ),
    InputDeclaration { .. } => row(
        "experiment.input_invalid",
        Side::Experiment,
        "An input declaration uses the kind the wire reserves, or mounts under an empty, absolute or escaping directory.",
        "`/inputs`, the list that holds the declaration.",
    ),
    Duplicate { .. } => row(
        "experiment.entry_repeated",
        Side::Experiment,
        "A list holds the same entry twice: a fixed path, a factor path or an input kind.",
        "-",
    ),
    ReservedRoot { .. } => row(
        "experiment.reserved_root_misused",
        Side::Experiment,
        "A value at a reserved root is not the root alone, or has no value.",
        "-",
    ),
    ReservedValue { .. } => row(
        "experiment.reserved_value_malformed",
        Side::Experiment,
        "The value of a reserved root is not the shape the root takes.",
        "-",
    ),
    AbsolutePath { .. } => row(
        "experiment.absolute_path",
        Side::Experiment,
        "A value holds an absolute path as a string, and a file enters a cell as an input digest.",
        "-",
    ),
    SourcesMismatch { .. } => row(
        "manifest.sources_mismatch",
        Side::Candler,
        "A sources list of a run record does not cover the values and typed roots, or names a path that is neither.",
        "-",
    ),
    KnownKind { .. } => row(
        "manifest.event_kind_known",
        Side::Candler,
        "An event of a kind this version knows was built as an unknown one.",
        "-",
    ),
    PathUnderFile { .. } => row(
        "manifest.tree_entry_under_file",
        Side::Candler,
        "A tree entry lies below a path that holds a file.",
        "-",
    ),
    DirNotEmpty { .. } => row(
        "manifest.tree_dir_not_empty",
        Side::Candler,
        "A tree directory listed as empty has an entry below it.",
        "-",
    ),
}

/// The pointer of a read document; a wrapped error gives its own.
fn evaluation_path(error: &EvaluationError) -> Option<String> {
    match error {
        EvaluationError::Toml { path, .. }
        | EvaluationError::ManifestToml { path, .. }
        | EvaluationError::SuiteUnknownField { path, .. }
        | EvaluationError::ManifestUnknownField { path, .. }
        | EvaluationError::Json { path, .. } => non_root(path.as_str()),
        EvaluationError::Canonical(inner) => Refusal::path(inner),
        EvaluationError::MissingField { .. }
        | EvaluationError::ManifestCommandEmpty
        | EvaluationError::EmptyList { .. }
        | EvaluationError::DuplicateId { .. }
        | EvaluationError::UnknownDependency { .. }
        | EvaluationError::Cycle { .. }
        | EvaluationError::LegacyCause { .. } => None,
    }
}

refusal_table! {
    EvaluationError, path = evaluation_path;
    Toml { .. } => row(
        "suite.document_invalid",
        Side::Suite,
        "A suite is not valid TOML, or does not have the shape of a suite.",
        "The field at fault, or its parent table when a field is missing; none at the root.",
    ),
    ManifestToml { .. } => row(
        "plugin.manifest_invalid",
        Side::Plugin,
        "A plug-in manifest is not valid TOML, or does not have the shape of a manifest.",
        "The field at fault, or its parent table when a field is missing; none at the root.",
    ),
    SuiteUnknownField { .. } => row(
        "suite.unknown_field",
        Side::Suite,
        "The suite holds a key its type does not have, such as a misspelt option.",
        "The unknown key itself.",
    ),
    ManifestUnknownField { .. } => row(
        "plugin.unknown_field",
        Side::Plugin,
        "The plug-in manifest holds a key its type does not have, such as a misspelt option.",
        "The unknown key itself.",
    ),
    Json { .. } => row(
        "suite.json_invalid",
        Side::Suite,
        "A suite is not valid JSON, or does not have the shape of a suite.",
        "The field at fault, or its parent object when a field is missing; none at the root.",
    ),
    MissingField { .. } => row(
        "protocol.field_unset",
        Side::Candler,
        "A builder of an evaluator protocol record was asked to build before a required field was set.",
        "-",
    ),
    ManifestCommandEmpty { .. } => row(
        "plugin.command_empty",
        Side::Plugin,
        "The `command` of a plug-in manifest has no member, so there is nothing to start.",
        "-",
    ),
    EmptyList { .. } => row(
        "suite.list_empty",
        Side::Suite,
        "A list of a suite entry that needs at least one member is empty, such as `any_of` or `applies_to`.",
        "-",
    ),
    DuplicateId { .. } => row(
        "suite.id_repeated",
        Side::Suite,
        "Two expectations of a suite share an id.",
        "-",
    ),
    UnknownDependency { .. } => row(
        "suite.dependency_unknown",
        Side::Suite,
        "An expectation depends on an id that no expectation has.",
        "-",
    ),
    Cycle { .. } => row(
        "suite.dependency_cycle",
        Side::Suite,
        "Expectations depend on each other in a loop.",
        "-",
    ),
    LegacyCause { .. } => row(
        "protocol.outcome_legacy_cause",
        Side::Plugin,
        "An outcome notification carries the legacy cause, which evaluators never emit.",
        "-",
    ),
    Canonical(inner) => delegate(inner, Side::Suite),
}

/// A wrapped error gives its own pointer; the outcome's own rules name no place.
fn outcome_path(error: &OutcomeError) -> Option<String> {
    match error {
        OutcomeError::Format(inner) => Refusal::path(inner),
        OutcomeError::MissingField { .. }
        | OutcomeError::CauseMissing { .. }
        | OutcomeError::CauseNotAllowed { .. }
        | OutcomeError::WeightNotAllowed { .. }
        | OutcomeError::WeightMismatch { .. }
        | OutcomeError::DirtyInstrument { .. }
        | OutcomeError::ProbabilityNotAllowed { .. }
        | OutcomeError::ZeroWeightWithoutLegacy { .. }
        | OutcomeError::WeightAboveOne { .. }
        | OutcomeError::FractionDenominator { .. }
        | OutcomeError::NumberNotFinite { .. }
        | OutcomeError::InstrumentDirty { .. }
        | OutcomeError::EvidenceEmpty
        | OutcomeError::TextEmpty { .. }
        | OutcomeError::ProbabilityRange { .. } => None,
    }
}

refusal_table! {
    OutcomeError, path = outcome_path;
    MissingField { .. } => row(
        "outcome.field_unset",
        Side::Candler,
        "A builder of an outcome record was asked to build before a required field was set.",
        "-",
    ),
    CauseMissing { .. } => row(
        "outcome.cause_missing",
        Side::Plugin,
        "A blocked, not-measured or not-applicable outcome carries no cause.",
        "-",
    ),
    CauseNotAllowed { .. } => row(
        "outcome.cause_not_allowed",
        Side::Plugin,
        "A pass or fail outcome carries a cause other than the legacy one.",
        "-",
    ),
    WeightNotAllowed { .. } => row(
        "outcome.weight_not_allowed",
        Side::Plugin,
        "A not-measured or not-applicable outcome carries a weight.",
        "-",
    ),
    WeightMismatch { .. } => row(
        "outcome.weight_mismatch",
        Side::Plugin,
        "The weight of an outcome does not fit its status.",
        "-",
    ),
    DirtyInstrument { .. } => row(
        "outcome.instrument_dirty",
        Side::Plugin,
        "A gate or quality outcome names an instrument that is not clean.",
        "-",
    ),
    ProbabilityNotAllowed { .. } => row(
        "outcome.probability_not_allowed",
        Side::Plugin,
        "A gate or quality outcome carries a probability.",
        "-",
    ),
    ZeroWeightWithoutLegacy { .. } => row(
        "outcome.zero_weight_without_legacy",
        Side::Plugin,
        "A weight of 0/0 comes without the legacy cause.",
        "-",
    ),
    WeightAboveOne { .. } => row(
        "outcome.weight_above_one",
        Side::Plugin,
        "The numerator of a weight exceeds its denominator.",
        "-",
    ),
    FractionDenominator { .. } => row(
        "outcome.fraction_denominator_zero",
        Side::Plugin,
        "The denominator of a fraction is 0.",
        "-",
    ),
    NumberNotFinite { .. } => row(
        "outcome.number_not_finite",
        Side::Candler,
        "A number of an outcome record is not finite.",
        "-",
    ),
    InstrumentDirty { .. } => row(
        "outcome.instrument_digest_dirty",
        Side::Plugin,
        "An instrument digest names a commit of a dirty tree.",
        "-",
    ),
    EvidenceEmpty { .. } => row(
        "outcome.evidence_empty",
        Side::Plugin,
        "A piece of evidence has neither a reference nor a note.",
        "-",
    ),
    TextEmpty { .. } => row(
        "outcome.text_empty",
        Side::Plugin,
        "A `note` of a piece of evidence, or a `calibration_ref` of a probability, has no character.",
        "-",
    ),
    ProbabilityRange { .. } => row(
        "outcome.probability_out_of_range",
        Side::Plugin,
        "A probability lies outside 0 to 1.",
        "-",
    ),
    Format(inner) => delegate(inner, Side::Plugin),
}

/// A format error carries the text, not the place it was read from.
fn format_path(_error: &FormatError) -> Option<String> {
    None
}

refusal_table! {
    FormatError, path = format_path;
    Name { .. } => row(
        "format.name_invalid",
        Side::Config,
        "A name holds a character a name may not hold, or is empty.",
        "-",
    ),
    NameVersion { .. } => row(
        "format.name_version_invalid",
        Side::Config,
        "A `name@version` text does not hold a name and a SemVer version.",
        "-",
    ),
    SecretName { .. } => row(
        "format.secret_name_invalid",
        Side::Config,
        "A secret name does not fit the format of a secret name.",
        "-",
    ),
    FieldPath { .. } => row(
        "format.field_path_invalid",
        Side::Config,
        "A field path is not segments of lowercase letters, digits and underscores joined by dots.",
        "-",
    ),
    RelPath { .. } => row(
        "format.rel_path_invalid",
        Side::Config,
        "A relative path is empty, absolute, or holds a `.` or `..` segment.",
        "-",
    ),
    ExpectationId { .. } => row(
        "format.expectation_id_invalid",
        Side::Config,
        "An expectation id does not fit the format of an id.",
        "-",
    ),
    ReasonCode { .. } => row(
        "format.reason_code_invalid",
        Side::Config,
        "A reason code does not fit the format of a reason code.",
        "-",
    ),
    EvidenceRef { .. } => row(
        "format.evidence_ref_invalid",
        Side::Config,
        "An evidence reference does not fit the format of a reference.",
        "-",
    ),
    Marker { .. } => row(
        "format.marker_invalid",
        Side::Config,
        "A marker does not fit the format of a marker.",
        "-",
    ),
    Digest { .. } => row(
        "format.digest_invalid",
        Side::Candler,
        "A digest is not `sha256:` and 64 lowercase hex digits.",
        "-",
    ),
    InstrumentDigest { .. } => row(
        "format.instrument_digest_invalid",
        Side::Candler,
        "An instrument digest does not fit its format.",
        "-",
    ),
    NameToken { .. } => row(
        "format.name_token_invalid",
        Side::Candler,
        "A `name@token` label does not fit its format.",
        "-",
    ),
    Timestamp { .. } => row(
        "format.timestamp_invalid",
        Side::Candler,
        "A timestamp does not fit the wire format.",
        "-",
    ),
    TimestampInstant { .. } => row(
        "format.timestamp_instant_invalid",
        Side::Candler,
        "A timestamp names no instant.",
        "-",
    ),
    RunKey { .. } => row(
        "format.run_key_invalid",
        Side::Candler,
        "A run key does not fit its format.",
        "-",
    ),
    RequestId { .. } => row(
        "format.request_id_invalid",
        Side::Candler,
        "A request id is not a lowercase hyphenated UUIDv7.",
        "-",
    ),
    JobId { .. } => row(
        "format.job_id_invalid",
        Side::Candler,
        "A job id does not fit its format.",
        "-",
    ),
    BootId { .. } => row(
        "format.boot_id_invalid",
        Side::Candler,
        "A boot id does not fit its format.",
        "-",
    ),
    ContainerId { .. } => row(
        "format.container_id_invalid",
        Side::Candler,
        "A container id does not fit its format.",
        "-",
    ),
}

/// A canonical-form error carries no place.
fn canonical_path(_error: &CanonicalError) -> Option<String> {
    None
}

refusal_table! {
    CanonicalError, path = canonical_path;
    IntegerRange { .. } => row(
        "canonical.integer_out_of_range",
        Side::Experiment,
        "An integer lies outside the range JSON numbers hold exactly, plus or minus 2^53 - 1.",
        "-",
    ),
    NotFinite { .. } => row(
        "canonical.number_not_finite",
        Side::Experiment,
        "A number is not finite.",
        "-",
    ),
    LoneSurrogate { .. } => row(
        "canonical.lone_surrogate",
        Side::Experiment,
        "A JSON text holds an unpaired surrogate escape.",
        "-",
    ),
    DuplicateKey { .. } => row(
        "canonical.key_repeated",
        Side::Experiment,
        "A JSON object repeats a key.",
        "-",
    ),
    Syntax { .. } => row(
        "canonical.syntax_invalid",
        Side::Experiment,
        "A text is not valid JSON.",
        "-",
    ),
}

/// A wrapped error gives its own pointer.
fn values_path(error: &ValuesError) -> Option<String> {
    match error {
        ValuesError::Canonical(inner) => Refusal::path(inner),
        ValuesError::Null => None,
    }
}

refusal_table! {
    ValuesError, path = values_path;
    Null { .. } => row(
        "values.null",
        Side::Config,
        "A setting is null, where an absent value is omitted.",
        "-",
    ),
    Canonical(inner) => delegate(inner, Side::Config),
}

/// The wrong tag sits in the `schema` field.
#[expect(
    clippy::unnecessary_wraps,
    reason = "the table takes one signature for every type, and most have no pointer"
)]
fn schema_tag_path(_error: &SchemaTagError) -> Option<String> {
    Some(pointer_of(["schema"]))
}

refusal_table!(struct SchemaTagError, path = schema_tag_path; row("format.schema_tag_mismatch", Side::Config, "The `schema` field of a document holds another version marker than its type's.", "`/schema`"));

/// A version mismatch is between two programs, not in a document.
fn version_path(_error: &VersionMismatch) -> Option<String> {
    None
}

refusal_table!(struct VersionMismatch, path = version_path; row("protocol.version_mismatch", Side::Plugin, "The protocol version of the evaluator is not accepted by the core's, or the reverse.", "-"));

/// The pointer of a read document, which is empty at the root and for a text that is no document: the root names no field.
fn non_root(path: &str) -> Option<String> {
    if path.is_empty() {
        None
    } else {
        Some(path.to_owned())
    }
}

/// A wrong tag sits in the `schema` field; the other faults name no place.
fn catalog_path(error: &CatalogError) -> Option<String> {
    match error {
        CatalogError::SchemaTag { .. } => Some(pointer_of(["schema"])),
        CatalogError::MissingField { .. }
        | CatalogError::ReservedKey { .. }
        | CatalogError::Null
        | CatalogError::EmptyName { .. }
        | CatalogError::DuplicateName { .. } => None,
    }
}

refusal_table! {
    CatalogError, path = catalog_path;
    SchemaTag { .. } => row(
        "catalog.schema_tag_mismatch",
        Side::Environment,
        "The tool catalog an image prints carries another `schema` marker than `tool-catalog-snapshot/1`.",
        "`/schema`",
    ),
    MissingField { .. } => row(
        "catalog.field_unset",
        Side::Candler,
        "A builder of a catalog snapshot was asked to build before a required field was set.",
        "-",
    ),
    ReservedKey { .. } => row(
        "catalog.extra_key_reserved",
        Side::Candler,
        "An extra key of a tool is the name of one of the tool's fields.",
        "-",
    ),
    Null { .. } => row(
        "catalog.field_null",
        Side::Candler,
        "An optional field of a catalog was written as null instead of being omitted.",
        "-",
    ),
    EmptyName { .. } => row(
        "catalog.name_empty",
        Side::Environment,
        "A server or tool name in the catalog an image prints is empty.",
        "-",
    ),
    DuplicateName { .. } => row(
        "catalog.name_repeated",
        Side::Environment,
        "A server or tool name in the catalog an image prints is repeated within its scope.",
        "-",
    ),
}
