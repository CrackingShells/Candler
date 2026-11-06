//! The real-Docker guards of M2b, ignored by default and run in the
//! `container-e2e` nextest group (`--run-ignored only`). Each e2e leaf fills
//! its own `#[path]` module, declared here by the S3 scaffold.

/// Holds the binary's place until the e2e leaves fill their modules.
///
/// It is ignored, so a default run lists it as skipped instead of hiding an
/// empty binary. Under `--run-ignored only` it panics, so the container gate
/// cannot pass before a leaf has replaced it with its own guards.
#[test]
#[ignore = "filled by container_run, container_stop, gateway and supervision_gates"]
fn placeholder_e2e_unfilled() {
    panic!("no e2e leaf has filled guard_container_e2e yet");
}
