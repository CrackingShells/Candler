//! Helpers the tests of this crate share, compiled once into the `it` binary.

pub mod cli_run;
pub mod contract_pages;
pub mod seam;

/// The shared test-image support of the core's tests: the same file, so the two crates find and
/// build their images one way.
#[path = "../../../../candler-core/tests/it/support/test_image.rs"]
pub mod test_image;
