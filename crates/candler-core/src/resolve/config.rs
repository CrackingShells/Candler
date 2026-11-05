//! The project configuration (`candler.toml`) and the user configuration (`config.toml`).
//!
//! Both files are TOML. A key that a table does not define is refused, naming the file and the
//! dotted key. `slots` is a machine-wide setting and is refused in a project configuration. A
//! relative path written in a configuration file resolves against the directory of that file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::{Table, Value};

/// The file name of a project configuration.
const PROJECT_FILE: &str = "candler.toml";

/// The tables of a project configuration and the keys each defines.
const PROJECT_TABLES: [(&str, &[&str]); 2] = [("store", &["root"]), ("plugins", &["dirs"])];

/// The tables of a user configuration and the keys each defines.
const USER_TABLES: [(&str, &[&str]); 1] = [("slots", &["count"])];

/// Why a configuration could not be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The file exists but could not be read.
    #[error("{}: cannot read the file: {source}", file.display())]
    Read {
        /// The configuration file.
        file: PathBuf,
        /// The underlying error.
        source: io::Error,
    },
    /// The file is not valid TOML.
    #[error("{}: {source}", file.display())]
    Parse {
        /// The configuration file.
        file: PathBuf,
        /// The parse error.
        source: toml::de::Error,
    },
    /// The file holds a key that no table defines.
    #[error("{}: unknown key `{key}`", file.display())]
    UnknownKey {
        /// The configuration file.
        file: PathBuf,
        /// The dotted key.
        key: String,
    },
    /// A project configuration holds a machine-wide setting.
    #[error("{}: `{key}` is a machine-wide setting and belongs in the user configuration", file.display())]
    MachineWideKey {
        /// The configuration file.
        file: PathBuf,
        /// The dotted key.
        key: String,
    },
    /// A key holds a value of the wrong type.
    #[error("{}: `{key}` must be {expected}", file.display())]
    WrongType {
        /// The configuration file.
        file: PathBuf,
        /// The dotted key.
        key: String,
        /// What the key must hold.
        expected: &'static str,
    },
    /// The working directory is not absolute, so no ancestor can be searched.
    #[error("the working directory {} is not absolute", cwd.display())]
    RelativeCwd {
        /// The working directory.
        cwd: PathBuf,
    },
}

/// The environment variables that locate the user configuration and the user state directory, read by the caller.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConfigEnv {
    /// The value of `XDG_CONFIG_HOME`; an empty value counts as unset.
    pub xdg_config_home: Option<PathBuf>,
    /// The value of `XDG_STATE_HOME`; an empty value counts as unset.
    pub xdg_state_home: Option<PathBuf>,
    /// The home directory.
    pub home: Option<PathBuf>,
}

impl ConfigEnv {
    /// An environment with the given variables.
    #[must_use]
    pub const fn new(
        xdg_config_home: Option<PathBuf>,
        xdg_state_home: Option<PathBuf>,
        home: Option<PathBuf>,
    ) -> Self {
        Self {
            xdg_config_home,
            xdg_state_home,
            home,
        }
    }

    /// Reads `XDG_CONFIG_HOME`, `XDG_STATE_HOME` and `HOME` from the process environment.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            xdg_state_home: std::env::var_os("XDG_STATE_HOME").map(PathBuf::from),
            home: std::env::var_os("HOME").map(PathBuf::from),
        }
    }

    /// The directory that holds the user configuration: `$XDG_CONFIG_HOME`, else
    /// `<home>/.config`.
    #[must_use]
    pub fn config_home(&self) -> Option<PathBuf> {
        match self.xdg_config_home.as_deref() {
            Some(xdg) if !xdg.as_os_str().is_empty() => Some(xdg.to_path_buf()),
            Some(_) | None => self.home.as_deref().map(|home| home.join(".config")),
        }
    }

    /// The directory that holds the user state: `$XDG_STATE_HOME`, else `<home>/.local/state`.
    #[must_use]
    pub fn state_home(&self) -> Option<PathBuf> {
        match self.xdg_state_home.as_deref() {
            Some(xdg) if !xdg.as_os_str().is_empty() => Some(xdg.to_path_buf()),
            Some(_) | None => self
                .home
                .as_deref()
                .map(|home| home.join(".local").join("state")),
        }
    }
}

/// The project configuration: the nearest `candler.toml` at or above the working directory.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProjectConfig {
    /// The directory that holds the file: the project root.
    pub root: PathBuf,
    /// The configuration file.
    pub file: PathBuf,
    /// `store.root`, absolute.
    pub store_root: Option<PathBuf>,
    /// `plugins.dirs`, each absolute.
    pub plugin_dirs: Vec<PathBuf>,
}

impl ProjectConfig {
    /// Finds and reads the nearest `candler.toml` in `cwd` or one of its ancestors.
    ///
    /// # Errors
    /// Returns [`ConfigError::RelativeCwd`] if `cwd` is not absolute, [`ConfigError::Read`] or
    /// [`ConfigError::Parse`] if the file cannot be read as TOML, [`ConfigError::UnknownKey`] for
    /// a key no table defines, [`ConfigError::MachineWideKey`] for `slots`, and
    /// [`ConfigError::WrongType`] for a value of the wrong type.
    pub fn discover(cwd: &Path) -> Result<Option<Self>, ConfigError> {
        if !cwd.is_absolute() {
            return Err(ConfigError::RelativeCwd {
                // BORROW: `.to_owned()` makes the owned path the error carries
                cwd: cwd.to_owned(),
            });
        }
        for dir in cwd.ancestors() {
            let file = dir.join(PROJECT_FILE);
            if file.is_file() {
                // BORROW: `.to_owned()` makes the owned root the config carries
                return Self::read(dir.to_owned(), file).map(Some);
            }
        }
        Ok(None)
    }

    fn read(root: PathBuf, file: PathBuf) -> Result<Self, ConfigError> {
        // BORROW: `.as_path()` views the owned file as a `&Path`
        let table = read_table(file.as_path())?;
        if let Some(slots) = table.get("slots") {
            let key = match slots.as_table().and_then(|t| t.keys().next()) {
                Some(first) => format!("slots.{first}"),
                None => "slots".to_owned(), // BORROW: owned key for the error
            };
            return Err(ConfigError::MachineWideKey { file, key });
        }
        // BORROW: `.as_path()` and `.as_slice()` view the owned file and the table list as borrows
        check_keys(file.as_path(), &table, PROJECT_TABLES.as_slice())?;
        let store_root = match table.get("store").and_then(Value::as_table) {
            Some(store) => store
                .get("root")
                // BORROW: `.as_path()` views the owned file and root as `&Path`
                .map(|v| path_value(file.as_path(), "store.root", v, root.as_path()))
                .transpose()?,
            None => None,
        };
        let plugin_dirs = match table.get("plugins").and_then(Value::as_table) {
            Some(plugins) => match plugins.get("dirs") {
                // BORROW: `.as_path()` views the owned file and root as `&Path`
                Some(dirs) => path_list(file.as_path(), "plugins.dirs", dirs, root.as_path())?,
                None => Vec::new(),
            },
            None => Vec::new(),
        };
        Ok(Self {
            root,
            file,
            store_root,
            plugin_dirs,
        })
    }
}

/// The user configuration: `slots.count`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct UserConfig {
    /// The file that was read; `None` when no file exists.
    pub file: Option<PathBuf>,
    /// `slots.count`.
    pub slots_count: Option<u32>,
}

impl UserConfig {
    /// Reads `$XDG_CONFIG_HOME/candler/config.toml`, else `<home>/.config/candler/config.toml`. A
    /// missing file, or no location to look in, is an empty configuration.
    ///
    /// # Errors
    /// Returns [`ConfigError::Read`] or [`ConfigError::Parse`] if the file cannot be read as
    /// TOML, [`ConfigError::UnknownKey`] for a key no table defines, and
    /// [`ConfigError::WrongType`] for a value of the wrong type.
    pub fn load(env: &ConfigEnv) -> Result<Self, ConfigError> {
        let Some(base) = env.config_home() else {
            return Ok(Self::default());
        };
        let file = base.join("candler").join("config.toml");
        if !file.is_file() {
            return Ok(Self::default());
        }
        // BORROW: `.as_path()` and `.as_slice()` view the owned file and the table list as borrows
        let table = read_table(file.as_path())?;
        // BORROW: `.as_path()` and `.as_slice()` view the owned file and the table list as borrows
        check_keys(file.as_path(), &table, USER_TABLES.as_slice())?;
        let slots_count = match table.get("slots").and_then(Value::as_table) {
            Some(slots) => slots
                .get("count")
                .map(|v| {
                    v.as_integer()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or_else(|| ConfigError::WrongType {
                            // BORROW: `.clone()` makes the owned file the error carries
                            file: file.clone(),
                            key: "slots.count".to_owned(), // BORROW: owned key for the error
                            expected: "a non-negative integer that fits in 32 bits",
                        })
                })
                .transpose()?,
            None => None,
        };
        Ok(Self {
            file: Some(file),
            slots_count,
        })
    }
}

/// Both configuration files and the working directory they were found from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Config {
    /// The caller's working directory; the base of paths given as arguments.
    pub cwd: PathBuf,
    /// The project configuration, if a `candler.toml` exists at or above `cwd`.
    pub project: Option<ProjectConfig>,
    /// The user configuration.
    pub user: UserConfig,
    /// The directory that holds the user state (`$XDG_STATE_HOME`, else `~/.local/state`); the
    /// default store root lives under it when there is no project.
    pub state_home: Option<PathBuf>,
}

impl Config {
    /// A configuration from its parts.
    #[must_use]
    pub const fn new(
        cwd: PathBuf,
        project: Option<ProjectConfig>,
        user: UserConfig,
        state_home: Option<PathBuf>,
    ) -> Self {
        Self {
            cwd,
            project,
            user,
            state_home,
        }
    }

    /// Loads the project configuration found from `cwd` and the user configuration located by
    /// `env`.
    ///
    /// # Errors
    /// Returns the [`ConfigError`] of either file.
    pub fn load(cwd: &Path, env: &ConfigEnv) -> Result<Self, ConfigError> {
        Ok(Self {
            // BORROW: `.to_owned()` makes the owned path the config carries
            cwd: cwd.to_owned(),
            project: ProjectConfig::discover(cwd)?,
            user: UserConfig::load(env)?,
            state_home: env.state_home(),
        })
    }
}

fn read_table(file: &Path) -> Result<Table, ConfigError> {
    let text = fs::read_to_string(file).map_err(|source| ConfigError::Read {
        // BORROW: `.to_owned()` makes the owned path the error carries
        file: file.to_owned(),
        source,
    })?;
    // BORROW: `.as_str()` views the owned text as a `&str`
    toml::from_str(text.as_str()).map_err(|source| ConfigError::Parse {
        // BORROW: `.to_owned()` makes the owned path the error carries
        file: file.to_owned(),
        source,
    })
}

/// Refuses every key of `table`, and of its tables, that `tables` does not define.
fn check_keys(file: &Path, table: &Table, tables: &[(&str, &[&str])]) -> Result<(), ConfigError> {
    let unknown = |key: String| ConfigError::UnknownKey {
        // BORROW: `.to_owned()` makes the owned path the error carries
        file: file.to_owned(),
        key,
    };
    for (key, value) in table {
        // BORROW: `.as_str()` views the key as a `&str` for the comparison
        let Some((name, keys)) = tables.iter().find(|(name, _)| *name == key.as_str()) else {
            return Err(unknown(key.clone())); // BORROW: owned key for the error
        };
        let Some(inner) = value.as_table() else {
            return Err(ConfigError::WrongType {
                // BORROW: `.to_owned()` makes the owned path the error carries
                file: file.to_owned(),
                key: (*name).to_owned(), // BORROW: owned key for the error
                expected: "a table",
            });
        };
        for inner_key in inner.keys() {
            // BORROW: `.as_str()` views the key as a `&str` for the comparison
            if !keys.contains(&inner_key.as_str()) {
                return Err(unknown(format!("{name}.{inner_key}")));
            }
        }
    }
    Ok(())
}

/// A string value as a path, made absolute against `base`.
fn path_value(file: &Path, key: &str, value: &Value, base: &Path) -> Result<PathBuf, ConfigError> {
    value
        // BORROW: `.as_str()` views the TOML string as a `&str`
        .as_str()
        .map(|text| base.join(text))
        .ok_or_else(|| ConfigError::WrongType {
            // BORROW: `.to_owned()` makes the owned path the error carries
            file: file.to_owned(),
            key: key.to_owned(), // BORROW: owned key for the error
            expected: "a string",
        })
}

/// An array of strings as paths, each made absolute against `base`.
fn path_list(
    file: &Path,
    key: &str,
    value: &Value,
    base: &Path,
) -> Result<Vec<PathBuf>, ConfigError> {
    let wrong = || ConfigError::WrongType {
        // BORROW: `.to_owned()` makes the owned path the error carries
        file: file.to_owned(),
        key: key.to_owned(), // BORROW: owned key for the error
        expected: "an array of strings",
    };
    value
        .as_array()
        .ok_or_else(wrong)?
        .iter()
        // BORROW: `.as_str()` views the TOML string as a `&str`
        .map(|item| item.as_str().map(|text| base.join(text)).ok_or_else(wrong))
        .collect()
}
