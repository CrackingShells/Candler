//! The one owner of git and probe boilerplate: a record directory, a supervised probe, `git -C`
//! calls with their deadlines, the archive of a tree at a commit, and the execute bit of an
//! extracted file.
//!
//! Every call here runs through [`Supervisor::run_captured`]. A caller keeps its own caps and maps
//! [`GitError`] or [`ProbeError`] to its own error type; nothing here decides a refusal code.

use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use candler_schemas::primitives::Marker;
use candler_schemas::process::ExitStatus;

use super::Supervisor;
use super::spawn::{SpawnRequest, StdinSpec};
use super::stop::{Captured, Ended};

/// The most bytes of a tree archive read; an archive of this size or more is refused.
pub const ARCHIVE_LIMIT: usize = 64 * 1024 * 1024;

/// The time `git rev-parse` and `git show` get.
pub const READ_DEADLINE: Duration = Duration::from_secs(60);

/// The time `git archive` gets.
pub const ARCHIVE_DEADLINE: Duration = Duration::from_mins(5);

/// Counts the record directories this process made, so two probes never share one.
static RECORDS: AtomicU64 = AtomicU64::new(0);

/// A directory that holds the record of a probe.
#[derive(Debug)]
pub struct RecordDir {
    path: PathBuf,
    remove: bool,
}

impl RecordDir {
    /// A new directory of its own below the system temp directory, removed when dropped.
    ///
    /// # Errors
    /// The error of creating the directory.
    pub fn temp(prefix: &str) -> io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "candler-{prefix}-{}-{nanos}-{}",
            std::process::id(),
            RECORDS.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path, remove: true })
    }

    /// The directory `path`, made when missing and left in place when dropped.
    ///
    /// # Errors
    /// The error of creating the directory.
    pub fn at(path: PathBuf) -> io::Result<Self> {
        std::fs::create_dir_all(&path)?;
        Ok(Self {
            path,
            remove: false,
        })
    }

    /// The directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RecordDir {
    fn drop(&mut self) {
        if self.remove {
            // A failed removal leaves the directory for the temp sweep.
            drop(std::fs::remove_dir_all(&self.path));
        }
    }
}

/// Why a probe could not be run.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProbeError {
    /// The request could not be built.
    #[error("{0}")]
    Request(String),
    /// The supervisor could not run the child.
    #[error("{0}")]
    Run(String),
}

/// A probe to run to its end.
#[derive(Debug, Clone)]
pub struct Probe {
    /// The program and its arguments.
    pub argv: Vec<String>,
    /// The child's whole environment.
    pub env: Vec<(String, String)>,
    /// The marker of the child's record.
    pub marker: Marker,
    /// Bytes written to the child's standard input; none leaves it closed.
    pub stdin: Option<Vec<u8>>,
    /// The time the child gets; none leaves it unbounded.
    pub deadline: Option<Duration>,
    /// The most bytes kept from each output.
    pub limit: usize,
}

/// Runs `probe` to its end with its record in `record`.
///
/// # Errors
/// [`ProbeError`] when the request cannot be built or the child cannot be run. A child that
/// overran its deadline or ended on a signal is an `Ok` whose exit says so.
pub async fn run_probe(
    supervisor: &Supervisor,
    probe: Probe,
    record: &RecordDir,
) -> Result<Captured, ProbeError> {
    let mut builder = SpawnRequest::builder()
        .argv(probe.argv)
        .env(probe.env)
        .marker(probe.marker)
        .record_dir(record.path().to_path_buf()); // BORROW: the request owns its path
    if let Some(bytes) = probe.stdin {
        builder = builder.stdin(StdinSpec::Bytes(bytes));
    }
    if let Some(deadline) = probe.deadline {
        builder = builder.deadline(deadline);
    }
    let request = builder
        .build()
        .map_err(|e| ProbeError::Request(e.to_string()))?;
    supervisor
        .run_captured(request, probe.limit)
        .await
        .map_err(|e| ProbeError::Run(e.to_string()))
}

/// Why a git call did not give an answer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GitError {
    /// The record directory could not be made.
    #[error("cannot make the probe directory: {0}")]
    Record(String),
    /// The request could not be built.
    #[error("{0}")]
    Request(String),
    /// The supervisor could not run git.
    #[error("cannot run git: {0}")]
    Run(String),
    /// A signal ended git.
    #[error("git ended on signal {0}")]
    Signal(i32),
    /// Git overran its deadline.
    #[error("git overran its time")]
    Deadline,
    /// Git exited with a non-zero code.
    #[error("git exited {0}")]
    Failed(i32),
    /// The output reached its cap and is refused.
    #[error("the output reached {0} bytes")]
    TooLarge(usize),
}

/// What a git call that ended by itself printed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput {
    /// The exit code.
    pub code: i32,
    /// The first bytes of standard output, up to the call's limit.
    pub stdout: Vec<u8>,
}

/// The environment of a git child: the path, the C locale, and no prompt for a credential.
fn git_env() -> Vec<(String, String)> {
    vec![
        ("PATH".to_owned(), std::env::var("PATH").unwrap_or_default()), // BORROW: owned pair for the environment
        ("LC_ALL".to_owned(), "C".to_owned()), // BORROW: owned pair for the environment
        ("GIT_TERMINAL_PROMPT".to_owned(), "0".to_owned()), // BORROW: owned pair
    ]
}

/// Runs a git argv to its end and returns its code and the first `limit` bytes of output.
async fn git_argv(
    supervisor: &Supervisor,
    argv: Vec<String>,
    marker: &str,
    limit: usize,
    deadline: Duration,
    record: &RecordDir,
) -> Result<GitOutput, GitError> {
    let marker = Marker::from_str(marker).map_err(|e| GitError::Request(e.to_string()))?;
    let probe = Probe {
        argv,
        env: git_env(),
        marker,
        stdin: None,
        deadline: Some(deadline),
        limit,
    };
    let captured = run_probe(supervisor, probe, record)
        .await
        .map_err(|e| match e {
            ProbeError::Request(text) => GitError::Request(text),
            ProbeError::Run(text) => GitError::Run(text),
        })?;
    if captured.exit.ended == Ended::Deadline {
        return Err(GitError::Deadline);
    }
    match captured.exit.status {
        ExitStatus::Code { code } => Ok(GitOutput {
            code,
            stdout: captured.stdout,
        }),
        ExitStatus::Signal { signal } => Err(GitError::Signal(signal)),
    }
}

/// Runs `git -C source args…` in a record directory of its own, removed when the call ends, and
/// returns its exit code and the first `limit` bytes of standard output.
///
/// # Errors
/// [`GitError`] when git cannot be run, overran `deadline` or ended on a signal. A non-zero code
/// is an `Ok` for the caller to judge.
pub async fn run_git(
    supervisor: &Supervisor,
    source: &Path,
    args: &[&str],
    marker: &str,
    limit: usize,
    deadline: Duration,
) -> Result<GitOutput, GitError> {
    let record = RecordDir::temp("git").map_err(|e| GitError::Record(e.to_string()))?;
    let mut command = vec![
        "git".to_owned(), // BORROW: owned argument for the request
        "-C".to_owned(),  // BORROW: owned argument for the request
        source.to_string_lossy().into_owned(),
    ];
    command.extend(args.iter().map(|arg| (*arg).to_owned())); // BORROW: owned arguments
    git_argv(supervisor, command, marker, limit, deadline, &record).await
}

/// The argv of `git -C source archive --format=tar commit -- subtree`.
#[must_use]
pub fn archive_argv(source: &Path, commit: &str, subtree: &str) -> Vec<String> {
    [
        "git",
        "-C",
        &source.to_string_lossy(),
        "archive",
        "--format=tar",
        commit,
        "--",
        subtree,
    ]
    .iter()
    .map(|word| (*word).to_owned()) // BORROW: the request owns its words
    .collect()
}

/// Runs a git archive argv (from [`archive_argv`] or a harness command) and returns the tar.
/// This is the one place an archive is spawned. `record` holds the child's record; none makes a
/// directory of its own, removed when the call ends.
///
/// # Errors
/// [`GitError::Failed`] for a non-zero exit, [`GitError::TooLarge`] for a tar of
/// [`ARCHIVE_LIMIT`] bytes or more, and the errors of [`run_git`].
pub async fn tree_at_commit(
    supervisor: &Supervisor,
    argv: Vec<String>,
    marker: &str,
    record: Option<&RecordDir>,
) -> Result<Vec<u8>, GitError> {
    capped_archive(supervisor, argv, marker, record, ARCHIVE_LIMIT).await
}

async fn capped_archive(
    supervisor: &Supervisor,
    argv: Vec<String>,
    marker: &str,
    record: Option<&RecordDir>,
    limit: usize,
) -> Result<Vec<u8>, GitError> {
    let own;
    let record = if let Some(record) = record {
        record
    } else {
        own = RecordDir::temp("archive").map_err(|e| GitError::Record(e.to_string()))?;
        &own
    };
    let out = git_argv(supervisor, argv, marker, limit, ARCHIVE_DEADLINE, record).await?;
    if out.code != 0 {
        return Err(GitError::Failed(out.code));
    }
    if out.stdout.len() >= limit {
        return Err(GitError::TooLarge(limit));
    }
    Ok(out.stdout)
}

/// Sets the execute bit (0755) of a file written from a tree.
///
/// # Errors
/// The error of setting the mode.
pub fn mark_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::Command;
    use std::sync::Arc;

    use candler_schemas::execution::EventPayload;
    use candler_schemas::primitives::RunKey;

    use super::*;
    use crate::supervise::EventSink;

    struct Silent;

    impl EventSink for Silent {
        fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
    }

    fn supervisor() -> Supervisor {
        Supervisor::new(Arc::new(Silent))
    }

    /// A repository with one commit holding `dir/a.txt` of `size` bytes.
    fn repo(size: usize) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir(dir.path().join("dir")).expect("subdir");
        std::fs::write(dir.path().join("dir/a.txt"), vec![b'x'; size]).expect("file");
        let run = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args([
                    "-c",
                    "user.name=t",
                    "-c",
                    "user.email=t@t",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .output()
                .expect("git");
            assert!(out.status.success(), "{args:?}");
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        run(&["init", "-q"]);
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "c"]);
        let commit = run(&["rev-parse", "HEAD"]);
        (dir, commit)
    }

    /// Guards: C1 — `run_git` prints what git printed and its code, and a non-zero code is an Ok.
    /// Oracle: the commit the fixture repository made. Bite: the code dropped or stdout empty.
    #[tokio::test]
    async fn c1_run_git_returns_output_and_code() {
        let (dir, commit) = repo(4);
        let sup = supervisor();
        let ok = run_git(
            &sup,
            dir.path(),
            &["rev-parse", "HEAD"],
            "t",
            4096,
            READ_DEADLINE,
        )
        .await
        .expect("git runs");
        assert_eq!(ok.code, 0);
        assert_eq!(String::from_utf8_lossy(&ok.stdout).trim(), commit);
        let bad = run_git(
            &sup,
            dir.path(),
            &["rev-parse", "--verify", "--quiet", "nope"],
            "t",
            4096,
            READ_DEADLINE,
        )
        .await
        .expect("git runs");
        assert_ne!(bad.code, 0);
    }

    /// Guards: C1 — a git call that overruns its deadline is `GitError::Deadline`. Oracle: an
    /// alias that sleeps for a minute, given 300 ms. Bite: the deadline not set.
    #[tokio::test]
    async fn c1_a_slow_git_call_is_a_deadline_error() {
        let (dir, _) = repo(4);
        let slow = run_git(
            &supervisor(),
            dir.path(),
            &["-c", "alias.slow=!sleep 60", "slow"],
            "t",
            4096,
            Duration::from_millis(300),
        )
        .await;
        assert_eq!(slow, Err(GitError::Deadline));
    }

    /// Guards: C1 — the archive cap: a tar below the limit is returned; a tar of exactly the limit
    /// is refused (`>=`); a failing archive is `Failed`. Oracle: the size of the real tar, read
    /// first without a cap. Bite: `>=` loosened to `>`.
    #[tokio::test]
    async fn c1_archive_refuses_a_tar_at_the_limit() {
        let (dir, commit) = repo(3000);
        let sup = supervisor();
        let argv = || archive_argv(dir.path(), &commit, "dir");
        let tar = tree_at_commit(&sup, argv(), "t", None)
            .await
            .expect("archive");
        let size = tar.len();
        let below = capped_archive(&sup, argv(), "t", None, size + 1).await;
        assert_eq!(below.map(|t| t.len()), Ok(size));
        let at = capped_archive(&sup, argv(), "t", None, size).await;
        assert_eq!(at, Err(GitError::TooLarge(size)));
        let missing =
            tree_at_commit(&sup, archive_argv(dir.path(), &commit, "absent"), "t", None).await;
        assert!(matches!(missing, Err(GitError::Failed(_))), "{missing:?}");
    }

    /// Guards: C1 — a temp record directory is removed on drop and an `at` directory is kept;
    /// the execute bit is 0755. Oracle: the file system. Bite: `remove` always false.
    #[test]
    fn c1_record_dir_lifecycle_and_execute_bit() {
        let temp = RecordDir::temp("t").expect("temp");
        let path = temp.path().to_path_buf();
        assert!(path.is_dir());
        drop(temp);
        assert!(!path.exists());
        let keep = tempfile::tempdir().expect("dir");
        let kept = RecordDir::at(keep.path().join("r")).expect("at");
        let kept_path = kept.path().to_path_buf();
        drop(kept);
        assert!(kept_path.is_dir());
        let file = keep.path().join("f");
        std::fs::write(&file, b"x").expect("file");
        mark_executable(&file).expect("mode");
        let mode = std::fs::metadata(&file).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}
