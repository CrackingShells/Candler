//! The egress step: the hostnames and ports a cell lets its agent reach.
//!
//! `isolation.egress` is a list of `host:port` strings. [`settle`] is the rule as one pure
//! function: it refuses a value or returns the canonical list, lowercase, sorted and without
//! duplicates. An absent value stays absent, so a cell that does not write the setting keeps its
//! digest, and an empty list is absent too. The step applies the rule to the experiment-wide
//! values, and planning applies it to each cell's own and writes the canonical list into the cell,
//! so a cell is the same cell whatever case, order or repetition its author wrote.
//!
//! A host is an RFC 1123 hostname: labels of 1 to 63 letters, digits and hyphens that neither
//! start nor end with a hyphen, at most 253 characters in all, at least one dot, and a last label
//! that is not all digits (so an IPv4 literal is no host). The port is 1 to 65535 and is never
//! defaulted. An entry that carries a scheme, a path, a wildcard or an IP literal is refused.
//!
//! Nothing enforces an allowlist yet, so [`refuse_until_proxy`] refuses a valid one with
//! [`EgressRefusal::Unsupported`], at resolve and per cell, rather than letting a cell record a
//! restriction the run does not have. [`settle`] stays the whole parse and canonical form.

use std::net::Ipv4Addr;
use std::str::FromStr as _;
use std::sync::Arc;

use candler_schemas::execution::paths;
use candler_schemas::primitives::{FieldPath, FormatError, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::Value;

use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, StepReport,
};

/// The name of the step.
pub const STEP_NAME: &str = "egress_setting@1";

/// The check that names an entry the rule refuses.
pub const CHECK_INVALID: &str = "egress_invalid@1";

/// The check that names an allowlist set together with `isolation.network = bridge`.
pub const CHECK_WITH_BRIDGE: &str = "egress_with_bridge@1";

/// The check that names an allowlist no run can enforce yet.
pub const CHECK_UNSUPPORTED: &str = "egress_unsupported@1";

/// The `isolation.network` value that reaches the whole internet.
const BRIDGE: &str = "bridge";

/// The longest hostname, in characters.
const MAX_HOST: usize = 253;

/// The longest label of a hostname, in characters.
const MAX_LABEL: usize = 63;

/// One allowed destination: a lowercase hostname and a port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostPort {
    /// The hostname, lowercase.
    pub host: String,
    /// The port, 1 to 65535.
    pub port: u16,
}

impl std::fmt::Display for HostPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.port)
    }
}

/// Why a cell's egress allowlist is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EgressRefusal {
    /// The value is not a list of `host:port` strings, or an entry breaks the rule.
    #[error("{detail}")]
    Invalid {
        /// The entry and what is wrong with it.
        detail: String,
    },
    /// An allowlist is set on a cell whose network is `bridge`.
    #[error("{detail}")]
    WithBridge {
        /// What is wrong, for the author of the experiment.
        detail: String,
    },
    /// An allowlist is set, and no run enforces one yet.
    #[error("{detail}")]
    Unsupported {
        /// What is wrong, for the author of the experiment.
        detail: String,
    },
}

impl EgressRefusal {
    fn invalid(entry: &str, why: &str) -> Self {
        Self::Invalid {
            detail: format!("isolation.egress entry `{entry}` is refused: {why}"),
        }
    }

    /// The check that names this refusal, as `name@token`.
    #[must_use]
    pub const fn check(&self) -> &'static str {
        match self {
            Self::Invalid { .. } => CHECK_INVALID,
            Self::WithBridge { .. } => CHECK_WITH_BRIDGE,
            Self::Unsupported { .. } => CHECK_UNSUPPORTED,
        }
    }

    /// What is wrong, for the author of the experiment.
    #[must_use]
    pub fn detail<'a>(&'a self) -> &'a str {
        match self {
            Self::Invalid { detail }
            | Self::WithBridge { detail }
            | Self::Unsupported { detail } => detail.as_str(),
        }
    }

    /// The refusal a failed check of the step stands for, when `check` names one.
    #[must_use]
    pub fn from_check(check: &str, detail: &str) -> Option<Self> {
        let detail = detail.to_owned(); // BORROW: the refusal owns the detail
        match check {
            CHECK_INVALID => Some(Self::Invalid { detail }),
            CHECK_WITH_BRIDGE => Some(Self::WithBridge { detail }),
            CHECK_UNSUPPORTED => Some(Self::Unsupported { detail }),
            // EXHAUSTIVE: any other check belongs to another step, which this refusal does not stand for
            _ => None,
        }
    }
}

/// Parses one entry, already known to be a string.
fn parse_entry(entry: &str) -> Result<HostPort, EgressRefusal> {
    let refuse = |why: &str| EgressRefusal::invalid(entry, why);
    if entry.contains("://") {
        return Err(refuse("a scheme is not allowed; write host:port"));
    }
    if entry.contains('*') {
        return Err(refuse("a wildcard is not allowed; name each host"));
    }
    if entry.contains('/') {
        return Err(refuse("a path is not allowed; write host:port"));
    }
    if entry.contains('[') || entry.contains(']') || entry.matches(':').count() > 1 {
        return Err(refuse("an IPv6 literal is not allowed; name a host"));
    }
    let Some((host, port)) = entry.split_once(':') else {
        return Err(refuse("the port is missing; write host:port"));
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(refuse("the port must be a number from 1 to 65535"));
    }
    let port = match port.parse::<u16>() {
        Ok(port) if port > 0 => port,
        Ok(_) | Err(_) => return Err(refuse("the port must be a number from 1 to 65535")),
    };
    if host.parse::<Ipv4Addr>().is_ok() {
        return Err(refuse("an IPv4 literal is not allowed; name a host"));
    }
    let host = host.to_ascii_lowercase();
    if let Some(why) = hostname_defect(&host) {
        return Err(refuse(why));
    }
    Ok(HostPort { host, port })
}

/// What breaks RFC 1123's hostname rule in `host` (already lowercase), if anything.
fn hostname_defect(host: &str) -> Option<&'static str> {
    if host.len() > MAX_HOST {
        return Some("the host is longer than 253 characters");
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 {
        return Some("the host needs at least one dot");
    }
    for label in &labels {
        if label.is_empty() || label.len() > MAX_LABEL {
            return Some("every label of the host is 1 to 63 characters");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Some("a label of the host may not start or end with a hyphen");
        }
        if !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Some("a host holds letters, digits, hyphens and dots only");
        }
    }
    if labels
        .last()
        .is_some_and(|last| last.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Some("the last label of a host is not all digits");
    }
    None
}

/// The egress rule of one cell, from its `isolation.egress` and `isolation.network`.
///
/// Returns `None` when no allowlist is written, and otherwise the canonical list: lowercase,
/// sorted by its `host:port` text and without duplicates. An empty list is no allowlist, so it
/// returns `None` like an absent value.
///
/// # Errors
/// [`EgressRefusal::Invalid`] naming the first entry (in written order) that breaks the rule or a
/// value that is not a list of strings, and [`EgressRefusal::WithBridge`] when the list holds an
/// entry and the network is `bridge`.
pub fn settle(
    egress: Option<&Value>,
    network: Option<&Value>,
) -> Result<Option<Vec<HostPort>>, EgressRefusal> {
    let Some(egress) = egress else {
        return Ok(None);
    };
    let Some(items) = egress.as_array() else {
        return Err(EgressRefusal::Invalid {
            detail: format!("isolation.egress is {egress}: write a list of \"host:port\" strings"),
        });
    };
    let mut entries = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(EgressRefusal::Invalid {
                detail: format!(
                    "isolation.egress entry {item} is refused: \
                     write each entry as a \"host:port\" string"
                ),
            });
        };
        entries.push(parse_entry(text)?);
    }
    entries.sort_by_cached_key(ToString::to_string);
    entries.dedup();
    if let Some(first) = entries.first()
        && network.and_then(Value::as_str) == Some(BRIDGE)
    {
        return Err(EgressRefusal::WithBridge {
            detail: format!(
                "isolation.egress lists `{first}` and isolation.network is bridge, which reaches \
                 the whole internet: drop one of the two"
            ),
        });
    }
    // An empty list allows nothing beyond what an absent value allows, so it is absent.
    Ok((!entries.is_empty()).then_some(entries))
}

/// Refuses a settled allowlist: nothing enforces one yet, so a cell that sets any entry does not
/// run. An absent list passes unchanged.
///
/// # Errors
/// [`EgressRefusal::Unsupported`] naming the first entry when `settled` holds one.
pub fn refuse_until_proxy(
    settled: Option<Vec<HostPort>>,
) -> Result<Option<Vec<HostPort>>, EgressRefusal> {
    match settled.as_deref().and_then(<[HostPort]>::first) {
        Some(first) => Err(EgressRefusal::Unsupported {
            detail: format!(
                "isolation.egress lists `{first}`, and no run enforces an egress allowlist yet: \
                 remove `isolation.egress` from the experiment and from the plug-in environment"
            ),
        }),
        None => Ok(settled),
    }
}

/// The canonical list as the JSON value a cell holds.
///
/// # Errors
/// [`candler_schemas::values::ValuesError`] when the value cannot be hashed; a list of strings
/// can always be.
pub fn canonical_value(
    entries: &[HostPort],
) -> Result<JsonValue, candler_schemas::values::ValuesError> {
    JsonValue::new(Value::Array(
        entries
            .iter()
            .map(|entry| Value::String(entry.to_string()))
            .collect(),
    ))
}

/// Validates `isolation.egress` of the experiment-wide values; it derives nothing.
pub struct EgressStep {
    name: NameToken,
}

impl EgressStep {
    /// The step.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    pub fn new() -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str(STEP_NAME)?,
        })
    }
}

/// The value the context resolved at `path`.
fn value_at<'a>(ctx: &'a ResolveContext, path: &str) -> Result<Option<&'a Value>, PreflightError> {
    let path = FieldPath::from_str(path).map_err(|error| PreflightError::Failed {
        detail: error.to_string(), // BORROW: .to_string() gives owned text
    })?;
    Ok(ctx.get(&path).map(JsonValue::as_value))
}

impl PreflightStep for EgressStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        let step = self.name.clone(); // BORROW: the future owns the step name
        let outcome = (|| {
            let mut report = StepReport::new(step);
            let egress = value_at(&ctx, paths::ISOLATION_EGRESS)?;
            let network = value_at(&ctx, paths::ISOLATION_NETWORK)?;
            if let Err(refusal) = settle(egress, network).and_then(refuse_until_proxy) {
                let name = NameToken::from_str(refusal.check()).map_err(|error| {
                    PreflightError::Failed {
                        detail: error.to_string(), // BORROW: .to_string() gives owned text
                    }
                })?;
                // BORROW: owned text
                let explanation = refusal.detail().to_owned();
                report = report.check(CheckResult::new(name, false, explanation));
            }
            Ok(report)
        })();
        Box::pin(async move { outcome })
    }
}
