//! The container assets the binary carries: the build contexts of the base, env, gateway and
//! test-agent images, embedded at compile time and written once, under a directory named by their
//! digest, below the store root. The binary runs from any directory, and the image tags, which
//! follow the content of a context, do not move with the path it is written to.

use std::path::{Path, PathBuf};

use candler_core::import::digest_of;
use candler_schemas::primitives::Digest;

use super::CliError;

/// Every file of `assets/docker`, by its path below that directory, in path order.
pub(crate) const ASSETS: [(&str, &[u8]); 11] = [
    (
        "base/.dockerignore",
        include_bytes!("../../../../assets/docker/base/.dockerignore"),
    ),
    (
        "base/Dockerfile",
        include_bytes!("../../../../assets/docker/base/Dockerfile"),
    ),
    (
        "env/.dockerignore",
        include_bytes!("../../../../assets/docker/env/.dockerignore"),
    ),
    (
        "env/Dockerfile",
        include_bytes!("../../../../assets/docker/env/Dockerfile"),
    ),
    (
        "gateway/.dockerignore",
        include_bytes!("../../../../assets/docker/gateway/.dockerignore"),
    ),
    (
        "gateway/Dockerfile",
        include_bytes!("../../../../assets/docker/gateway/Dockerfile"),
    ),
    (
        "gateway/entrypoint.sh",
        include_bytes!("../../../../assets/docker/gateway/entrypoint.sh"),
    ),
    (
        "gateway/gw.js",
        include_bytes!("../../../../assets/docker/gateway/gw.js"),
    ),
    (
        "gateway/nginx.conf.template",
        include_bytes!("../../../../assets/docker/gateway/nginx.conf.template"),
    ),
    (
        "testagent/Dockerfile",
        include_bytes!("../../../../assets/docker/testagent/Dockerfile"),
    ),
    (
        "testagent/testagent.sh",
        include_bytes!("../../../../assets/docker/testagent/testagent.sh"),
    ),
];

/// The directory below a store root that holds the written assets.
const CACHE_DIR: &str = "assets";

/// The directory the assets are written to inside a cache entry.
const DOCKER_DIR: &str = "docker";

/// The digest of the embedded assets: the digest of the lines `<path>\0<digest of the bytes>\n`
/// in path order.
///
/// # Errors
/// [`CliError::Failed`] when a digest cannot be formed, which a SHA-256 never does.
pub(crate) fn digest() -> Result<Digest, CliError> {
    let mut lines: Vec<u8> = Vec::new();
    for (path, bytes) in ASSETS {
        let each = digest_of(bytes).map_err(|e| CliError::failed("run", e))?;
        lines.extend_from_slice(path.as_bytes());
        lines.push(0);
        lines.extend_from_slice(each.hex().as_bytes());
        lines.push(b'\n');
    }
    digest_of(lines.as_slice()).map_err(|e| CliError::failed("run", e)) // BORROW: explicit .as_slice()
}

/// The failure of writing the assets at `at`.
fn unwritten(at: &Path, error: &std::io::Error) -> CliError {
    CliError::failed(
        "run",
        std::io::Error::new(
            error.kind(),
            format!("cannot write the image assets to {}: {error}", at.display()),
        ),
    )
}

/// Writes every asset below `dir/docker`.
fn write_all(dir: &Path) -> Result<(), CliError> {
    for (path, bytes) in ASSETS {
        let target = dir.join(DOCKER_DIR).join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| unwritten(parent, &e))?;
        }
        std::fs::write(&target, bytes).map_err(|e| unwritten(&target, &e))?;
    }
    Ok(())
}

/// The directory that holds the assets for `store_root`, written when it is not there yet:
/// `<store root>/assets/<digest>`, with the contexts below its `docker` directory. A directory
/// that is there is left as it is, so a second call writes nothing. The files are written into a
/// hidden temporary directory that is renamed onto the final one, so a half-written directory is
/// never the answer; a rename that finds the directory present lost a race and is not an error.
///
/// # Errors
/// [`CliError::Failed`] when a directory or a file cannot be written.
pub(crate) fn materialise(store_root: &Path) -> Result<PathBuf, CliError> {
    let cache = store_root.join(CACHE_DIR);
    let dir = cache.join(digest()?.hex());
    if dir.join(DOCKER_DIR).is_dir() {
        return Ok(dir);
    }
    std::fs::create_dir_all(&cache).map_err(|e| unwritten(&cache, &e))?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let staging = cache.join(format!(".staging-{}-{nanos}", std::process::id()));
    let written = write_all(&staging);
    let renamed = written.and_then(|()| {
        std::fs::rename(&staging, &dir).or_else(|error| {
            if dir.join(DOCKER_DIR).is_dir() {
                Ok(())
            } else {
                Err(unwritten(&dir, &error))
            }
        })
    });
    if staging.exists() {
        // A leftover staging directory is ours alone: it is named by this process and this instant.
        drop(std::fs::remove_dir_all(&staging));
    }
    renamed?;
    Ok(dir)
}
