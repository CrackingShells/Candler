# /// script
# requires-python = ">=3.11"
# ///
"""Pick the adapter's three real streams from the extracted golden corpus.

Usage: uv run select_streams.py GOLDEN_DIR [--write]

GOLDEN_DIR is `campaign/20260806-cont-train` of the golden tag, extracted. For each class below
the script picks, among the streams of that class, the smallest one that is at most 150 KB
gzipped and is not one of the three streams the mini-corpus of `candler-core` already holds.
Without `--write` it prints the choice; with it, it copies the chosen file byte for byte to
`streams/<name>/agent_stream.jsonl.gz` and prints the source path and sha256.

Classes:
  multi_init         several `system`/`init` lines and several `result` lines sharing one
                     session id (a turn started by `origin: task-notification`)
  subagent_denials   a line with a `parent_tool_use_id` and a non-empty `permission_denials`
  api_error          a `result` of subtype `success` with `is_error: true`
"""

import gzip
import hashlib
import json
import shutil
import sys
from pathlib import Path

LIMIT = 150 * 1024
HERE = Path(__file__).resolve().parent
MINI = HERE.parents[2] / "candler-core" / "tests" / "fixtures" / "mini-corpus"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def facts(path: Path) -> dict:
    init = 0
    subagent = False
    denials = 0
    errored = False
    for raw in gzip.open(path).read().split(b"\n"):
        if not raw.strip():
            continue
        line = json.loads(raw)
        if line.get("type") == "system" and line.get("subtype") == "init":
            init += 1
        if line.get("parent_tool_use_id"):
            subagent = True
        if line.get("type") == "result":
            denials += len(line.get("permission_denials") or [])
            if line.get("subtype") == "success" and line.get("is_error"):
                errored = True
    return {
        "multi_init": init > 1,
        "subagent_denials": subagent and denials > 0,
        "api_error": errored,
    }


def main() -> None:
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    golden = Path(sys.argv[1])
    write = "--write" in sys.argv[2:]
    held = {
        sha256(p) for p in MINI.glob("*/_run_cache/with_skill/run-*/agent_stream.jsonl.gz")
    }
    streams = sorted(golden.glob("*/_run_cache/*/run-*/agent_stream.jsonl.gz"))
    candidates = [
        p for p in streams if p.stat().st_size <= LIMIT and sha256(p) not in held
    ]
    for name in ("multi_init", "subagent_denials", "api_error"):
        chosen = min(
            (p for p in candidates if facts(p)[name]),
            key=lambda p: (p.stat().st_size, str(p)),
        )
        digest = sha256(chosen)
        print(f"{name}\t{chosen.relative_to(golden.parent)}\t{chosen.stat().st_size}\t{digest}")
        if write:
            target = HERE / "streams" / name / "agent_stream.jsonl.gz"
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(chosen, target)


if __name__ == "__main__":
    main()
