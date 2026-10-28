#!/usr/bin/env python3
"""Select golden rows from the legacy grading corpus by shape class.

Reads `campaign/20260806-cont-train/` of the tag `candler-m2a-golden` in the
artifact repository through `git show` and `git cat-file --batch`; the working
tree is never touched. `expectations.jsonl` holds one row per (run,
expectation). A row's shape class is the tuple

    (status, value kind, weight class, cause kind)

    status        pass, fail, blocked, not_measured or not_applicable
    value kind    none, int, float or text
    weight class  default: score/max_score is the status default
                  (pass 1/1, fail 0/1, blocked 0/1, not_measured 0/0)
                  0/0: a zero weight on a status whose default is not 0/0
                  other: any other score/max_score
    cause kind    none: a pass or fail row
                  artifact: a blocked row with a `blocked_by` file
                  marker:<code>: a not_measured row whose evidence carries
                  `TIER_D_REPLAY: ... reason=<code>`
                  zero_mappings: a not_measured row whose evidence says the
                  mapping declares zero 'mappings' entries
                  unrecorded: the grading entry gives no readable cause

Each row's grading entry is read from the run's `eval-N-grading.json` at
`<cell>/eval-1-tier-a-domain-structure/<config>/<run>/<eval>`. AppleDouble
`._*` resource forks are never addressed.

Outputs, next to this script (or in --out):
    golden_rows.jsonl     the first row of each class in corpus order, verbatim
    golden_grading.jsonl  that row's entry from its grading file, keyed by cell,
                          run, eval and id
    golden_floats.json    every distinct float `value` in the corpus, sorted

Standard output lists each class with its row count.

Usage: python3 -I select_golden.py [--repo PATH] [--out DIR]
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_REPO = "/Users/hacker/Documents/src/External/PCfVW/agenticai4hpc26-artifact"
TAG = "candler-m2a-golden"
CAMPAIGN = "campaign/20260806-cont-train"
EVAL_DIR = "eval-1-tier-a-domain-structure"

DEFAULT_WEIGHT = {
    "pass": (1, 1),
    "fail": (0, 1),
    "blocked": (0, 1),
    "not_measured": (0, 0),
    "not_applicable": (0, 0),
}
MARKER = re.compile(r"TIER_D_REPLAY: status=not_measured reason=([a-z][a-z0-9_]*)")
ZERO_MAPPINGS = "declares zero 'mappings' entries"


def git_show(repo, path):
    return subprocess.run(
        ["git", "-C", repo, "show", f"{TAG}:{path}"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout


def read_blobs(repo, paths):
    """Return {path: bytes or None} for each path, through one cat-file process."""
    result = {}
    proc = subprocess.Popen(
        ["git", "-C", repo, "cat-file", "--batch"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
    )
    for path in paths:
        proc.stdin.write(f"{TAG}:{path}\n".encode())
        proc.stdin.flush()
        header = proc.stdout.readline().split()
        if len(header) == 3 and header[1] == b"blob":
            size = int(header[2])
            result[path] = proc.stdout.read(size)
            proc.stdout.read(1)
        else:
            result[path] = None
    proc.stdin.close()
    proc.wait()
    return result


def grading_path(row):
    return "/".join([CAMPAIGN, row["cell"], EVAL_DIR, row["config"], row["run"], row["eval"]])


def parse_grading(blob):
    if blob is None:
        return {}
    try:
        document = json.loads(blob)
    except ValueError:
        return {}
    entries = document.get("expectations") if isinstance(document, dict) else None
    if not isinstance(entries, list):
        return {}
    return {e["id"]: e for e in entries if isinstance(e, dict) and "id" in e}


def value_kind(value):
    if value is None:
        return "none"
    if isinstance(value, bool):
        return "other"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str):
        return "text"
    return "other"


def weight_class(row):
    weight = (row["score"], row["max_score"])
    if weight == DEFAULT_WEIGHT[row["status"]]:
        return "default"
    if weight == (0, 0):
        return "0/0"
    return "other"


def cause_kind(row, entry):
    status = row["status"]
    if status == "blocked":
        blocked_by = (entry or {}).get("blocked_by")
        return "artifact" if isinstance(blocked_by, str) and blocked_by else "unrecorded"
    if status == "not_measured":
        evidence = (entry or {}).get("evidence")
        if not isinstance(evidence, str):
            return "unrecorded"
        marker = MARKER.search(evidence)
        if marker:
            return "marker:" + marker.group(1)
        if ZERO_MAPPINGS in evidence:
            return "zero_mappings"
        return "unrecorded"
    return "none"


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--repo", default=DEFAULT_REPO, help="artifact repository path")
    parser.add_argument("--out", type=Path, default=Path(__file__).resolve().parent)
    args = parser.parse_args(argv)

    lines = git_show(args.repo, f"{CAMPAIGN}/expectations.jsonl").decode("utf-8").split("\n")
    lines = [line for line in lines if line]
    rows = [json.loads(line) for line in lines]

    blobs = read_blobs(args.repo, sorted({grading_path(r) for r in rows}))
    gradings = {path: parse_grading(blob) for path, blob in blobs.items()}

    counts = {}
    first = {}
    for index, row in enumerate(rows):
        entry = gradings[grading_path(row)].get(row["id"])
        shape = (row["status"], value_kind(row["value"]), weight_class(row), cause_kind(row, entry))
        counts[shape] = counts.get(shape, 0) + 1
        first.setdefault(shape, index)

    selected = sorted(first.values())
    floats = sorted({r["value"] for r in rows if isinstance(r["value"], float)})

    args.out.mkdir(parents=True, exist_ok=True)
    with open(args.out / "golden_rows.jsonl", "w", encoding="utf-8", newline="\n") as handle:
        for index in selected:
            handle.write(lines[index] + "\n")
    with open(args.out / "golden_grading.jsonl", "w", encoding="utf-8", newline="\n") as handle:
        for index in selected:
            row = rows[index]
            entry = gradings[grading_path(row)].get(row["id"])
            if entry is None:
                sys.exit(f"no grading entry for {row['cell']} {row['run']} {row['id']}")
            keyed = {
                "cell": row["cell"],
                "run": row["run"],
                "eval": row["eval"],
                "id": row["id"],
                "entry": entry,
            }
            handle.write(json.dumps(keyed, separators=(",", ":")) + "\n")
    with open(args.out / "golden_floats.json", "w", encoding="utf-8", newline="\n") as handle:
        json.dump(floats, handle, indent=2)
        handle.write("\n")

    print(f"{len(rows)} rows, {len(counts)} shape classes, {len(floats)} distinct floats")
    for shape in sorted(counts):
        print(f"{counts[shape]:6d}  " + " / ".join(shape))
    return 0


if __name__ == "__main__":
    sys.exit(main())
