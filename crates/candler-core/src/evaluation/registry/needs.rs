//! The needs of a kind that the core can meet.

use candler_schemas::evaluation::Needs;
use globset::Glob;

/// The prefix every artifact glob has: artifacts are the run's stored `outputs/` files.
const OUTPUTS_PREFIX: &str = "outputs/";

/// Why a need cannot be met.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum NeedError {
    /// A need this version of the core does not support.
    #[error("unsupported in this version: {need}")]
    Unsupported {
        /// The need: `snapshots` or `inputs`.
        need: &'static str,
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
/// can; artifact globs must lie under `outputs/`; snapshots and inputs are refused.
pub(super) fn check(needs: &Needs) -> Result<(), NeedError> {
    if !needs.snapshots.is_empty() {
        return Err(NeedError::Unsupported { need: "snapshots" });
    }
    if !needs.inputs.is_empty() {
        return Err(NeedError::Unsupported { need: "inputs" });
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
