//! Guards for the one choke point of destruction (I22): the docker verbs that remove a container,
//! a network or an image appear only inside the destroy half of `isolation/docker.rs`, and only
//! `isolation/container/guard.rs` calls them. The scan reads the source of the core as text; a
//! tree with a planted violation in a temporary directory proves that it can fail.
#![cfg(test)]

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use regex::Regex;

/// The one file that may spell a destroy verb, and only inside its destroy impl.
const DOCKER: &str = "isolation/docker.rs";

/// The one file that may call a destroy driver.
const GUARD: &str = "isolation/container/guard.rs";

/// The files that may name the destroy trait: its definition, its implementations (the real one
/// and the test-support fakes) and its one holder.
const MAY_NAME_DESTROY_DRIVER: [&str; 4] =
    [DOCKER, GUARD, "isolation/driver.rs", "isolation/testing.rs"];

/// The test-support fakes delegate to the driver they wrap and name the verb they check. They
/// are compiled only with the `test-support` feature, which no production edge enables.
const FAKES: &str = "isolation/testing.rs";

/// What a scan read and found.
#[derive(Debug, Default)]
struct Scan {
    /// The `.rs` files read.
    files: usize,
    /// The destroy verbs found where they are allowed: inside the destroy impl of `DOCKER`.
    allowed_verbs: usize,
    /// What a rule refused, as `path:line: text`.
    violations: Vec<String>,
}

/// The `.rs` files under `dir`, as paths relative to `root`, sorted.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            sources(&path, found)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
    found.sort();
    Ok(())
}

/// The line without its `//` comment. A `//` inside a string literal does not start one.
fn code_of(line: &str) -> &str {
    let bytes = line.as_bytes();
    let (mut quoted, mut index) = (false, 0);
    while let Some(byte) = bytes.get(index) {
        match byte {
            b'\\' if quoted => index += 1,
            b'"' => quoted = !quoted,
            b'/' if !quoted && bytes.get(index + 1) == Some(&b'/') => {
                return line.get(..index).unwrap_or(line);
            }
            _ => {}
        }
        index += 1;
    }
    line
}

/// Reads every `.rs` file under `root` and applies the three rules of the choke point.
fn scan(root: &Path) -> io::Result<Scan> {
    let verb = Regex::new(
        r#""(?:(?:container|network|image)\s+)?(?:stop|rm|kill|rmi|prune|remove)(?:\s[^\s"{}]*)?"|"[^"]*\bdocker\s+(?:(?:container|network)\s+)?(?:rm|stop|kill|remove)\b"#,
    )
    .expect("a pattern");
    let call = Regex::new(
        r"\.(rm|network_rm)\(|\b(destroy|destroyer|driver|inner|kill)\s*\.\s*(stop|rm|network_rm)\(",
    )
    .expect("a pattern");
    let named =
        Regex::new(r"\b(DestroyDriver|DockerDestroyer)\b|\.destroyer\(").expect("a pattern");
    let reap = Regex::new(r"waitpid\(\s*(-1|None)").expect("a pattern");
    let mut result = Scan::default();
    let mut paths = Vec::new();
    sources(root, &mut paths)?;
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .map_err(io::Error::other)?
            .to_string_lossy()
            .replace('\\', "/");
        result.files += 1;
        let text = fs::read_to_string(&path)?;
        let (mut inside, mut depth) = (false, 0_i32);
        for (index, line) in text.lines().enumerate() {
            let code = code_of(line);
            if relative == DOCKER && code.contains("impl DestroyDriver for") {
                inside = true;
                depth = 0;
            }
            let mut flag = |rule: &str| {
                result.violations.push(format!(
                    "{relative}:{}: {rule}: {}",
                    index + 1,
                    line.trim()
                ));
            };
            if verb.is_match(code) {
                if relative == DOCKER && inside {
                    result.allowed_verbs += verb.find_iter(code).count();
                } else if !(relative == FAKES && code.contains("check_container(")) {
                    flag("a destroy verb outside the destroy impl of the docker module");
                }
            }
            if call.is_match(code)
                && relative != GUARD
                && !(relative == FAKES && code.contains("self.inner."))
            {
                flag("a call to a destroy driver outside the guard");
            }
            if named.is_match(code) && !MAY_NAME_DESTROY_DRIVER.contains(&relative.as_str()) {
                // BORROW: .as_str() compares the path with the list
                flag("a holder of the destroy driver that is not the guard");
            }
            if reap.is_match(code) {
                flag("a wait for any child");
            }
            if inside {
                depth += i32::try_from(code.matches('{').count()).unwrap_or(0)
                    - i32::try_from(code.matches('}').count()).unwrap_or(0);
                if depth <= 0 && code.contains('}') {
                    inside = false;
                }
            }
        }
    }
    Ok(result)
}

/// The file that alone may mint a lost proof, relative to the directory of the crates.
const MINTS_LOST_PROOF: &str = "candler-core/src/jobs/lost.rs";

/// Every call that mints a lost proof under the `src` of each crate below `crates`, as
/// `path:line` relative to `crates`, sorted.
fn lost_proof_calls(crates: &Path) -> io::Result<Vec<String>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(crates)? {
        let src = entry?.path().join("src");
        if !src.is_dir() {
            continue;
        }
        let mut paths = Vec::new();
        sources(&src, &mut paths)?;
        for path in paths {
            let relative = path
                .strip_prefix(crates)
                .map_err(io::Error::other)?
                .to_string_lossy()
                .replace('\\', "/");
            for (index, line) in fs::read_to_string(&path)?.lines().enumerate() {
                for _ in code_of(line).matches("LostProof::held(") {
                    found.push(format!("{relative}:{}", index + 1));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Writes `files` under a new temporary directory.
fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    for (name, text) in files {
        let path = dir.path().join(name);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    dir
}

/// The docker module and the guard of a tree that obeys the rules.
const CLEAN: [(&str, &str); 2] = [
    (
        DOCKER,
        "impl DestroyDriver for Destroyer {\n    fn rm(&self) { run([\"container\", \"rm\"]); }\n}\n",
    ),
    (
        GUARD,
        "fn go(d: &dyn DestroyDriver) { d.rm(id); self.destroy.rm(id); }\n",
    ),
];

/// Guards: B16 — one choke point: the destroy verbs are spelled only inside the destroy impl of
/// `isolation/docker.rs`, destroy drivers are called only in `isolation/container/guard.rs` and
/// named only by the files that define, implement or hold one, and nothing waits for any child.
/// A lost proof is minted in one place: `LostProof::held(` appears in the `src` of the crates
/// only in `jobs/lost.rs`, once.
/// Oracle: a scan of `crates/candler-core/src` for the verbs `stop`, `rm`, `kill`, `rmi` and
/// `prune` (so also `network rm` and `image rm`), for calls on a destroy driver and for
/// `waitpid(-1)`; a temporary tree with each violation planted is refused, and a clean tree
/// that obeys the rules is not; and a scan of every crate's `src` for `LostProof::held(`, where a
/// tree with the call in another file, or twice in `jobs/lost.rs`, is refused.
/// Bite: add the line `let _ = LostProof::held(&lock);` to any file under `crates/*/src` other
/// than `jobs/lost.rs`; or add the line `struct D; impl D { fn rm(&self) {} } let driver = D; driver.rm();` to the
/// start of `LabelSweeper::collect` in `isolation/container/sweeper.rs`; or the line
/// `let _ = "docker rm -f x";` there.
#[test]
fn b16_destroy_verbs_only_in_guard() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = scan(&source).expect("the source can be read");
    assert!(found.files > 20, "the scan read only {} files", found.files);
    // The positive control: the scan did see the verbs where they are allowed.
    assert!(found.allowed_verbs >= 3, "{found:?}");
    assert!(found.violations.is_empty(), "{:#?}", found.violations);

    let clean = tree(&CLEAN);
    let ok = scan(clean.path()).expect("the tree can be read");
    assert_eq!((ok.files, ok.allowed_verbs), (2, 1), "{ok:?}");
    assert!(ok.violations.is_empty(), "{:#?}", ok.violations);

    let planted = [
        (
            "a verb in another file",
            "fn f() { run([\"container\", \"rm\"]); }\n",
        ),
        ("a kill verb", "fn f() { run([\"kill\", id]); }\n"),
        (
            "the remove alias",
            "fn f() { run([\"container\", \"remove\", id]); }\n",
        ),
        ("a single-string rm", "fn f() { sh(\"docker rm x\"); }\n"),
        ("a flagged rm", "fn f() { sh(\"docker rm -f x\"); }\n"),
        (
            "a flagged container rm",
            "fn f() { sh(\"docker container rm -f x\"); }\n",
        ),
        ("a shell rm", "fn f() { sh(\"sh -c docker rm x\"); }\n"),
        (
            "a single-string container rm",
            "fn f() { sh(\"docker container rm x\"); }\n",
        ),
        (
            "a verb after a slash pair in a string",
            "fn f() { let u = \"http://x\"; run([\"rm\"]); }\n",
        ),
        (
            "the destroyer type",
            "use crate::isolation::docker::DockerDestroyer;\n",
        ),
        (
            "the destroyer accessor",
            "fn f() { let d = cli.destroyer(); }\n",
        ),
        ("a driver call", "fn f() { self.driver.rm(id); }\n"),
        ("a network call", "fn f() { d.network_rm(name); }\n"),
        (
            "a stop on the destroy driver",
            "fn f() { self.destroy.stop(id); }\n",
        ),
        ("another holder", "struct S { d: Arc<dyn DestroyDriver> }\n"),
        (
            "a wait for any child",
            "fn f() { let _ = waitpid(None, flags); }\n",
        ),
        (
            "a wait for any child by -1",
            "fn f() { let _ = waitpid(-1, flags); }\n",
        ),
    ];
    for (label, text) in planted {
        let mut files = CLEAN.to_vec();
        files.push(("isolation/container/sweeper.rs", text));
        let dir = tree(&files);
        let seen = scan(dir.path()).expect("the tree can be read");
        assert_eq!(seen.violations.len(), 1, "{label}: {:#?}", seen.violations);
    }
    // A lost proof is minted only by recovery, once.
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let minted = lost_proof_calls(&crates).expect("the crates can be read");
    assert_eq!(minted.len(), 1, "{minted:#?}");
    assert!(
        minted.iter().all(|site| site.starts_with(MINTS_LOST_PROOF)),
        "{minted:#?}"
    );
    let call = "fn f(lock: &JobLock) { let p = LostProof::held(lock); }\n";
    let once = tree(&[(MINTS_LOST_PROOF, call)]);
    let sites = lost_proof_calls(once.path()).expect("the tree can be read");
    assert_eq!(sites.len(), 1, "{sites:#?}");
    for (label, files) in [
        (
            "a call in another file",
            vec![
                (MINTS_LOST_PROOF, call),
                ("candler/src/commands/ps.rs", call),
            ],
        ),
        (
            "a second call in the recovery file",
            vec![(
                MINTS_LOST_PROOF,
                "fn f(l: &JobLock) { LostProof::held(l); LostProof::held(l); }\n",
            )],
        ),
        (
            "no call at all",
            vec![("candler-core/src/jobs/lost.rs", "fn f() {}\n")],
        ),
    ] {
        let dir = tree(&files);
        let sites = lost_proof_calls(dir.path()).expect("the tree can be read");
        let allowed = sites.len() == 1 && sites.iter().all(|s| s.starts_with(MINTS_LOST_PROOF));
        assert!(!allowed, "{label}: {sites:#?}");
    }
    // A verb in the docker module but outside its destroy impl is refused too.
    let mut files = CLEAN.to_vec();
    // INDEX: the clean tree has more than one file
    files[0] = (
        DOCKER,
        "impl DestroyDriver for Destroyer {\n    fn rm(&self) { run([\"rm\"]); }\n}\nfn other() { run([\"stop\"]); }\n",
    );
    let outside = tree(&files);
    let seen = scan(outside.path()).expect("the tree can be read");
    assert_eq!(seen.violations.len(), 1, "{:#?}", seen.violations);
}
