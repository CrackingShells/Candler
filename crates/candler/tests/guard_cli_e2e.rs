//! `candler kill --all -y` against a real Docker daemon.
//!
//! The test makes its own jobs, each with its own `candler.job` label, and its own containers
//! from the testagent image. The binary reaches docker through a forwarding shim that allows a
//! destroy only of an id the test created and refuses everything else, and the test destroys what
//! it leaves behind only through the container guard over an allowlist of the ids it captured.
//! No call removes a container by name, label or prefix.
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::Arc;

use candler_core::isolation::container::guard::{ContainerGuard, LostProof};
use candler_core::isolation::docker::{ClientEnv, DockerCli};
use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
use candler_core::isolation::model::{LabelSel, NetworkName};
use candler_core::isolation::testing::{AllowlistDriver, FakeJobView};
use candler_core::jobs::dir::JobDir;
use candler_core::jobs::events::JobEvents;
use candler_core::jobs::lock::JobLock;
use candler_core::jobs::lost::{JobLiveness, classify};
use candler_core::store::Store;
use candler_core::store::layout::StoreLayout;
use candler_core::supervise::{EventSink, Supervisor};
use candler_schemas::execution::{EventPayload, JobEnd, JobRecord, JobState, SupervisorIdentity};
use candler_schemas::primitives::{Digest, JobId, RunKey};
use candler_schemas::process::ProcessStart;

/// The image the testagent scaffold builds; it is not pulled.
const IMAGE: &str = "candler-testagent:test";

/// The forwarding client. It is written beside its allowlist (`allow`, one id or network name per
/// line) and its call log (`docker.log`), which it finds from its own path because the docker
/// client gives its child no variable of the test. It uses no command but the shell's own, since
/// the child has no `PATH`. It restores the `HOME` the test had, which the binary under test
/// does not, so that the real client finds its context; it refuses to run if the real client it
/// forwards to is itself. A read verb passes through. A destroy verb passes only when it names
/// at least one id and every id it names is a line of the allowlist. Anything else, including a
/// global option before the verb and any `-H`, `--host` or `--context` anywhere, is refused:
/// exit status 97 and a `REFUSE` line in the log.
const SHIM: &str = r#"#!/bin/sh
case "$0" in
/*) ;;
*) exit 98 ;;
esac
real='@REAL@'
@HOME@
[ "$real" != "$0" ] || exit 99
[ ! "$real" -ef "$0" ] || exit 99
here="${0%/*}"
log="$here/docker.log"
allow="$here/allow"
all="$*"

refuse() {
    printf 'REFUSE %s\n' "$all" >> "$log"
    printf 'docker shim: refused: %s\n' "$all" >&2
    exit 97
}

listed() {
    [ -n "$1" ] || return 1
    [ -f "$allow" ] || return 1
    while IFS= read -r line || [ -n "$line" ]; do
        [ "$line" = "$1" ] && return 0
    done < "$allow"
    return 1
}

# check_destroy <stop|kill|rm> <words of the verb> <the whole command line>
check_destroy() {
    kind=$1
    skip=$2
    shift 2
    shift "$skip"
    ids=0
    while [ $# -gt 0 ]; do
        case "$1" in
        -f | --force | -v | --volumes | -fv | -vf) ;;
        -t | --time)
            [ "$kind" = stop ] || refuse
            shift
            [ $# -gt 0 ] || refuse
            ;;
        -s | --signal)
            [ "$kind" = kill ] || refuse
            shift
            [ $# -gt 0 ] || refuse
            ;;
        -*) refuse ;;
        *)
            listed "$1" || refuse
            ids=$((ids + 1))
            ;;
        esac
        shift
    done
    [ "$ids" -gt 0 ] || refuse
}

for arg in "$@"; do
    case "$arg" in
    -H* | --host | --host=* | --context | --context=*) refuse ;;
    esac
done

case "$1" in
-*) refuse ;;
container)
    case "$2" in
    ls | list | ps | inspect) ;;
    stop) check_destroy stop 2 "$@" ;;
    kill) check_destroy kill 2 "$@" ;;
    rm) check_destroy rm 2 "$@" ;;
    *) refuse ;;
    esac
    ;;
network)
    case "$2" in
    ls | inspect) ;;
    rm) check_destroy rm 2 "$@" ;;
    *) refuse ;;
    esac
    ;;
image)
    case "$2" in
    inspect) ;;
    *) refuse ;;
    esac
    ;;
ps | inspect) ;;
stop) check_destroy stop 1 "$@" ;;
kill) check_destroy kill 1 "$@" ;;
rm) check_destroy rm 1 "$@" ;;
*) refuse ;;
esac
printf 'ALLOW %s\n' "$all" >> "$log"
exec "$real" "$@"
"#;

/// The shim installed in a directory of its own, with its allowlist and call log beside it.
struct Shim {
    dir: PathBuf,
}

impl Shim {
    /// Writes the shim into `dir`, forwarding to the docker at `real`, allowing the destroy of
    /// exactly the ids in `allowed`.
    fn install(dir: &Path, real: &Path, allowed: &[String]) -> Self {
        fs::create_dir_all(dir).unwrap();
        let real_text = real.to_str().unwrap();
        assert!(
            !real_text.contains('\''),
            "the real docker path is quotable"
        );
        let script = dir.join("docker");
        assert_ne!(real, script.as_path(), "the shim would forward to itself");
        let home = std::env::var("HOME").unwrap_or_default();
        assert!(!home.contains('\''), "HOME is quotable");
        let home_line = if home.is_empty() {
            String::new()
        } else {
            format!("export HOME='{home}'")
        };
        let text = SHIM
            .replace("@REAL@", real_text)
            .replace("@HOME@", &home_line);
        fs::write(&script, text).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.join("docker.log"), "").unwrap();
        let shim = Self {
            dir: dir.to_path_buf(), // BORROW: the shim owns its directory
        };
        shim.allow(allowed);
        shim
    }

    /// Replaces the allowlist with exactly the lines of `allowed`.
    fn allow(&self, allowed: &[String]) {
        let list = allowed.join("\n") + if allowed.is_empty() { "" } else { "\n" };
        fs::write(self.dir.join("allow"), list).unwrap();
    }

    /// What the shim was asked and decided, one call per line.
    fn log(&self) -> String {
        fs::read_to_string(self.dir.join("docker.log")).unwrap()
    }

    /// The lines of the log that record a refusal.
    fn refusals(&self) -> Vec<String> {
        self.log()
            .lines()
            .filter(|line| line.starts_with("REFUSE"))
            .map(str::to_owned) // BORROW: .to_owned() gives each line its own text
            .collect()
    }
}

/// The first absolute `docker` on this process's `PATH`.
fn real_docker() -> PathBuf {
    let path = std::env::var_os("PATH").expect("a PATH");
    std::env::split_paths(&path)
        .map(|dir| dir.join("docker"))
        .find(|candidate| candidate.is_absolute() && candidate.is_file())
        .expect("an absolute docker on PATH")
}

/// Runs the real docker for a read or a creation the test makes; it is never a destroy.
fn docker(real: &Path, args: &[&str]) -> Output {
    Command::new(real).args(args).output().unwrap()
}

/// Whether docker has the `kind` (`container` or `network`) `id`.
fn exists(real: &Path, kind: &str, id: &str) -> bool {
    let out = docker(real, &[kind, "inspect", "--format", "{{.Id}}", id]);
    if out.status.success() {
        return true;
    }
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        stderr.contains("No such") || stderr.contains("not found"),
        "{kind} inspect failed: {stderr}"
    );
    false
}

/// A job id no other run uses: 48 random bits fill the digits a network name takes from it.
fn unique_job() -> JobId {
    use std::hash::{BuildHasher as _, Hasher as _};
    let random = || {
        std::collections::hash_map::RandomState::new()
            .build_hasher()
            .finish()
    };
    let (a, b, c) = (random(), random(), random());
    format!(
        "{:08x}-{:04x}-7000-8000-{:012x}",
        a & 0xffff_ffff,
        b & 0xffff,
        c & 0xffff_ffff_ffff
    )
    .parse()
    .unwrap()
}

/// Drops every event.
struct Silent;

impl EventSink for Silent {
    fn emit(&self, _run: Option<&RunKey>, _payload: EventPayload) {}
}

/// Something the test made in docker, captured at creation.
enum Made {
    Container { job: JobId, cid: String },
    Network { job: JobId, name: NetworkName },
}

/// Destroys what the test made and left, by exact id, through the guard, when it is dropped.
#[derive(Default)]
struct Cleanup(Vec<Made>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        if self.0.is_empty() {
            return;
        }
        let made = &self.0;
        std::thread::scope(|scope| {
            let _ = scope
                .spawn(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .expect("a runtime");
                    runtime.block_on(destroy_made(made));
                })
                .join();
        });
    }
}

/// Destroys each of `made` that docker still has, through a guard whose driver refuses any id
/// the test did not create. A job is proven lost by a lock on a directory of its own.
async fn destroy_made(made: &[Made]) {
    let scratch = tempfile::tempdir().expect("a scratch directory");
    let control = scratch.path().join("control");
    fs::create_dir_all(&control).expect("a control directory");
    // TRAIT_OBJECT: the supervisor takes its sink as a trait object
    let sink: Arc<dyn EventSink> = Arc::new(Silent);
    let cli = DockerCli::new(
        Arc::new(Supervisor::new(sink)),
        ClientEnv::from_process(),
        control,
    )
    .expect("a docker client");
    let destroyer = Arc::new(AllowlistDriver::new(cli.destroyer_for_tests()));
    let mut jobs: BTreeMap<String, JobId> = BTreeMap::new();
    for item in made {
        match item {
            Made::Container { job, cid } => {
                destroyer.allow_container(cid);
                jobs.insert(job.to_string(), job.clone()); // BORROW: .clone() gives the map its own job
            }
            Made::Network { job, name } => {
                destroyer.allow_network(name.as_str()); // BORROW: .as_str() names the network
                jobs.insert(job.to_string(), job.clone()); // BORROW: .clone() gives the map its own job
            }
        }
    }
    // TRAIT_OBJECT: the guard holds its drivers as trait objects
    let destroy: Arc<dyn DestroyDriver> = destroyer.clone(); // BORROW: .clone() shares the allowlist
    // TRAIT_OBJECT: the guard holds its drivers as trait objects
    let reads: Arc<dyn DockerDriver> = Arc::new(cli);
    let guard = ContainerGuard::new(
        destroy,
        Arc::clone(&reads),
        FakeJobView::new(unique_job()).shared(),
    );
    for job in jobs.values() {
        let path = scratch.path().join("jobs").join(job.to_string());
        fs::create_dir_all(&path).expect("a job directory");
        let dir = JobDir::new(job.clone(), path); // BORROW: .clone() gives the directory its own job
        let Ok(lock) = JobLock::acquire(&dir) else {
            eprintln!("cleanup: the lock of {job} is not ours");
            continue;
        };
        let proof = LostProof::held(&lock);
        let sel = LabelSel {
            key: String::from("candler.job"),
            value: job.to_string(),
        };
        for container in reads.ps_by_label(&sel).await.unwrap_or_default() {
            let ours = made.iter().any(|item| {
                matches!(item, Made::Container { cid, .. } if cid.as_str() == container.cid.as_str()) // BORROW: .as_str() compares ids as text
            });
            if !ours {
                continue;
            }
            if let Err(error) = guard.destroy_lost(&container, &proof).await {
                eprintln!("cleanup: {} was not destroyed: {error}", container.cid);
            }
        }
        for item in made {
            if let Made::Network { job: owner, name } = item
                && owner == job
                && let Err(error) = guard.destroy_network_lost(name, &proof).await
            {
                eprintln!("cleanup: {} was not destroyed: {error}", name.as_str()); // BORROW: .as_str() names the network
            }
        }
    }
}

/// Starts a detached testagent container of `job`, on `network` when given, and returns its full
/// id, which is recorded for cleanup at once.
fn start(real: &Path, cleanup: &mut Cleanup, job: &JobId, network: Option<&NetworkName>) -> String {
    let mut command = Command::new(real);
    command
        .args(["run", "--detach", "--stop-timeout", "1", "--label"])
        .arg(format!("candler.job={job}"))
        .args([
            "--label",
            "candler.role=agent",
            "--label",
            "candler.test=kill_e2e",
        ]);
    if let Some(name) = network {
        command.args(["--network", name.as_str()]); // BORROW: .as_str() names the network
    }
    let output = command.args([IMAGE, "sleep-forever"]).output().unwrap();
    assert!(
        output.status.success(),
        "docker run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let cid = String::from_utf8_lossy(&output.stdout).trim().to_owned(); // BORROW: .to_owned() gives the id its own text
    assert_eq!(cid.len(), 64, "docker run printed a full id: {cid:?}");
    cleanup.0.push(Made::Container {
        job: job.clone(), // BORROW: .clone() gives the record its own job
        cid: cid.clone(), // BORROW: .clone() gives the record its own id
    });
    cid
}

/// Creates the network of `job` and returns its name and full id.
fn network(real: &Path, cleanup: &mut Cleanup, job: &JobId) -> (NetworkName, String) {
    let name = NetworkName::for_job(job);
    let output = Command::new(real)
        .args(["network", "create", "--internal", "--label"])
        .arg(format!("candler.job={job}"))
        .args(["--label", "candler.test=kill_e2e"])
        .arg(name.as_str()) // BORROW: .as_str() names the network
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "docker network create failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let id = String::from_utf8_lossy(&output.stdout).trim().to_owned(); // BORROW: .to_owned() gives the id its own text
    assert_eq!(id.len(), 64, "docker network create printed a full id");
    cleanup.0.push(Made::Network {
        job: job.clone(), // BORROW: .clone() gives the record its own job
        name: NetworkName::for_job(job),
    });
    (name, id)
}

/// Makes the job `id` in the store: a record, `job.started`, and `end` when the job ended. The
/// lock is never taken, so a job without an end is lost.
fn make_job(layout: &StoreLayout, id: &JobId, end: Option<JobEnd>) {
    let record = JobRecord::builder()
        .id(id.clone()) // BORROW: the record owns its id
        .supervisor(SupervisorIdentity::new(1, ProcessStart::darwin(1, 0)).unwrap())
        .spec_digest(
            format!("sha256:{}", "0".repeat(64))
                .parse::<Digest>()
                .unwrap(),
        )
        .state(if end.is_some() {
            JobState::Ended
        } else {
            JobState::Running
        })
        .build()
        .unwrap();
    let dir = JobDir::create(layout, id.clone(), &record).unwrap(); // BORROW: .clone() gives the directory its own id
    let events = JobEvents::open(&dir).unwrap();
    events.append(None, EventPayload::job_started()).unwrap();
    if let Some(end) = end {
        events.append(None, EventPayload::job_ended(end)).unwrap();
    }
}

mod kill_e2e {
    use super::*;

    /// Guards: B40 — `candler kill --all -y` destroys the containers and the network of every
    /// lost job, and only theirs, on a real daemon.
    ///
    /// Oracle: `docker container inspect` and `docker network inspect` by the ids captured at
    /// creation, the binary's output lines, the jobs' own logs read back through `classify`, and
    /// the shim's call log. The shim allows a destroy of the lost jobs' ids and refuses every
    /// other, so a kill that reached the ended job's or the bystander's container would show as
    /// a refusal.
    ///
    /// Bite: `kill` stops only the first target, so the second lost job keeps its container and
    /// network.
    #[test]
    #[ignore = "needs a Docker daemon and the candler-testagent:test image; run by the container-e2e group"]
    fn b40_kill_all_destroys_lost_jobs_containers() {
        let real = real_docker();
        let scratch = tempfile::tempdir().unwrap();
        let mut cleanup = Cleanup::default();
        let (first_lost, second_lost) = (unique_job(), unique_job());
        let (ended, bystander) = (unique_job(), unique_job());

        let store = scratch.path().join("store");
        Store::init(&store).unwrap();
        let layout = StoreLayout::new(&store);
        make_job(&layout, &first_lost, None);
        make_job(&layout, &second_lost, None);
        make_job(&layout, &ended, Some(JobEnd::Completed));

        // The binary reads no store but the test's own, and no configuration or state of the
        // user's: the first call, without `-y`, must be refused and list exactly the test's jobs.
        let candler = |args: &[&str], shim: &Shim| {
            Command::new(env!("CARGO_BIN_EXE_candler"))
                .args(args)
                .arg("--store")
                .arg(&store)
                .current_dir(scratch.path())
                .env_remove("CANDLER_STORE")
                .env("HOME", scratch.path())
                .env("XDG_CONFIG_HOME", scratch.path().join("xdg-config"))
                .env("XDG_DATA_HOME", scratch.path().join("xdg-data"))
                .env("XDG_STATE_HOME", scratch.path().join("xdg-state"))
                .env("XDG_CACHE_HOME", scratch.path().join("xdg-cache"))
                .env("PATH", format!("{}:/usr/bin:/bin", shim.dir.display()))
                .stdin(Stdio::null())
                .output()
                .unwrap()
        };
        let shim = Shim::install(&scratch.path().join("bin"), &real, &[]);
        let dry = candler(&["kill", "--all"], &shim);
        let dry_out = String::from_utf8_lossy(&dry.stdout).into_owned();
        let mut own: Vec<String> = [&first_lost, &second_lost, &ended]
            .iter()
            .map(ToString::to_string)
            .collect();
        own.sort();
        assert_eq!(
            dry_out.lines().next(),
            Some(format!("would stop: {}", own.join(" ")).as_str()),
            "the store holds jobs the test did not make: {dry_out}"
        );
        assert_eq!(dry.status.code(), Some(2), "the dry run was not refused");
        assert_eq!(shim.log(), "", "the dry run made a docker call");

        let (first_net, first_net_id) = network(&real, &mut cleanup, &first_lost);
        let (second_net, second_net_id) = network(&real, &mut cleanup, &second_lost);
        let first_cid = start(&real, &mut cleanup, &first_lost, Some(&first_net));
        let second_cid = start(&real, &mut cleanup, &second_lost, Some(&second_net));
        let cid_ended = start(&real, &mut cleanup, &ended, None);
        let cid_bystander = start(&real, &mut cleanup, &bystander, None);

        // The ids of the two lost jobs' containers and networks, with the names docker is asked
        // for a network by, and the egress names of the same jobs.
        let allowed = vec![
            first_cid.clone(),              // BORROW: .clone() gives the list its own id
            second_cid.clone(),             // BORROW: .clone() gives the list its own id
            first_net_id.clone(),           // BORROW: .clone() gives the list its own id
            second_net_id.clone(),          // BORROW: .clone() gives the list its own id
            first_net.as_str().to_owned(),  // BORROW: .to_owned() gives the list its own name
            second_net.as_str().to_owned(), // BORROW: .to_owned() gives the list its own name
            NetworkName::egress_for_job(&first_lost).as_str().to_owned(), // BORROW: .to_owned() gives the list its own name
            NetworkName::egress_for_job(&second_lost)
                .as_str()
                .to_owned(), // BORROW: .to_owned() gives the list its own name
        ];
        shim.allow(&allowed);
        let out = candler(&["kill", "--all", "-y"], &shim);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(0), "{stdout}{stderr}");

        for job in [&first_lost, &second_lost] {
            let wanted = format!("{job}: recovered (1 containers destroyed)");
            let found = stdout.lines().filter(|line| *line == wanted).count();
            assert_eq!(found, 1, "no single `{wanted}` line in: {stdout}");
        }
        assert!(
            stdout
                .lines()
                .any(|line| line.starts_with(&format!("{ended}: "))
                    && line.ends_with("(left alone)")),
            "the ended job was not left alone: {stdout}"
        );

        for cid in [&first_cid, &second_cid] {
            assert!(!exists(&real, "container", cid), "{cid} survived the kill");
        }
        for id in [&first_net_id, &second_net_id] {
            assert!(
                !exists(&real, "network", id),
                "network {id} survived the kill"
            );
        }
        assert!(
            exists(&real, "container", &cid_ended),
            "the ended job's container is gone"
        );
        assert!(
            exists(&real, "container", &cid_bystander),
            "the bystander's container is gone"
        );

        for job in [&first_lost, &second_lost] {
            let dir = JobDir::open(&layout, job).unwrap();
            assert_eq!(
                classify(&dir).unwrap(),
                JobLiveness::Ended(JobEnd::Failed),
                "{job} is not ended (failed)"
            );
        }

        assert_eq!(
            shim.refusals(),
            Vec::<String>::new(),
            "the shim refused a call"
        );
        let log = shim.log();
        for cid in [&first_cid, &second_cid] {
            assert!(
                log.lines()
                    .any(|line| line.starts_with("ALLOW container rm")
                        && line.contains(cid.as_str())), // BORROW: .as_str() compares the id as text
                "no destroy of {cid} went through the shim: {log}"
            );
        }
    }
}

mod shim_refusals {
    use super::*;

    /// The words of `line`, with the stand-ins for the test's ids put in.
    fn words(line: &str) -> Vec<String> {
        line.split_whitespace()
            .map(|word| match word {
                "ID" => "a".repeat(64),
                "OTHER" => "b".repeat(64),
                "IDPREFIX" => "a".repeat(12),
                "NET" => "candler-0123456789ab-net".to_owned(), // BORROW: .to_owned() gives the word its own text
                "ONET" => "candler-ba9876543210-net".to_owned(), // BORROW: .to_owned() gives the word its own text
                other => other.to_owned(), // BORROW: .to_owned() gives the word its own text
            })
            .collect()
    }

    /// Runs the shim with no environment, as the docker client does.
    fn run(shim: &Shim, args: &[String]) -> Output {
        Command::new(shim.dir.join("docker"))
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// A stand-in for the real docker that records each call in `real.log` beside it, and the
    /// path of that log.
    fn fake_real(dir: &Path) -> (PathBuf, PathBuf) {
        fs::create_dir_all(dir).unwrap();
        let script = dir.join("docker");
        fs::write(
            &script,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"${0%/*}/real.log\"\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        (script, dir.join("real.log"))
    }

    fn calls(log: &Path) -> Vec<String> {
        fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned) // BORROW: .to_owned() gives each line its own text
            .collect()
    }

    /// Guards: the shim B40 runs the binary behind allows a destroy only of an allowlisted id and
    /// fails closed on every other.
    ///
    /// Oracle: the exit status of the shim, its own log, and the calls a stand-in for the real
    /// docker recorded. Every refusal has an allowed twin that names the allowlisted id, so a
    /// refusal is the shim's decision about the id and not about the verb.
    ///
    /// Bite: the shim passes every destroy verb through, so the destroys of ids outside the
    /// allowlist reach the real docker.
    #[test]
    fn the_shim_refuses_a_destroy_of_an_id_outside_the_allowlist() {
        let scratch = tempfile::tempdir().unwrap();
        let (real, real_log) = fake_real(&scratch.path().join("real"));
        let allowed = words("ID NET");
        let shim = Shim::install(&scratch.path().join("bin"), &real, &allowed);

        let cases: &[(&str, bool)] = &[
            (
                "container ls --all --no-trunc --quiet --filter label=candler.job=x",
                true,
            ),
            ("container inspect --format x OTHER", true),
            ("network inspect --format x NET", true),
            ("image inspect --format x busybox", true),
            ("ps -a", true),
            ("container stop -t 1 ID", true),
            ("container stop -t 1 OTHER", false),
            ("container rm -v ID", true),
            ("container rm -v OTHER", false),
            ("container rm -f ID", true),
            ("container rm -f OTHER", false),
            ("container kill -s KILL ID", true),
            ("container kill -s KILL OTHER", false),
            ("rm ID", true),
            ("rm -f -v ID", true),
            ("rm ID ID", true),
            ("rm OTHER", false),
            ("rm -f OTHER", false),
            ("stop ID", true),
            ("stop OTHER", false),
            ("kill ID", true),
            ("kill OTHER", false),
            ("network rm NET", true),
            ("network rm -f NET", true),
            ("network rm ONET", false),
            ("network rm OTHER", false),
            ("rm ID OTHER", false),
            ("rm OTHER ID", false),
            ("container rm -v ID OTHER", false),
            ("network rm NET ONET", false),
            ("rm IDPREFIX", false),
            ("rm", false),
            ("rm -f", false),
            ("container rm", false),
            ("container rm -t 1 ID", false),
            ("container rm --force=true ID", false),
            ("container rm -- ID", false),
            ("stop -s KILL ID", false),
            ("rmi ID", false),
            ("image rm ID", false),
            ("container prune -f", false),
            ("system prune -f", false),
            ("network prune -f", false),
            ("volume rm ID", false),
            ("-H unix:///x rm ID", false),
            ("container rm -v ID -H unix:///x", false),
            ("rm --host unix:///x ID", false),
            ("rm --context x ID", false),
            ("ps --context=x", false),
            ("-H unix:///x rm OTHER", false),
            ("--context x rm OTHER", false),
            ("run --rm busybox true", false),
            ("exec ID true", false),
            ("frobnicate", false),
        ];
        let mut expected_log = Vec::new();
        let mut expected_real = Vec::new();
        for (line, allowed_call) in cases {
            let args = words(line);
            let out = run(&shim, &args);
            let text = args.join(" ");
            if *allowed_call {
                assert!(
                    out.status.success(),
                    "`{line}` was refused: {}",
                    String::from_utf8_lossy(&out.stderr)
                );
                expected_log.push(format!("ALLOW {text}"));
                expected_real.push(text);
            } else {
                assert_eq!(out.status.code(), Some(97), "`{line}` was not refused");
                expected_log.push(format!("REFUSE {text}"));
            }
        }
        assert_eq!(
            shim.log().lines().map(str::to_owned).collect::<Vec<_>>(),
            expected_log
        ); // BORROW: .to_owned() gives each line its own text
        assert_eq!(
            calls(&real_log),
            expected_real,
            "a refused call reached the real docker"
        );

        // An empty argument is no id.
        let before = calls(&real_log).len();
        let out = run(&shim, &["rm".to_owned(), String::new()]); // BORROW: .to_owned() gives the argument its own text
        assert_eq!(out.status.code(), Some(97));
        assert_eq!(calls(&real_log).len(), before);

        // The shim reaches the real docker by its absolute path, so a `PATH` led by the shim does
        // not make it call itself.
        let out = Command::new(shim.dir.join("docker"))
            .arg("ps")
            .env_clear()
            .env("PATH", &shim.dir)
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(calls(&real_log).len(), before + 1);
    }

    /// Guards: an empty allowlist and a missing one refuse every destroy, and still let a read
    /// through.
    ///
    /// Oracle: the exit status of the shim and the calls the stand-in for the real docker
    /// recorded.
    ///
    /// Bite: the shim treats a missing or empty allowlist as no restriction.
    #[test]
    fn the_shim_with_no_allowlist_refuses_every_destroy() {
        let scratch = tempfile::tempdir().unwrap();
        let (real, real_log) = fake_real(&scratch.path().join("real"));
        let empty = Shim::install(&scratch.path().join("empty"), &real, &[]);
        let missing = Shim::install(&scratch.path().join("missing"), &real, &[]);
        fs::remove_file(missing.dir.join("allow")).unwrap();
        for shim in [&empty, &missing] {
            for line in [
                "rm ID",
                "container rm -v ID",
                "network rm NET",
                "stop ID",
                "kill ID",
            ] {
                let out = run(shim, &words(line));
                assert_eq!(out.status.code(), Some(97), "`{line}` was not refused");
            }
            assert!(run(shim, &words("ps -a")).status.success());
        }
        assert_eq!(calls(&real_log), ["ps -a", "ps -a"]);
    }
}
