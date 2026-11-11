//! Loading plug-in manifests.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use candler_schemas::evaluation::PluginManifest;
use candler_schemas::primitives::Name;

use super::RegistryError;

/// The file names a plug-in directory may hold its manifest under: `plugin.toml`, and
/// `candler-plugin.toml` for a plug-in whose directory is shared with other files.
pub const MANIFEST_NAMES: [&str; 2] = ["plugin.toml", "candler-plugin.toml"];

/// The manifest each directory of `dirs` holds, in the order of the directories. A directory is
/// searched at its top level only.
///
/// # Errors
/// Returns [`RegistryError::NoManifest`] for a directory that holds none of the
/// [`MANIFEST_NAMES`] and [`RegistryError::ManyManifests`] for one that holds more than one.
pub fn discover_manifests(dirs: &[PathBuf]) -> Result<Vec<PathBuf>, RegistryError> {
    let mut found = Vec::with_capacity(dirs.len());
    for dir in dirs {
        let mut held = MANIFEST_NAMES
            .iter()
            .map(|name| dir.join(name))
            .filter(|path| path.is_file());
        let (Some(first), second) = (held.next(), held.next()) else {
            // BORROW: .clone() keeps the directory in the error
            return Err(RegistryError::NoManifest { dir: dir.clone() });
        };
        if second.is_some() {
            // BORROW: .clone() keeps the directory in the error
            return Err(RegistryError::ManyManifests { dir: dir.clone() });
        }
        found.push(first);
    }
    Ok(found)
}

/// A plug-in manifest with the directory it was read from: `command` and the environment files
/// are relative to that directory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Plugin {
    /// The directory that holds the manifest.
    pub dir: PathBuf,
    /// The manifest.
    pub manifest: PluginManifest,
}

impl Plugin {
    /// A plug-in from its directory and manifest.
    #[must_use]
    pub const fn new(dir: PathBuf, manifest: PluginManifest) -> Self {
        Self { dir, manifest }
    }
}

/// The loaded plug-ins, by name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Registry {
    pub(super) plugins: BTreeMap<Name, Plugin>,
}

impl Registry {
    /// Reads each manifest in `manifests` and keeps it with the directory that holds it.
    ///
    /// # Errors
    /// Returns [`RegistryError::Read`] for a file that cannot be read, [`RegistryError::Manifest`]
    /// for one that is not a plug-in manifest and [`RegistryError::DuplicatePlugin`] for two that
    /// share a name.
    pub fn load(manifests: &[PathBuf]) -> Result<Self, RegistryError> {
        let mut plugins = BTreeMap::new();
        for path in manifests {
            let text = fs::read_to_string(path).map_err(|source| RegistryError::Read {
                // BORROW: .clone() keeps the path in the error
                path: path.clone(),
                source,
            })?;
            // BORROW: explicit .as_str() instead of Deref coercion
            let manifest = PluginManifest::from_toml_str(text.as_str()).map_err(|source| {
                RegistryError::Manifest {
                    // BORROW: .clone() keeps the path in the error
                    path: path.clone(),
                    source,
                }
            })?;
            let dir = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map_or_else(|| Path::new(".").to_owned(), Path::to_owned);
            // BORROW: .clone() makes the owned name the map is keyed by
            let name = manifest.name().clone();
            if plugins
                .insert(name.clone(), Plugin::new(dir, manifest))
                .is_some()
            {
                return Err(RegistryError::DuplicatePlugin { name });
            }
        }
        Ok(Self { plugins })
    }

    /// The loaded plug-ins, by name.
    #[must_use]
    pub const fn plugins<'a>(&'a self) -> &'a BTreeMap<Name, Plugin> {
        &self.plugins
    }

    /// The plug-in named `name`.
    #[must_use]
    pub fn plugin<'a>(&'a self, name: &Name) -> Option<&'a Plugin> {
        self.plugins.get(name)
    }
}
