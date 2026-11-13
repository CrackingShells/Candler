//! The harness drivers this binary ships, selected by the name an experiment's
//! `request.harness` gives.
//!
//! This is the one place the binary names a driver crate: everything else asks the registry for
//! the driver of a harness by name and asks the driver for the rest.

use std::collections::BTreeMap;
use std::str::FromStr as _;
use std::sync::{Arc, LazyLock};

use candler_core::import::raw::Recognised;
use candler_harness::driver::HarnessDriver;
use candler_harness::reader::TraceReader;
use candler_harness_claude_code::driver::ClaudeCodeDriver;
use candler_harness_opencode::driver::OpenCodeDriver;
use candler_schemas::primitives::Name;
use candler_schemas::raw::StreamName;

use crate::commands::CliError;
use crate::commands::refusal::CliRefusal;

/// The harnesses this binary ships, by the name of their driver.
pub struct Registry {
    drivers: BTreeMap<String, Arc<dyn HarnessDriver>>,
}

/// The registry of the shipped harnesses, built on first use.
static BUILTIN: LazyLock<Registry> = LazyLock::new(|| {
    // TRAIT_OBJECT: the driver is chosen by the harness's name at runtime
    let shipped: [Arc<dyn HarnessDriver>; 2] = [
        Arc::new(ClaudeCodeDriver::new()),
        Arc::new(OpenCodeDriver::new()),
    ];
    Registry {
        drivers: shipped
            .into_iter()
            .map(|driver| (driver.id().to_string(), driver))
            .collect(),
    }
});

impl Registry {
    /// The registry of the harnesses this binary ships.
    #[must_use]
    pub fn builtin() -> &'static Self {
        &BUILTIN
    }

    /// The names of the registered harnesses, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.drivers.keys().cloned().collect() // BORROW: the names are owned by the caller
    }

    /// A new reader of the live output of the harness `name`, or `None` when no harness has that
    /// name or its driver reads none.
    // TRAIT_OBJECT: the reader is the driver's, chosen by the harness's name at runtime
    #[must_use]
    pub fn reader(&self, name: &str) -> Option<Box<dyn TraceReader>> {
        self.drivers.get(name)?.live_reader()
    }

    /// The streams the harness `name` collects after a run, each with the offline reader its
    /// driver gives it. A harness that is not registered, or that collects none, gives none.
    // TRAIT_OBJECT: the readers are the drivers', chosen by the harness's name at runtime
    #[must_use]
    pub fn offline(&self, name: &str) -> Vec<(StreamName, Box<dyn TraceReader>)> {
        let Some(driver) = self.drivers.get(name) else {
            return Vec::new();
        };
        driver
            .offline_streams()
            .into_iter()
            .filter_map(|stream| {
                let reader = driver.offline_reader(&stream)?;
                Some((stream, reader))
            })
            .collect()
    }

    /// What the first harness that recognises `sample`, the start of a raw stream, says of it. The
    /// harness is named as its driver names itself, and a version is supported when the driver's
    /// requirement matches it.
    #[must_use]
    pub fn recognise(&self, sample: &[u8]) -> Option<Recognised> {
        self.drivers.iter().find_map(|(name, driver)| {
            let version = driver.detect(sample)?;
            let supported = driver.supported().matches(&version);
            Some(Recognised::new(
                Name::from_str(name).ok()?,
                version,
                supported,
            ))
        })
    }

    /// The driver of the harness `name`.
    ///
    /// # Errors
    /// [`CliError::Coded`] carrying [`CliRefusal::UnknownHarness`], naming the harness and the
    /// registered ones, when no harness has that name.
    pub fn driver(&self, name: &str) -> Result<Arc<dyn HarnessDriver>, CliError> {
        self.drivers.get(name).cloned().ok_or_else(|| {
            CliError::refused_by(CliRefusal::UnknownHarness {
                name: name.to_owned(), // BORROW: the refusal owns the name
                registered: self.names().join(", "),
            })
        })
    }
}
