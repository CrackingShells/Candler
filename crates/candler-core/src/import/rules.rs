//! The caller-supplied rules that name a legacy folder's domain.

use candler_schemas::primitives::{Digest, FieldPath, Name, NameToken, ReasonCode};
use regex::Regex;

use super::ImportError;

/// Where the harness name and version of a run come from: the name the importer writes, and the JSON pointer to the version in the first line of the run's stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HarnessRule {
    /// The harness name written into every cell.
    pub name: Name,
    /// The JSON pointer to the harness version in the first line of a run's stream.
    pub version_pointer: String,
}

impl HarnessRule {
    /// A harness rule.
    #[must_use]
    pub const fn new(name: Name, version_pointer: String) -> Self {
        Self {
            name,
            version_pointer,
        }
    }
}

/// Where the treatment of a cell comes from: the JSON pointer into `experiment.json` to a value written `<name>@<tag>`, and the top-level key of `experiment.json` that holds the version the treatment resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct TreatmentRule {
    /// The JSON pointer into `experiment.json`.
    pub pointer: String,
    /// The top-level key of `experiment.json` that holds the resolved version; it is also the path of that value among the cell's values.
    pub resolved: FieldPath,
}

impl TreatmentRule {
    /// A treatment rule.
    #[must_use]
    pub const fn new(pointer: String, resolved: FieldPath) -> Self {
        Self { pointer, resolved }
    }
}

/// One snapshot of a cell: its kind, and the JSON pointer into the folder's provenance file to its sha256.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SnapshotRule {
    /// The kind written into the cell's snapshot list.
    pub kind: Name,
    /// The JSON pointer into the provenance file to a bare 64-digit sha256.
    pub pointer: String,
}

impl SnapshotRule {
    /// A snapshot rule.
    #[must_use]
    pub const fn new(kind: Name, pointer: String) -> Self {
        Self { kind, pointer }
    }
}

/// The reason code a marker rule gives: a fixed code, or the text of a named group of the pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MarkerCode {
    /// The same code whenever the pattern matches.
    Fixed(ReasonCode),
    /// The text that the named group of the pattern captured.
    Capture(String),
}

/// A pattern tried on the evidence of a `not_measured` row, and the reason code it gives.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MarkerRule {
    /// The pattern, searched anywhere in the evidence text.
    pub pattern: Regex,
    /// The code a match gives.
    pub code: MarkerCode,
}

impl MarkerRule {
    /// A marker rule.
    #[must_use]
    pub const fn new(pattern: Regex, code: MarkerCode) -> Self {
        Self { pattern, code }
    }
}

/// The rules that name a legacy folder's domain: every string the importer writes that belongs to the legacy format comes from here, so the core names no domain.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ImportRules {
    /// The label of the legacy format, written into every import record.
    pub format: NameToken,
    /// Where the harness name and version come from.
    pub harness: HarnessRule,
    /// Where the treatment comes from.
    pub treatment: TreatmentRule,
    /// The name of the instrument that graded the folder, whose version and digest come from the folder's pinned evaluator.
    pub instrument: Name,
    /// The snapshots of a cell.
    pub snapshots: Vec<SnapshotRule>,
    /// The patterns tried, in order, on the evidence of a `not_measured` row.
    pub markers: Vec<MarkerRule>,
    /// The kind of every suite entry the importer synthesises.
    pub suite_kind: Name,
    /// The manifest values copied into each run context's metadata.
    pub metadata: Vec<FieldPath>,
    /// The sha256 of the rules file these rules were read from, written into every import record; absent when the rules were not read from a file.
    pub source_digest: Option<Digest>,
}

impl ImportRules {
    /// Starts a builder; the format, the harness, the treatment, the instrument and the suite kind must be set before it builds.
    #[must_use]
    pub fn builder() -> ImportRulesBuilder {
        ImportRulesBuilder::default()
    }
}

/// Builds [`ImportRules`].
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ImportRulesBuilder {
    format: Option<NameToken>,
    harness: Option<HarnessRule>,
    treatment: Option<TreatmentRule>,
    instrument: Option<Name>,
    snapshots: Vec<SnapshotRule>,
    markers: Vec<MarkerRule>,
    suite_kind: Option<Name>,
    metadata: Vec<FieldPath>,
    source_digest: Option<Digest>,
}

impl ImportRulesBuilder {
    /// Sets the label of the legacy format.
    #[must_use]
    pub fn format(mut self, format: NameToken) -> Self {
        self.format = Some(format);
        self
    }

    /// Sets where the harness name and version come from.
    #[must_use]
    pub fn harness(mut self, harness: HarnessRule) -> Self {
        self.harness = Some(harness);
        self
    }

    /// Sets where the treatment comes from.
    #[must_use]
    pub fn treatment(mut self, treatment: TreatmentRule) -> Self {
        self.treatment = Some(treatment);
        self
    }

    /// Sets the name of the grading instrument.
    #[must_use]
    pub fn instrument(mut self, instrument: Name) -> Self {
        self.instrument = Some(instrument);
        self
    }

    /// Adds a snapshot rule.
    #[must_use]
    pub fn snapshot(mut self, snapshot: SnapshotRule) -> Self {
        self.snapshots.push(snapshot);
        self
    }

    /// Adds a marker rule after those already added.
    #[must_use]
    pub fn marker(mut self, marker: MarkerRule) -> Self {
        self.markers.push(marker);
        self
    }

    /// Sets the kind of every synthesised suite entry.
    #[must_use]
    pub fn suite_kind(mut self, suite_kind: Name) -> Self {
        self.suite_kind = Some(suite_kind);
        self
    }

    /// Adds a manifest value path to the run context metadata.
    #[must_use]
    pub fn metadata(mut self, path: FieldPath) -> Self {
        self.metadata.push(path);
        self
    }

    /// Sets the sha256 of the rules file the rules were read from.
    #[must_use]
    pub fn source_digest(mut self, digest: Digest) -> Self {
        self.source_digest = Some(digest);
        self
    }

    /// Builds the rules.
    ///
    /// # Errors
    /// Returns [`ImportError::MissingField`] if the format, the harness, the treatment, the
    /// instrument or the suite kind was not set, and [`ImportError::Rules`] if a pointer is not a
    /// JSON pointer or a marker captures a group its pattern lacks.
    pub fn build(self) -> Result<ImportRules, ImportError> {
        let missing = |field| ImportError::MissingField { field };
        let rules = ImportRules {
            format: self.format.ok_or_else(|| missing("format"))?,
            harness: self.harness.ok_or_else(|| missing("harness"))?,
            treatment: self.treatment.ok_or_else(|| missing("treatment"))?,
            instrument: self.instrument.ok_or_else(|| missing("instrument"))?,
            snapshots: self.snapshots,
            markers: self.markers,
            suite_kind: self.suite_kind.ok_or_else(|| missing("suite_kind"))?,
            metadata: self.metadata,
            source_digest: self.source_digest,
        };
        rules.check()?;
        Ok(rules)
    }
}

/// Reports whether `pointer` is a JSON pointer: empty, or starting with `/`.
fn is_pointer(pointer: &str) -> bool {
    pointer.is_empty() || pointer.starts_with('/')
}

impl ImportRules {
    /// Checks every pointer and every captured group.
    fn check(&self) -> Result<(), ImportError> {
        let pointers = std::iter::once(("harness.version_pointer", &self.harness.version_pointer))
            .chain(std::iter::once((
                "treatment.pointer",
                &self.treatment.pointer,
            )))
            .chain(
                self.snapshots
                    .iter()
                    .map(|snapshot| ("snapshots.pointer", &snapshot.pointer)),
            );
        for (field, pointer) in pointers {
            // BORROW: explicit .as_str() instead of Deref coercion
            if !is_pointer(pointer.as_str()) {
                return Err(ImportError::Rules {
                    detail: format!("`{field}` is not a JSON pointer (got {pointer:?})"),
                });
            }
        }
        // BORROW: explicit .as_str() instead of Deref coercion
        let resolved = self.treatment.resolved.as_str();
        if resolved.contains('.') {
            return Err(ImportError::Rules {
                detail: format!("`treatment.resolved` is not a top-level key (got {resolved:?})"),
            });
        }
        for marker in &self.markers {
            if let MarkerCode::Capture(group) = &marker.code
                && !marker
                    .pattern
                    .capture_names()
                    // BORROW: explicit .as_str() instead of Deref coercion
                    .any(|name| name == Some(group.as_str()))
            {
                // BORROW: Regex::as_str gives the pattern text
                let pattern = marker.pattern.as_str();
                return Err(ImportError::Rules {
                    detail: format!("marker pattern {pattern:?} has no group named `{group}`"),
                });
            }
        }
        Ok(())
    }
}
