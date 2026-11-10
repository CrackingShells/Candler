//! The image step: the env lock and the local image the agent runs in.
//!
//! `isolation.build.*` are settings: where the requirement sources, a ready lock and an optional
//! Dockerfile with its context are, and which files the env build prunes. A cell holds the lock's
//! digest (`isolation.env_lock`) and the image built from it (`isolation.image`, an image id,
//! never a tag). The `inputs` entry for the build inputs exists only when the experiment adds its
//! own layer. Changing the build request moves the cell only through those values. The prune list
//! keys the env image with the lock.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use candler_schemas::canonical::CanonicalError;
use candler_schemas::execution::{DigestRef, TreatmentRef};
use candler_schemas::primitives::{Digest, FieldPath, FormatError, Name, NameToken};
use candler_schemas::values::JsonValue;
use futures::future::BoxFuture;
use serde_json::{Map, Value, json};
use sha2::{Digest as _, Sha256};

use super::identity::show_file;
use crate::experiment::paths;
use crate::isolation::image::{
    BuildSetting, Extension, ImageService, LockDigest, LockRequest, LockSource, package_name_ok,
    prune_name_ok,
};
use crate::resolve::preflight::{PreflightError, PreflightStep, ResolveContext, StepReport};
use crate::supervise::Supervisor;

/// The prefix of a requirement source that lives in the treatment's repository.
const TREATMENT_PREFIX: &str = "treatment:";

/// What the experiment's own image layer is built from: the Dockerfile and the files of its
/// context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BuildInputs {
    /// The digest of the Dockerfile's bytes.
    pub dockerfile: Digest,
    /// The digest of each file of the context, by its path relative to the context, with `/` as
    /// the separator. A symbolic link is listed by the digest of its target text, and not
    /// followed.
    pub context: BTreeMap<String, Digest>,
}

impl BuildInputs {
    /// Reads the Dockerfile at `dockerfile` and walks the directory `context`.
    ///
    /// # Errors
    /// The [`io::Error`] of a file that cannot be read, or of a name that is not UTF-8.
    pub fn read(dockerfile: &Path, context: &Path) -> Result<Self, io::Error> {
        let mut files = BTreeMap::new();
        walk(context, "", &mut files)?;
        Ok(Self {
            dockerfile: digest_file(dockerfile)?,
            context: files,
        })
    }

    /// The inputs as the JSON object their digest is taken over.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let context: Vec<Value> = self
            .context
            .iter()
            .map(|(path, digest)| json!({"path": path, "digest": digest}))
            .collect();
        let mut object = Map::new();
        object.insert("dockerfile".to_owned(), json!(self.dockerfile)); // BORROW: owned key for the object
        object.insert("context".to_owned(), Value::Array(context)); // BORROW: owned key for the object
        Value::Object(object)
    }
}

/// The digest of `inputs`: the SHA-256 of their canonical JSON text.
///
/// # Errors
/// [`CanonicalError`] when the inputs have no canonical text.
pub fn build_input_digest(inputs: &BuildInputs) -> Result<Digest, CanonicalError> {
    Digest::of_canonical(&inputs.to_json())
}

fn digest_of_hash(hasher: Sha256) -> Result<Digest, io::Error> {
    let mut hex = String::new();
    for byte in hasher.finalize() {
        write!(hex, "{byte:02x}").map_err(io::Error::other)?;
    }
    Digest::from_hex(&hex).map_err(|e| io::Error::other(e.to_string()))
}

fn digest_of(bytes: &[u8]) -> Result<Digest, io::Error> {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    digest_of_hash(hasher)
}

/// `error` with the path it failed on in its text.
fn at(path: &Path, error: &io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{}: {error}", path.display()))
}

fn digest_file(path: &Path) -> Result<Digest, io::Error> {
    let mut file = fs::File::open(path).map_err(|e| at(path, &e))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| at(path, &e))?;
        let Some(chunk) = buffer.get(..read) else {
            return Err(io::Error::other("a read returned more than the buffer"));
        };
        if chunk.is_empty() {
            break;
        }
        hasher.update(chunk);
    }
    digest_of_hash(hasher)
}

/// Lists the files below `dir`, keyed by their path below the context root.
fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Digest>) -> Result<(), io::Error> {
    for entry in fs::read_dir(dir).map_err(|e| at(dir, &e))? {
        let entry = entry.map_err(|e| at(dir, &e))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| io::Error::other("a file name in the context is not UTF-8"))?;
        let key = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let kind = entry.file_type().map_err(|e| at(&entry.path(), &e))?;
        if kind.is_symlink() {
            let target = fs::read_link(entry.path()).map_err(|e| at(&entry.path(), &e))?;
            let text = format!("symlink:{}", target.to_string_lossy());
            out.insert(key, digest_of(text.as_bytes())?);
        } else if kind.is_dir() {
            walk(&entry.path(), &key, out)?;
        } else {
            out.insert(key, digest_file(&entry.path())?);
        }
    }
    Ok(())
}

fn failed(detail: String) -> PreflightError {
    PreflightError::Failed { detail }
}

fn path_of(text: &str) -> Result<FieldPath, PreflightError> {
    FieldPath::from_str(text).map_err(|e| failed(e.to_string()))
}

fn text_at(ctx: &ResolveContext, text: &str) -> Result<Option<String>, PreflightError> {
    let Some(found) = ctx.get(&path_of(text)?) else {
        return Ok(None);
    };
    found
        .as_value()
        .as_str()
        // BORROW: `.to_owned()` makes the owned text the step works with
        .map(|s| Some(s.to_owned()))
        .ok_or_else(|| failed(format!("`{text}` must be a string")))
}

fn required_text(ctx: &ResolveContext, text: &str) -> Result<String, PreflightError> {
    text_at(ctx, text)?.ok_or_else(|| failed(format!("`{text}` has no value")))
}

fn read_value<T: serde::de::DeserializeOwned>(
    ctx: &ResolveContext,
    text: &str,
) -> Result<T, PreflightError> {
    let found = ctx
        .get(&path_of(text)?)
        .ok_or_else(|| failed(format!("`{text}` has no value")))?;
    // BORROW: `.clone()` hands the JSON to the deserializer, which takes it by value
    serde_json::from_value(found.as_value().clone())
        .map_err(|e| failed(format!("`{text}` is not valid: {e}")))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<JsonValue, PreflightError> {
    let json = serde_json::to_value(value).map_err(|e| failed(e.to_string()))?;
    JsonValue::new(json).map_err(|e| failed(e.to_string()))
}

/// Derives `isolation.env_lock`, `isolation.image` and, with an extension, the `inputs` entry.
pub struct ImageIdStep {
    name: NameToken,
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    images: Arc<dyn ImageService>,
    supervisor: Arc<Supervisor>,
}

impl ImageIdStep {
    /// A step that compiles the lock and asks `images` for the image it names; `supervisor` reads
    /// requirement sources out of the pinned treatment commit.
    ///
    /// # Errors
    /// [`FormatError`] when the step name is not a name token.
    // TRAIT_OBJECT: the service is docker, or a fake in tests
    pub fn new(
        images: Arc<dyn ImageService>,
        supervisor: Arc<Supervisor>,
    ) -> Result<Self, FormatError> {
        Ok(Self {
            name: NameToken::from_str("image_id@1")?,
            images,
            supervisor,
        })
    }
}

impl PreflightStep for ImageIdStep {
    fn name<'a>(&'a self) -> &'a NameToken {
        &self.name
    }

    fn run(
        &self,
        ctx: Arc<ResolveContext>,
    ) -> BoxFuture<'static, Result<StepReport, PreflightError>> {
        // BORROW: `Arc::clone` moves a share of the service into the future
        let images = Arc::clone(&self.images);
        // BORROW: `Arc::clone` moves a share of the supervisor into the future
        let supervisor = Arc::clone(&self.supervisor);
        // BORROW: `.clone()` moves the step name into the future
        let step = self.name.clone();
        Box::pin(async move {
            let prune = prune_of(&ctx)?;
            check_record_packages(&ctx)?;
            let lock = lock_bytes(&ctx, &images, &supervisor).await?;
            let lock_digest = digest_of(&lock).map_err(|e| failed(e.to_string()))?;
            let extension = extension_of(&ctx)?;
            let inputs = match &extension {
                Some(layer) => {
                    // BORROW: `.clone()` moves owned paths into the blocking read
                    let (dockerfile, context) = (layer.dockerfile.clone(), layer.context.clone());
                    Some(read_inputs(dockerfile, context).await?)
                }
                None => None,
            };
            let image = images
                .ensure(
                    &LockDigest(lock_digest.hex().to_owned()), // BORROW: owned hex for the lock digest
                    &BuildSetting {
                        lock,
                        extension,
                        prune,
                    },
                )
                .await
                .map_err(|e| failed(e.to_string()))?;
            Digest::from_str(&image.id.0).map_err(|e| {
                failed(format!(
                    "the image `{}` is not an image id of the form sha256:<hex>: {e}",
                    image.id.0
                ))
            })?;
            let mut report = StepReport::new(step)
                .derive(path_of(paths::ISOLATION_ENV_LOCK)?, to_json(&lock_digest)?)
                .derive(path_of(paths::ISOLATION_IMAGE)?, to_json(&image.id.0)?);
            if let Some(inputs) = inputs {
                let digest = build_input_digest(&inputs).map_err(|e| failed(e.to_string()))?;
                let kind = Name::from_str("build_inputs").map_err(|e| failed(e.to_string()))?;
                report = report.derive(
                    path_of(paths::INPUTS)?,
                    to_json(&[DigestRef::new(kind, digest)])?,
                );
            }
            Ok(report)
        })
    }
}

/// The entries of `isolation.build.prune`: file names, none when the setting is absent.
fn prune_of(ctx: &ResolveContext) -> Result<Vec<String>, PreflightError> {
    let Some(found) = ctx.get(&path_of(paths::ISOLATION_BUILD_PRUNE)?) else {
        return Ok(Vec::new());
    };
    let entries = found.as_value().as_array().ok_or_else(|| {
        failed(format!(
            "`{}` must be a list of file names, not {}",
            paths::ISOLATION_BUILD_PRUNE,
            found.as_value()
        ))
    })?;
    entries
        .iter()
        .map(|entry| {
            let name = entry.as_str().filter(|name| prune_name_ok(name)); // BORROW: .as_str() reads the entry's text
            name.map(str::to_owned) // BORROW: owned name for the build
                .ok_or_else(|| {
                    failed(format!(
                        "`{}` takes names of letters, digits, `.`, `_` and `-`, not {entry}",
                        paths::ISOLATION_BUILD_PRUNE
                    ))
                })
        })
        .collect()
}

/// Refuses `isolation.record_packages` unless it is a list of import package names: each name
/// becomes a field-path segment of the manifest.
fn check_record_packages(ctx: &ResolveContext) -> Result<(), PreflightError> {
    let Some(found) = ctx.get(&path_of(paths::ISOLATION_RECORD_PACKAGES)?) else {
        return Ok(());
    };
    let refused = |value: &Value| {
        failed(format!(
            "`{}` must be a list of import names of lower-case letters, digits and `_`, not {value}",
            paths::ISOLATION_RECORD_PACKAGES
        ))
    };
    let entries = found
        .as_value()
        .as_array()
        .ok_or_else(|| refused(found.as_value()))?;
    let bad = entries
        .iter()
        .find(|entry| !entry.as_str().is_some_and(package_name_ok)); // BORROW: .as_str() reads the entry's text
    bad.map_or(Ok(()), |value| Err(refused(value)))
}

/// The experiment's own layer, when both `isolation.build.dockerfile` and
/// `isolation.build.context` are set.
fn extension_of(ctx: &ResolveContext) -> Result<Option<Extension>, PreflightError> {
    let dockerfile = text_at(ctx, paths::ISOLATION_BUILD_DOCKERFILE)?;
    let context = text_at(ctx, paths::ISOLATION_BUILD_CONTEXT)?;
    match (dockerfile, context) {
        (Some(dockerfile), Some(context)) => Ok(Some(Extension {
            dockerfile: ctx.config.cwd.join(dockerfile),
            context: ctx.config.cwd.join(context),
        })),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(failed(format!(
            "`{}` and `{}` must be set together",
            paths::ISOLATION_BUILD_DOCKERFILE,
            paths::ISOLATION_BUILD_CONTEXT
        ))),
    }
}

/// The env lock: the `isolation.build.lock` file verbatim when it is set, else the lock the image
/// service compiles from `isolation.build.requirements`.
async fn lock_bytes(
    ctx: &ResolveContext,
    images: &Arc<dyn ImageService>,
    supervisor: &Supervisor,
) -> Result<Vec<u8>, PreflightError> {
    if let Some(file) = text_at(ctx, paths::ISOLATION_BUILD_LOCK)? {
        let path = ctx.config.cwd.join(file);
        return tokio::fs::read(&path)
            .await
            .map_err(|e| failed(format!("the lock {} cannot be read: {e}", path.display())));
    }
    let mut sources = Vec::new();
    for entry in requirements(ctx)? {
        sources.push(source_of(ctx, supervisor, &entry).await?);
    }
    images
        .lock(&LockRequest { sources })
        .await
        .map_err(|e| failed(e.to_string()))
}

/// The entries of `isolation.build.requirements`; none when the setting is absent.
fn requirements(ctx: &ResolveContext) -> Result<Vec<String>, PreflightError> {
    let Some(found) = ctx.get(&path_of(paths::ISOLATION_BUILD_REQUIREMENTS)?) else {
        return Ok(Vec::new());
    };
    let entries = found.as_value().as_array().ok_or_else(|| {
        failed(format!(
            "`{}` must be a list of paths",
            paths::ISOLATION_BUILD_REQUIREMENTS
        ))
    })?;
    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                // BORROW: `.to_owned()` makes the owned entry the step works with
                .map(str::to_owned)
                .ok_or_else(|| failed("a requirement source must be a string".to_owned()))
        })
        .collect()
}

/// One requirement source: `treatment:<path>` read at the pinned commit, or a path relative to the
/// experiment file.
async fn source_of(
    ctx: &ResolveContext,
    supervisor: &Supervisor,
    entry: &str,
) -> Result<LockSource, PreflightError> {
    let (path, bytes) = if let Some(inside) = entry.strip_prefix(TREATMENT_PREFIX) {
        let treatment: TreatmentRef = read_value(ctx, paths::TREATMENT)?;
        let commit = treatment.commit.ok_or_else(|| {
            failed(format!(
                "`{entry}` needs a treatment pinned to a commit, and the treatment has none"
            ))
        })?;
        let source = ctx
            .config
            .cwd
            .join(required_text(ctx, paths::REQUEST_TREATMENT_SOURCE)?);
        let bytes = show_file(supervisor, &source, &commit, inside).await?;
        (inside, bytes)
    } else {
        let path = ctx.config.cwd.join(entry);
        let bytes = tokio::fs::read(&path).await.map_err(|e| {
            failed(format!(
                "the requirement source {} cannot be read: {e}",
                path.display()
            ))
        })?;
        (entry, bytes)
    };
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| failed(format!("`{entry}` names no file")))?;
    Ok(LockSource {
        name: name.to_owned(), // BORROW: owned name for the request
        bytes,
    })
}

/// Reads the build inputs off the runtime's threads.
async fn read_inputs(dockerfile: PathBuf, context: PathBuf) -> Result<BuildInputs, PreflightError> {
    tokio::task::spawn_blocking(move || BuildInputs::read(&dockerfile, &context))
        .await
        .map_err(|e| failed(format!("the build inputs were not read: {e}")))?
        .map_err(|e| failed(format!("the build inputs cannot be read: {e}")))
}
