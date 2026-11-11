//! The needs of a kind that the core can meet.

use candler_schemas::evaluation::Needs;
use globset::Glob;

/// The prefix every artifact glob has: artifacts are the run's stored `outputs/` files.
const OUTPUTS_PREFIX: &str = "outputs/";

/// The kind of the input list that the image step derives.
const RESERVED_INPUT_KIND: &str = "build_inputs";

/// Why a need cannot be met.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NeedError {
    /// A need this version of the core does not support.
    #[error("unsupported in this version: {need}")]
    Unsupported {
        /// The need: `snapshots`.
        need: &'static str,
    },
    /// An input kind the image reserves for its own list.
    #[error("input kind `{kind}` is reserved for the image and cannot be needed")]
    ReservedInput {
        /// The kind.
        kind: String,
    },
    /// An artifact glob that does not lie under `outputs/`.
    #[error("artifact glob `{glob}` is not under `outputs/`")]
    OutsideOutputs {
        /// The glob.
        glob: String,
    },
    /// An artifact glob that does not compile.
    #[error("artifact glob `{glob}` is not a valid glob: {detail}")]
    BadGlob {
        /// The glob.
        glob: String,
        /// What the glob compiler reported.
        detail: String,
    },
}

/// Checks that the core can meet every need in `needs`: the trace and the metadata fields always
/// can; artifact globs must lie under `outputs/`; snapshots are refused and an input need may not name `build_inputs`; whether the run holds
/// the other kinds is checked when a run's context is assembled.
pub(super) fn check(needs: &Needs) -> Result<(), NeedError> {
    if !needs.snapshots.is_empty() {
        return Err(NeedError::Unsupported { need: "snapshots" });
    }
    if let Some(kind) = needs
        .inputs
        .iter()
        .find(|kind| kind.as_str() == RESERVED_INPUT_KIND)
    {
        return Err(NeedError::ReservedInput {
            kind: kind.to_string(),
        });
    }
    for glob in &needs.artifacts {
        // BORROW: explicit .as_str() instead of Deref coercion
        let text = glob.as_str();
        if !text.starts_with(OUTPUTS_PREFIX) || text.split('/').any(|segment| segment == "..") {
            return Err(NeedError::OutsideOutputs {
                // BORROW: .clone() keeps the glob in the error
                glob: glob.clone(),
            });
        }
        if let Err(error) = Glob::new(text) {
            return Err(NeedError::BadGlob {
                // BORROW: .clone() keeps the glob in the error
                glob: glob.clone(),
                detail: error.to_string(),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use candler_schemas::evaluation::Needs;

    use super::{NeedError, check};

    /// Guards: R01 §Evaluator protocol — an evaluator cannot need the image's own list `build_inputs`, and may need any other kind. Oracle: hand-written; the kinds are the ones below.
    #[test]
    fn needs_inputs_refused_for_the_reserved_kind() {
        let mut needs = Needs::default();
        needs.inputs = vec!["truth".parse().expect("a name")];
        assert_eq!(check(&needs), Ok(()));
        needs.inputs = vec![
            "truth".parse().expect("a name"),
            "build_inputs".parse().expect("a name"),
        ];
        assert_eq!(
            check(&needs),
            Err(NeedError::ReservedInput {
                kind: "build_inputs".into()
            })
        );
    }
}
