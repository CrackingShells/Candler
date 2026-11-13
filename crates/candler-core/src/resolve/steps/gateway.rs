//! The gateway step: whether the endpoint accepts the token, checked without spending any.
//!
//! The step asks the endpoint for its model list (`GET <model.endpoint>/v1/models`) from the host,
//! with the secret `model.token_secret` names as a bearer token. A model list is a catalogue read:
//! no completion is requested and no sidecar is started. `200` passes, `401` fails with "the token
//! was refused", and no answer fails with "unreachable". The request goes to `curl`, which reads
//! its header from its standard input, so the token is in no argument and no file; the check
//! names the secret and never holds its value.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use candler_schemas::primitives::{FieldPath, FormatError, Marker, NameToken};
use candler_schemas::process::ExitStatus;
use futures::future::BoxFuture;

use crate::resolve::preflight::{
    CheckResult, PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver, StepReport,
};
use crate::supervise::Supervisor;
use crate::supervise::spawn::{SpawnRequest, StdinSpec};
use candler_schemas::execution::paths;

/// How long the request gets, connection and answer together.
const TIMEOUT: Duration = Duration::from_secs(8);

/// The bytes of output the probe keeps: the status code is three.
const OUTPUT_LIMIT: usize = 64;

fn failed(detail: &impl ToString) -> PreflightError {
    PreflightError::Failed {
        detail: detail.to_string(),
    }
}

/// What the endpoint answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Answer {
    /// An HTTP status.
    Status(u16),
    /// No HTTP answer: the connection or the name failed, or the time ran out.
    Unreachable,
}

/// The check the answer makes: whether it passed and what was found. The explanation names the
/// endpoint and the secret, and holds no value.
fn judge(answer: Answer, endpoint: &str, secret: &str) -> (bool, String) {
    match answer {
        Answer::Status(200) => (
            true,
            format!("{endpoint} accepted the secret {secret}: its model list was read"),
        ),
        Answer::Status(401) => (
            false,
            format!("the token was refused: {endpoint} answered 401 to the secret {secret}"),
        ),
        Answer::Status(status) => (
            false,
            format!("{endpoint} answered {status} to the read of its model list"),
        ),
        Answer::Unreachable => (false, format!("unreachable: {endpoint} did not answer")),
    }
}

/// Whether `token` can be written inside a quoted curl config value.
fn config_safe(token: &str) -> bool {
    !token.is_empty()
        && token
            .chars()
            .all(|c| c.is_ascii_graphic() && c != '"' && c != '\\')
}

/// Checks that the endpoint accepts the token, through a read of its model list.
pub struct GatewayStep {
    name: NameToken,
    check: NameToken,
    supervisor: Arc<Supervisor>,
}

impl GatewayStep {
    /// A step that runs its probe through `supervisor`.
    ///
    /// # Errors
    /// [`FormatError`] when a name is not a name token.
    pub fn new(supervisor: Arc<Supervisor>) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("gateway@1")?,
            check: NameToken::from_str("gateway_token@1")?,
            supervisor,
        })
    }
}

/// The text the context resolved at `path`.
fn text_at(ctx: &ResolveContext, path: &str) -> Option<String> {
    let path = FieldPath::from_str(path).ok()?;
    ctx.get(&path)
        .and_then(|value| value.as_value().as_str())
        .map(str::to_owned) // BORROW: the future owns its text
}

/// Reads `<endpoint>/v1/models` with `token`, through the supervisor.
async fn read_models(
    supervisor: &Supervisor,
    endpoint: &str,
    token: &str,
) -> Result<Answer, PreflightError> {
    let url = format!("{}/v1/models", endpoint.trim_end_matches('/'));
    let argv = [
        "curl",
        "--silent",
        "--output",
        "/dev/null",
        "--max-time",
        "6",
        "--write-out",
        "%{http_code}",
        "--config",
        "-",
        url.as_str(),
    ]
    .iter()
    .map(|word| (*word).to_owned()) // BORROW: the request owns its words
    .collect();
    let marker = Marker::from_str("gateway-token").map_err(|e| failed(&e))?;
    let record_dir = std::env::temp_dir().join(format!(
        "candler-gateway-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos())
    ));
    tokio::fs::create_dir_all(&record_dir)
        .await
        .map_err(|e| failed(&format!("cannot make the probe directory: {e}")))?;
    let config = format!("header = \"Authorization: Bearer {token}\"\n");
    let request = SpawnRequest::builder()
        .argv(argv)
        .env(vec![(
            "PATH".to_owned(), // BORROW: the request owns its names
            std::env::var("PATH").unwrap_or_default(),
        )])
        .marker(marker)
        .stdin(StdinSpec::Bytes(config.into_bytes()))
        .deadline(TIMEOUT)
        .record_dir(record_dir.clone()) // BORROW: this function removes the directory
        .build()
        .map_err(|e| failed(&e))?;
    let outcome = supervisor.run_captured(request, OUTPUT_LIMIT).await;
    drop(tokio::fs::remove_dir_all(&record_dir).await);
    let captured = outcome.map_err(|e| failed(&format!("cannot run curl: {e}")))?;
    match captured.exit.status {
        ExitStatus::Code { code: 0 } => {
            let text = String::from_utf8_lossy(&captured.stdout);
            Ok(text
                .trim()
                .parse::<u16>()
                .map_or(Answer::Unreachable, Answer::Status))
        }
        ExitStatus::Code {
            code: 7 | 6 | 28 | 35 | 52 | 56,
        }
        | ExitStatus::Signal { .. } => Ok(Answer::Unreachable),
        ExitStatus::Code { code: 127 } => Err(failed(&"curl is not installed")),
        ExitStatus::Code { code } => Err(failed(&format!("curl ended with status {code}"))),
    }
}

impl PreflightStep for GatewayStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: `Arc::clone` moves a share of the supervisor into the future
        let supervisor = Arc::clone(&self.supervisor);
        // BORROW: `.clone()` moves the names into the future
        let (step, check) = (self.name.clone(), self.check.clone());
        Box::pin(async move {
            let report = StepReport::new(step);
            let Some(endpoint) = text_at(&ctx, paths::MODEL_ENDPOINT) else {
                return Ok(report.check(CheckResult::new(
                    check,
                    false,
                    format!("not checked: `{}` is not set", paths::MODEL_ENDPOINT),
                )));
            };
            let Some(secret) = text_at(&ctx, paths::MODEL_TOKEN_SECRET) else {
                return Ok(report.check(CheckResult::new(
                    check,
                    false,
                    format!("not checked: `{}` is not set", paths::MODEL_TOKEN_SECRET),
                )));
            };
            let token = std::env::var(&secret).unwrap_or_default();
            if !config_safe(&token) {
                return Ok(report.check(CheckResult::new(
                    check,
                    false,
                    format!("not checked: the secret {secret} is not set, or cannot be sent as a bearer token"),
                )));
            }
            let answer = read_models(&supervisor, &endpoint, &token).await?;
            let (passed, explanation) = judge(answer, &endpoint, &secret);
            Ok(report.check(CheckResult::new(check, passed, explanation)))
        })
    }
}

/// Registers the gateway step on `resolver`, after the steps already there.
///
/// # Errors
/// [`ResolveError`] when a step of that name is registered already or the name is malformed.
pub fn register_gateway_step(
    resolver: Resolver,
    supervisor: Arc<Supervisor>,
) -> Result<Resolver, ResolveError> {
    let step = GatewayStep::new(supervisor)?;
    resolver.register(Box::new(step)) // TRAIT_OBJECT: steps of different types are registered side by side
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards: O1 — the gateway check's verdicts: 200 passes, 401 says the token was refused, any
    /// other status and no answer fail, and no explanation holds a token. Oracle: hand-written
    /// expectations. Bite: 401 passing; "unreachable" worded as a refusal.
    #[test]
    fn o1_the_gateway_verdicts_follow_the_answer() {
        let ok = judge(Answer::Status(200), "https://e.example", "KEY");
        assert!(ok.0);
        let refused = judge(Answer::Status(401), "https://e.example", "KEY");
        assert!(!refused.0 && refused.1.contains("the token was refused"));
        let other = judge(Answer::Status(503), "https://e.example", "KEY");
        assert!(!other.0 && other.1.contains("503"));
        let gone = judge(Answer::Unreachable, "https://e.example", "KEY");
        assert!(!gone.0 && gone.1.starts_with("unreachable"));
        assert!(!gone.1.contains("refused"));
    }

    /// Guards: O1 — a token that cannot sit in a quoted curl config value is never sent. Oracle:
    /// hand-written cases. Bite: a quote or a line break let through.
    #[test]
    fn o1_a_token_with_a_quote_or_a_line_break_is_not_sent() {
        assert!(config_safe("abc-123_XYZ.def"));
        for bad in ["", "a\"b", "a\\b", "a\nb", "a b"] {
            assert!(!config_safe(bad), "{bad:?}");
        }
    }
}
