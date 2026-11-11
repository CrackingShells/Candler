//! The input-leak check: an output that equals an input by content.

use candler_schemas::execution::InvalidReason;

use super::{Evidence, Inputs};

/// The digest of an empty file.
const EMPTY: &str = "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Returns [`InvalidReason::InputLeak`] when an output has the same digest as an input. Only
/// digests are compared: a copied input is renamed, and a different file may share a size. The
/// digest of no content at all matches nothing: copying an empty file leaks nothing. It runs on
/// collected output only.
#[must_use]
pub fn input_leak(evidence: &Evidence, inputs: &Inputs) -> Option<InvalidReason> {
    let leaked = evidence.outputs.iter().any(|output| {
        inputs
            .digests
            .iter()
            .any(|input| input.digest == output.digest && output.digest.as_str() != EMPTY)
    });
    leaked.then_some(InvalidReason::InputLeak)
}
