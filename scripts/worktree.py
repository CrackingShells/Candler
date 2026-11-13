#!/usr/bin/env python3
"""Keep every checkout of this repository set up the same way: the main one and each worktree.

    python3 scripts/worktree.py sync [--quiet]            hook and build settings of this checkout
    python3 scripts/worktree.py add <path> <branch> [--base <ref>]
    python3 scripts/worktree.py remove <path>

sync rewrites the shared commit-msg hook from main's cog.toml when it differs. Every worktree runs
the hooks of the common git directory, so one hook serves them all, and it follows main rather than
the branch at hand. sync then checks the settings a build relies on:
  - no core.hooksPath, which would bypass that hook;
  - no shared target directory (CARGO_TARGET_DIR, or build.target-dir in ~/.cargo/config.toml),
    because workspace members of different worktrees write the same artifact names into it, and a
    build there links a sibling worktree's code;
  - sccache as the rustc wrapper when it is installed: a warning only, since builds work without it.
Incremental compilation is off for every checkout through the tracked .cargo/config.toml.

add creates the worktree on a new branch from <ref> (origin/main by default, fetched first) and
syncs it. remove deletes a worktree whose branch main already contains, then the branch; it never
forces either.

Exit status: 0 set up (warnings allowed); 1 a setting needs a fix by hand, or remove refused;
2 a git command failed. Python 3.11 or later, standard library only.
"""

import argparse
import os
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

MAIN = "origin/main"


def git(*args: str, cwd: Path | None = None) -> str:
    done = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if done.returncode != 0:
        print(f"worktree: git {' '.join(args)}: {done.stderr.strip()}", file=sys.stderr)
        sys.exit(2)
    return done.stdout.strip()


def hook_script(cwd: Path) -> str:
    """The commit-msg script of main's cog.toml, or of this checkout's when main is not fetched."""
    shown = subprocess.run(["git", "show", f"{MAIN}:cog.toml"], cwd=cwd, capture_output=True, text=True)
    text = shown.stdout if shown.returncode == 0 else (cwd / "cog.toml").read_text()
    return tomllib.loads(text)["git_hooks"]["commit-msg"]["script"]


def sync(cwd: Path, quiet: bool) -> int:
    notes: list[str] = []
    problems: list[str] = []
    hook = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir", cwd=cwd)) / "hooks" / "commit-msg"
    script = hook_script(cwd)
    if not hook.exists() or hook.read_text() != script:
        hook.parent.mkdir(exist_ok=True)
        staged = hook.with_suffix(".new")
        staged.write_text(script)
        staged.chmod(0o755)
        staged.replace(hook)
        notes.append(f"commit-msg hook rewritten from {MAIN}'s cog.toml")
    if subprocess.run(["git", "config", "core.hooksPath"], cwd=cwd, capture_output=True).returncode == 0:
        problems.append("core.hooksPath is set, so the shared hooks never run: git config --unset core.hooksPath")
    user = Path.home() / ".cargo" / "config.toml"
    build = tomllib.loads(user.read_text()).get("build", {}) if user.exists() else {}
    if os.environ.get("CARGO_TARGET_DIR") or "target-dir" in build:
        problems.append("a shared target directory is set (CARGO_TARGET_DIR or build.target-dir): unset it")
    if shutil.which("sccache") and "sccache" not in (os.environ.get("RUSTC_WRAPPER", "") + str(build.get("rustc-wrapper", ""))):
        notes.append(f'sccache is installed but not the rustc wrapper: add `[build] rustc-wrapper = "sccache"` to {user}')
    for line in problems if quiet else problems + notes:
        print(f"worktree: {line}")
    if not quiet and not problems and not notes:
        print("worktree: set up")
    return 1 if problems else 0


def add(path: Path, branch: str, base: str) -> int:
    if base.startswith("origin/"):
        git("fetch", "-q", "origin")
    git("worktree", "add", "-b", branch, str(path), base)
    return sync(path, quiet=False)


def remove(path: Path) -> int:
    path = path.resolve()
    git("fetch", "-q", "origin")
    branch = git("-C", str(path), "symbolic-ref", "--short", "-q", "HEAD")
    merged = subprocess.run(["git", "merge-base", "--is-ancestor", branch, MAIN], cwd=path).returncode == 0
    if not merged:
        print(f"worktree: {branch} is not in {MAIN} yet; nothing removed", file=sys.stderr)
        return 1
    main_checkout = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir", cwd=path)).parent
    git("worktree", "remove", str(path), cwd=main_checkout)
    git("branch", "-d", branch, cwd=main_checkout)
    print(f"worktree: removed {path} and {branch}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = parser.add_subparsers(dest="command", required=True)
    s = sub.add_parser("sync")
    s.add_argument("--quiet", action="store_true", help="print only what needs attention")
    a = sub.add_parser("add")
    a.add_argument("path", type=Path)
    a.add_argument("branch")
    a.add_argument("--base", default=MAIN)
    r = sub.add_parser("remove")
    r.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.command == "sync":
        return sync(Path.cwd(), args.quiet)
    if args.command == "add":
        return add(args.path, args.branch, args.base)
    return remove(args.path)


if __name__ == "__main__":
    sys.exit(main())
