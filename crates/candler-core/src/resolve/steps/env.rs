//! The environment table of an experiment: the names it may not set.
//!
//! The harness process gets the `env` table of its cell. A name the container plan sets itself,
//! and a name of a declared secret, is refused before any value is resolved, naming the key.

use candler_schemas::execution::paths;
use candler_schemas::primitives::FieldPath;
use candler_schemas::values::JsonValue;

use crate::isolation::container::PLAN_ENV_NAMES;

/// Why an entry of the `env` table is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EnvRefusal {
    /// The name is one the container plan sets itself.
    #[error("`env.{name}` names a variable Candler sets")]
    Reserved {
        /// The variable name.
        name: String,
    },
    /// The name is one of the declared secrets.
    #[error("`env.{name}` names a declared secret, whose value reaches the gateway only")]
    Secret {
        /// The variable name.
        name: String,
    },
}

impl EnvRefusal {
    /// The name of the refused variable.
    #[must_use]
    pub fn name<'a>(&'a self) -> &'a str {
        match self {
            Self::Reserved { name } | Self::Secret { name } => name.as_str(),
        }
    }
}

/// Refuses the first name of the `env` table among `entries` that is reserved or one of `secrets`.
///
/// # Errors
/// [`EnvRefusal`] naming the variable.
pub fn check<'a>(
    entries: impl IntoIterator<Item = (&'a FieldPath, &'a JsonValue)>,
    secrets: &[String],
) -> Result<(), EnvRefusal> {
    for (path, value) in entries {
        if path.as_str() != paths::ENV {
            continue;
        }
        let Some(table) = value.as_value().as_object() else {
            continue;
        };
        for name in table.keys() {
            if PLAN_ENV_NAMES.contains(&name.as_str()) {
                return Err(EnvRefusal::Reserved {
                    name: name.clone(), // BORROW: the refusal owns the name
                });
            }
            if secrets.iter().any(|secret| secret == name) {
                return Err(EnvRefusal::Secret {
                    name: name.clone(), // BORROW: the refusal owns the name
                });
            }
        }
    }
    Ok(())
}
