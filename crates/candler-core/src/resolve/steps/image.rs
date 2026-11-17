//! The image step: the env lock and the local image the agent runs in.
//!
//! `isolation.build.*` are settings: where the requirement sources, a ready lock and an optional
//! Dockerfile with its context are, and which files the env build prunes. A cell holds the lock's
//! digest (`isolation.env_lock`) and the image built from it (`isolation.image`, an image id,
//! never a tag). The `inputs` entry for the build inputs exists only when the experiment adds its
//! own layer. Changing the build request moves the cell only through those values. The prune list
//! keys the env image with the lock.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use candler_schemas::execution::{DigestRef, GitCommit, TreatmentRef, TreatmentSpec};
use candler_schemas::primitives::{Digest, FieldPath, FormatError, Name, NameToken, plain_segment};
use candler_schemas::values::{JsonValue, ValueSource};
use futures::future::BoxFuture;
use serde_json::Value;

use super::identity::{Extracted, show_file, tree_at};
use super::plugin_environment::{Fragment, STEP_BASE};
use crate::isolation::image::{
    BuildSetting, Extension, ImageService, LockDigest, LockRequest, LockSource, package_name_ok,
};
use crate::resolve::preflight::{PreflightError, PreflightStep, ResolveContext, StepReport};
use crate::supervise::Supervisor;
use crate::tree::BuildContext;
use candler_schemas::execution::paths;

fn failed(detail: String) -> PreflightError {
    PreflightError::Failed { detail }
}

fn path_of(text: &str) -> Result<FieldPath, PreflightError> {
    FieldPath::from_str(text).map_err(|e| failed(e.to_string()))
}

fn text_at(ctx: &ResolveContext, text: &str) -> Result<Option<String>, PreflightError> {
    let Some(found) = ctx.value_at(text)? else {
        return Ok(None);
    };
    found
        .as_str()
        // BORROW: `.to_owned()` makes the owned text the step works with
        .map(|s| Some(s.to_owned()))
        .ok_or_else(|| failed(format!("`{text}` must be a string")))
}

fn read_value<T: serde::de::DeserializeOwned>(
    ctx: &ResolveContext,
    text: &str,
) -> Result<T, PreflightError> {
    let found = ctx
        .value_at(text)?
        .ok_or_else(|| failed(format!("`{text}` has no value")))?;
    // BORROW: `.clone()` hands the JSON to the deserializer, which takes it by value
    serde_json::from_value(found.clone()).map_err(|e| failed(format!("`{text}` is not valid: {e}")))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<JsonValue, PreflightError> {
    let json = serde_json::to_value(value).map_err(|e| failed(e.to_string()))?;
    JsonValue::new(json).map_err(|e| failed(e.to_string()))
}

/// Derives `isolation.env_lock`, `isolation.image` and the `inputs` root: the build inputs, when
/// there is an extension, and the agent inputs the request declares, sorted by kind.
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
            let fragment = fragment_of(&ctx);
            let prune = prune_of(&ctx, fragment.as_ref())?;
            check_record_packages(&ctx)?;
            let lock = lock_bytes(&ctx, fragment.as_ref(), &images, &supervisor).await?;
            let lock_digest = Digest::of_bytes(&lock);
            // The tree of a `treatment:` context lives until the image is built.
            let (extension, _tree) = match extension_of(&ctx, &supervisor).await? {
                Some((layer, tree)) => (Some(layer), tree),
                None => (None, None),
            };
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
            let mut roots: Vec<DigestRef> = declared_inputs(&ctx)?;
            if let Some(inputs) = inputs {
                let digest = inputs.digest().map_err(|e| failed(e.to_string()))?;
                let kind = Name::from_str("build_inputs").map_err(|e| failed(e.to_string()))?;
                roots.push(DigestRef::new(kind, digest));
            }
            if !roots.is_empty() {
                roots.sort();
                report = report.derive(path_of(paths::INPUTS)?, to_json(&roots)?);
            }
            Ok(report)
        })
    }
}

/// The plug-in environment the experiment selects, if any. An environment that cannot be read is
/// the plug-in environment step's to refuse, with its own code; this step goes on without it.
fn fragment_of(ctx: &ResolveContext) -> Option<Fragment> {
    // BORROW: explicit .as_slice()
    Fragment::from_values(ctx.values.as_slice(), ctx.settings.as_slice())
        .ok() // EXPLICIT: an unreadable environment is refused by the plug-in environment step, not here
        .flatten()
}

/// `first` and then `then`, each entry once, at its first place.
fn appended(first: Vec<String>, then: Vec<String>) -> Vec<String> {
    let mut all: Vec<String> = Vec::with_capacity(first.len() + then.len());
    for entry in first.into_iter().chain(then) {
        if !all.contains(&entry) {
            all.push(entry);
        }
    }
    all
}

/// The entries of `isolation.build.prune`: file names, the plug-in environment's first, none
/// when neither sets the setting.
fn prune_of(
    ctx: &ResolveContext,
    fragment: Option<&Fragment>,
) -> Result<Vec<String>, PreflightError> {
    let own = fragment
        .map(|f| f.list(paths::ISOLATION_BUILD_PRUNE))
        .unwrap_or_default();
    Ok(appended(own, written_prune(ctx)?))
}

/// The entries of `isolation.build.prune` the resolved values hold.
fn written_prune(ctx: &ResolveContext) -> Result<Vec<String>, PreflightError> {
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
            let name = entry.as_str().filter(|name| plain_segment(name)); // BORROW: .as_str() reads the entry's text
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

/// The agent inputs the request declares, as `{kind, digest}`; none when it declares none.
fn declared_inputs(ctx: &ResolveContext) -> Result<Vec<DigestRef>, PreflightError> {
    if ctx.get(&path_of(paths::REQUEST_INPUTS)?).is_none() {
        return Ok(Vec::new());
    }
    read_value(ctx, paths::REQUEST_INPUTS)
}

/// The experiment's own layer, when both `isolation.build.dockerfile` and
/// `isolation.build.context` are set, with the extracted tree when the context is
/// `treatment:<path>`.
async fn extension_of(
    ctx: &ResolveContext,
    supervisor: &Supervisor,
) -> Result<Option<(Extension, Option<Extracted>)>, PreflightError> {
    let dockerfile = text_at(ctx, paths::ISOLATION_BUILD_DOCKERFILE)?;
    let context = text_at(ctx, paths::ISOLATION_BUILD_CONTEXT)?;
    match (dockerfile, context) {
        (Some(_), Some(_)) if pair_split(ctx)? => Err(failed(format!(
            "`{}` and `{}` must be set together, by one layer: the plug-in environment sets one \
             and the experiment the other",
            paths::ISOLATION_BUILD_DOCKERFILE,
            paths::ISOLATION_BUILD_CONTEXT
        ))),
        (Some(dockerfile), Some(context)) => {
            let (context, tree) = context_dir(ctx, supervisor, &context).await?;
            Ok(Some((
                Extension {
                    dockerfile: ctx.config.cwd.join(dockerfile),
                    context,
                },
                tree,
            )))
        }
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => Err(failed(format!(
            "`{}` and `{}` must be set together",
            paths::ISOLATION_BUILD_DOCKERFILE,
            paths::ISOLATION_BUILD_CONTEXT
        ))),
    }
}

/// The directory a build context names: a path relative to the experiment file, or, for
/// `treatment:<path>`, `<path>` of the treatment's checkout at the pinned commit, extracted to a
/// scratch directory the returned tree owns.
pub(super) async fn context_dir(
    ctx: &ResolveContext,
    supervisor: &Supervisor,
    text: &str,
) -> Result<(PathBuf, Option<Extracted>), PreflightError> {
    let Some(inside) = text.strip_prefix(paths::TREATMENT_SOURCE_PREFIX) else {
        return Ok((ctx.config.cwd.join(text), None));
    };
    let (source, commit) = checkout_of(ctx, text)?;
    let tree = tree_at(supervisor, &source, &commit, inside).await?;
    Ok((tree.root().to_path_buf(), Some(tree)))
}

/// The treatment's checkout and the commit it is pinned to, for the `treatment:` entry `entry`.
fn checkout_of(ctx: &ResolveContext, entry: &str) -> Result<(PathBuf, GitCommit), PreflightError> {
    let treatment: TreatmentRef = read_value(ctx, paths::TREATMENT)?;
    let commit = treatment.commit.ok_or_else(|| {
        failed(format!(
            "`{entry}` needs a treatment pinned to a commit, and the treatment has none"
        ))
    })?;
    let spec: TreatmentSpec = read_value(ctx, paths::REQUEST_TREATMENT)?;
    let source = spec.source.ok_or_else(|| {
        failed(format!(
            "`{entry}` is read from the treatment's checkout, and the treatment names no \
             `treatment.source`"
        ))
    })?;
    Ok((ctx.config.cwd.join(source), commit))
}

/// Whether the plug-in environment derived the value at `text`.
fn from_plugin(ctx: &ResolveContext, text: &str) -> Result<bool, PreflightError> {
    let path = path_of(text)?;
    Ok(ctx.sources.iter().any(|entry| {
        *entry.path() == path
            && matches!(
                entry.source(),
                ValueSource::Derived { step, .. } if step.name() == STEP_BASE
            )
    }))
}

/// Whether exactly one of the Dockerfile and its context comes from the plug-in environment.
fn pair_split(ctx: &ResolveContext) -> Result<bool, PreflightError> {
    Ok(from_plugin(ctx, paths::ISOLATION_BUILD_DOCKERFILE)?
        != from_plugin(ctx, paths::ISOLATION_BUILD_CONTEXT)?)
}

/// Whether the lock comes from one layer, the plug-in environment or the experiment, and a
/// requirements list from the other: the lock would silently beat the list.
fn lock_split(ctx: &ResolveContext, fragment: Option<&Fragment>) -> Result<bool, PreflightError> {
    let plugin_lock = from_plugin(ctx, paths::ISOLATION_BUILD_LOCK)?;
    let plugin_list =
        fragment.is_some_and(|held| !held.list(paths::ISOLATION_BUILD_REQUIREMENTS).is_empty());
    let own_list =
        !requirements(ctx)?.is_empty() && !from_plugin(ctx, paths::ISOLATION_BUILD_REQUIREMENTS)?;
    Ok(if plugin_lock { own_list } else { plugin_list })
}

/// The env lock: the `isolation.build.lock` file verbatim when it is set, else the lock the image
/// service compiles from `isolation.build.requirements`.
async fn lock_bytes(
    ctx: &ResolveContext,
    fragment: Option<&Fragment>,
    images: &Arc<dyn ImageService>,
    supervisor: &Supervisor,
) -> Result<Vec<u8>, PreflightError> {
    if let Some(file) = text_at(ctx, paths::ISOLATION_BUILD_LOCK)? {
        if lock_split(ctx, fragment)? {
            return Err(failed(format!(
                "`{}` and `{}` must be set together, by one layer: the plug-in environment sets \
                 one and the experiment the other, and the lock would beat the list",
                paths::ISOLATION_BUILD_LOCK,
                paths::ISOLATION_BUILD_REQUIREMENTS
            )));
        }
        let path = ctx.config.cwd.join(file);
        return tokio::fs::read(&path)
            .await
            .map_err(|e| failed(format!("the lock {} cannot be read: {e}", path.display())));
    }
    let own = fragment
        .map(|f| f.list(paths::ISOLATION_BUILD_REQUIREMENTS))
        .unwrap_or_default();
    let mut sources: Vec<LockSource> = Vec::new();
    for entry in appended(own, requirements(ctx)?) {
        let source = source_of(ctx, supervisor, &entry).await?;
        if fragment.is_some() && sources.iter().any(|held| held.name == source.name) {
            return Err(failed(format!(
                "two requirement sources are named `{}`: the compiler reads each under its file name",
                source.name
            )));
        }
        sources.push(source);
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
    let (path, bytes) = if let Some(inside) = entry.strip_prefix(paths::TREATMENT_SOURCE_PREFIX) {
        let (source, commit) = checkout_of(ctx, entry)?;
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
async fn read_inputs(
    dockerfile: PathBuf,
    context: PathBuf,
) -> Result<BuildContext, PreflightError> {
    tokio::task::spawn_blocking(move || BuildContext::read(&dockerfile, &context))
        .await
        .map_err(|e| failed(format!("the build inputs were not read: {e}")))?
        .map_err(|e| failed(format!("the build inputs cannot be read: {e}")))
}
