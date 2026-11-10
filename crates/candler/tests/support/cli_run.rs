//! The world `candler run` is tried in: a PATH-fake docker client that keeps a small daemon's
//! state in files beside it, a git repository holding the skills, a store, and one experiment
//! of one cell. No real container exists; the fake's call log is the record of what was asked.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The fake docker client. Its state and log are found from the script's own path, because the
/// client gives its child no variable of the test. Every call is logged, one per line.
const FAKE: &str = r#"#!/bin/sh
here="${0%/*}"
st="$here/state"
mkdir -p "$st/c" "$st/n"
printf '%s\n' "$*" >> "$here/docker.log"
img_id="sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
layer="sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
# The labels of a call, one k=v per line, from its --label arguments.
labels_of() {
    want=0
    for a in "$@"; do
        if [ "$want" = 1 ]; then printf '%s\n' "$a"; want=0; fi
        if [ "$a" = "--label" ]; then want=1; fi
    done
}
json_labels() {
    first=1
    printf '{'
    while IFS= read -r l; do
        [ -z "$l" ] && continue
        k="${l%%=*}"; v="${l#*=}"
        [ "$first" = 1 ] || printf ','
        first=0
        printf '"%s":"%s"' "$k" "$v"
    done
    printf '}'
}
cidfile_of() {
    want=0
    for a in "$@"; do
        if [ "$want" = 1 ]; then printf '%s' "$a"; return; fi
        if [ "$a" = "--cidfile" ]; then want=1; fi
    done
}
new_container() {
    id=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
    labels_of "$@" > "$st/c/$id.labels"
    echo running > "$st/c/$id.state"
    cf=$(cidfile_of "$@")
    [ -n "$cf" ] && printf '%s' "$id" > "$cf"
    printf '%s' "$id" >> "$here/handed.log"; printf '\n' >> "$here/handed.log"
    NEW="$id"
}
inspect_container() {
    id="$1"
    if [ ! -f "$st/c/$id.state" ]; then
        echo "Error: No such container: $id" >&2
        exit 1
    fi
    s=$(cat "$st/c/$id.state")
    run=false; [ "$s" = running ] && run=true
    printf '{"Id":"%s","Created":"2024-01-01T00:00:00.000000000Z","Image":"%s","Config":{"Labels":%s},"State":{"Status":"%s","Running":%s,"ExitCode":0,"OOMKilled":false,"StartedAt":"2024-01-01T00:00:01.000000000Z","FinishedAt":"0001-01-01T00:00:00Z"}}\n' \
        "$id" "$img_id" "$(json_labels < "$st/c/$id.labels")" "$s" "$run"
}
last() { for a in "$@"; do l="$a"; done; printf '%s' "$l"; }
case "$1" in
image)
    printf '{"Id":"%s","RootFS":{"Layers":["%s"]},"Config":{"Labels":{"candler.cc_version":"2.1.311"}}}\n' "$img_id" "$layer"
    ;;
build)
    echo "$img_id"
    ;;
network)
    case "$2" in
    create)
        name=$(last "$@")
        internal=false
        for a in "$@"; do [ "$a" = "--internal" ] && internal=true; done
        echo "$internal" > "$st/n/$name.internal"
        labels_of "$@" > "$st/n/$name.labels"
        echo "$name"
        ;;
    inspect)
        name=$(last "$@")
        if [ ! -f "$st/n/$name.internal" ]; then
            echo "Error: No such network: $name" >&2
            exit 1
        fi
        printf '{"Name":"%s","Internal":%s,"Labels":%s}\n' "$name" "$(cat "$st/n/$name.internal")" "$(json_labels < "$st/n/$name.labels")"
        ;;
    rm)
        name=$(last "$@")
        rm -f "$st/n/$name.internal" "$st/n/$name.labels"
        echo "$name"
        ;;
    connect) ;;
    esac
    ;;
create)
    new_container "$@"
    echo "$NEW"
    ;;
container)
    case "$2" in
    inspect)
        inspect_container "$(last "$@")"
        ;;
    ls)
        want=""
        for a in "$@"; do case "$a" in label=*) want="${a#label=}";; esac; done
        for f in "$st"/c/*.labels; do
            [ -f "$f" ] || continue
            id="${f##*/}"; id="${id%.labels}"
            [ -f "$st/c/$id.state" ] || continue
            if grep -qxF "$want" "$f"; then echo "$id"; fi
        done
        ;;
    stop)
        echo exited > "$st/c/$(last "$@").state"
        ;;
    rm)
        id=$(last "$@")
        rm -f "$st/c/$id.state" "$st/c/$id.labels"
        echo "$id"
        ;;
    esac
    ;;
start | restart | kill)
    echo running > "$st/c/$(last "$@").state"
    ;;
stop)
    echo exited > "$st/c/$(last "$@").state"
    ;;
rm)
    id=$(last "$@")
    rm -f "$st/c/$id.state" "$st/c/$id.labels"
    echo "$id"
    ;;
exec)
    case " $* " in
    *" -i "*) cat > /dev/null ;;
    esac
    ;;
logs) ;;
run)
    cf=$(cidfile_of "$@")
    if [ -n "$cf" ]; then
        new_container "$@"
        sleep 1
        echo exited > "$st/c/$NEW.state"
    elif case " $* " in *" --version "*) true;; *) false;; esac; then
        echo "2.1.311 (Claude Code)"
    else
        printf '{"schema":"tool-catalog-snapshot/1","captured_at":"2024-01-01T00:00:00Z","provider":"candler-image@1","servers":[]}'
    fi
    ;;
*)
    echo "UNHANDLED: $*" >> "$here/docker.log"
    echo "Error: unhandled" >&2
    exit 1
    ;;
esac
"#;

/// The one secret the experiment declares; its value is plain text of the characters a token has.
pub const SECRET_NAME: &str = "FIXTURE_TOKEN";

/// The directories of one trial.
pub struct World {
    /// The directory that holds all the others.
    pub root: PathBuf,
    /// A working directory unrelated to everything else in the world.
    pub work: PathBuf,
    /// The state home; the store is below it.
    pub state: PathBuf,
    /// The directory of the fake client, first on the path.
    pub bin: PathBuf,
    /// The path of the experiment file.
    pub experiment: PathBuf,
    /// The repository that holds the treatment's skills.
    pub skills: PathBuf,
}

/// Runs `git` in `dir`, panicking when it fails.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.org")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.org")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A fresh world: the fake client, a skills repository with one tagged commit, a one-cell
/// experiment, and a working directory that is none of them.
pub fn world() -> World {
    let root = std::env::temp_dir().join(format!(
        "candler-guard-run-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let bin = root.join("bin");
    let work = root.join("work");
    let state = root.join("state");
    let skills = root.join("skills");
    for dir in [&bin, &work, &state, &skills, &root.join("home")] {
        fs::create_dir_all(dir).unwrap();
    }
    let fake = bin.join("docker");
    fs::write(&fake, FAKE).unwrap();
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(bin.join("docker.log"), "").unwrap();
    fs::write(bin.join("handed.log"), "").unwrap();

    fs::create_dir_all(skills.join("demo")).unwrap();
    fs::write(skills.join("demo").join("SKILL.md"), "# demo\n").unwrap();
    git(&skills, &["init", "-q"]);
    git(&skills, &["add", "."]);
    git(&skills, &["commit", "-q", "-m", "skills"]);
    git(&skills, &["tag", "v1"]);

    let lock = work.join("env.lock");
    fs::write(&lock, "# no requirements\n").unwrap();
    let experiment = root.join("experiment.toml");
    let text = format!(
        r#"schema = "experiment/1"
replicates = 1
secrets = ["{SECRET_NAME}"]

[fixed]
"model.endpoint" = "http://127.0.0.1:9"
"prompt.template" = "say hello"

[[factors]]
path = "model.id"
levels = ["fixture-model"]

[budget.per_run]
wall_s = 30

[harness]
name = "claude-code"

[treatment]
name = "demo"
tag = "v1"
source = "{}"
"#,
        skills.display(),
    );
    fs::write(&experiment, text).unwrap();
    World {
        root,
        work,
        state,
        bin,
        experiment,
        skills,
    }
}

impl World {
    /// The store the run uses: below the state home, as no project configuration names one.
    pub fn store(&self) -> PathBuf {
        self.state.join("candler").join("store")
    }

    /// What the fake client was asked, one call per line.
    pub fn calls(&self) -> String {
        fs::read_to_string(self.bin.join("docker.log")).unwrap()
    }

    /// The ids of the containers the fake client created, one per line.
    pub fn handed_out(&self) -> Vec<String> {
        fs::read_to_string(self.bin.join("handed.log"))
            .unwrap()
            .lines()
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Runs `candler run` on the experiment from the unrelated working directory, with stdin
    /// closed, and returns once the launcher has exited.
    pub fn launch(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_candler"))
            .arg("run")
            .arg(&self.experiment)
            .arg("--set")
            .arg(format!(
                "request.treatment_source={}",
                self.skills.display()
            ))
            .arg("--set")
            .arg("isolation.build.lock=env.lock")
            .arg("--set")
            .arg(format!("model.token_secret={SECRET_NAME}"))
            .current_dir(&self.work)
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", self.bin.display()))
            .env("HOME", self.root.join("home"))
            .env("XDG_STATE_HOME", &self.state)
            .env(SECRET_NAME, "fixture-token-0123456789")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// Removes the world's directory.
    pub fn remove(&self) {
        drop(fs::remove_dir_all(&self.root));
    }
}

/// Polls `done` every 50 ms until it holds or `limit` passes; returns whether it held.
pub fn wait_for(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + limit;
    loop {
        if done() {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Every process on the machine with its environment, as `ps eww` prints it: `pid command env…`.
pub fn ps_with_environment() -> String {
    let out = Command::new("ps")
        .args(["eww", "-ax", "-o", "pid=,command="])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The run keys the fake client saw in `candler.run=` labels: the markers of the agent runs.
pub fn run_keys(calls: &str) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    for word in calls.split_whitespace() {
        if let Some(key) = word.strip_prefix("candler.run=")
            && !keys.iter().any(|known| known == key)
        {
            keys.push(key.to_owned());
        }
    }
    keys
}
