"""Select and vendor the real runs of the mini-corpus, and check the vendored files.

Build (reads an extracted golden corpus, writes the real part of the mini-corpus):

    uv run python select_mini.py --corpus <extracted>/campaign/20260806-cont-train --out <mini-corpus>

Check (verifies every vendored file against PROVENANCE.toml; with --corpus, also that each real
file is byte-identical to its source):

    uv run python select_mini.py --check <mini-corpus> [--corpus <extracted>/campaign/20260806-cont-train]

Three real runs are chosen by predicate, each the smallest (stream plus outputs) that satisfies it:

* `clean`: verdict clean, a `TIER_D_REPLAY` not_measured row, a pass weighing 0/0, a float
  value and a failing row with a fractional weight; first cell.
* `incomplete`: verdict incomplete, a blocked row with `blocked_by`, a non-empty `outputs/`;
  same cell as the clean run.
* `zero_mappings`: verdict clean, a not_measured row whose evidence says the mapping declares
  no entries; a different cell, so `factors` is non-empty.

Each run keeps `eval_metadata.json` in every eval directory and, in the first eval directory
(which also holds the `eval-K-grading.json` files), every file: `grading_summary.json`,
`timing.json` and `metrics.json` are identical in all thirteen eval directories of a run.
"""

import argparse
import collections
import gzip
import hashlib
import json
import os
import shutil
import sys
import tomllib

CORPUS_SUBDIR = "campaign/20260806-cont-train"
FIRST_EVAL = "eval-1-tier-a-domain-structure"
MARKER = "TIER_D_REPLAY"
ZERO_MAPPINGS = "declares zero 'mappings' entries"
KEPT_PROVENANCE_KEYS = (
    "purpose",
    "reminder",
    "regraded_by",
    "regraded_at",
    "original_grading",
    "pinned",
    "eval_infra",
    "catalog",
)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def load_rows(corpus):
    with open(os.path.join(corpus, "expectations.jsonl"), encoding="utf-8") as f:
        return [(line, json.loads(line)) for line in f.read().splitlines() if line]


def grading_evidence(corpus, cell, run, eval_file):
    path = os.path.join(corpus, cell, FIRST_EVAL, "with_skill", run, eval_file)
    with open(path, encoding="utf-8") as f:
        return json.load(f)["expectations"]


def run_facts(corpus, cell, run, rows, verdict):
    stream = os.path.join(corpus, cell, "_run_cache/with_skill", run, "agent_stream.jsonl.gz")
    outputs = os.path.join(corpus, cell, "_run_cache/with_skill", run, "outputs")
    out_files = sorted(os.listdir(outputs)) if os.path.isdir(outputs) else []
    size = os.path.getsize(stream) + sum(os.path.getsize(os.path.join(outputs, n)) for n in out_files)
    replay = zero = blocked_by = False
    for _, r in rows:
        if r["status"] == "not_measured":
            for e in grading_evidence(corpus, cell, run, r["eval"]):
                if e["id"] == r["id"]:
                    replay = replay or MARKER in e["evidence"]
                    zero = zero or ZERO_MAPPINGS in e["evidence"]
        if r["status"] == "blocked":
            for e in grading_evidence(corpus, cell, run, r["eval"]):
                if e["id"] == r["id"] and "blocked_by" in e:
                    blocked_by = True
    return {
        "verdict": verdict,
        "size": size,
        "outputs": out_files,
        "replay": replay,
        "zero_mappings": zero,
        "blocked_by": blocked_by,
        "zero_weight_pass": any(r["status"] == "pass" and r["max_score"] == 0 for _, r in rows),
        "float": any(isinstance(r["value"], float) for _, r in rows),
        "fraction_fail": any(
            r["status"] == "fail" and r["max_score"] > 1 and r["score"] > 0 for _, r in rows
        ),
    }


def select(corpus):
    with open(os.path.join(corpus, "PROVENANCE.json"), encoding="utf-8") as f:
        provenance = json.load(f)
    verdicts = {
        (r["cell"], r["run"]): r["original_integrity"]["verdict"] for r in provenance["runs"]
    }
    by_run = collections.defaultdict(list)
    for line, row in load_rows(corpus):
        by_run[(row["cell"], row["run"])].append((line, row))
    facts = {
        key: run_facts(corpus, key[0], key[1], rows, verdicts[key])
        for key, rows in sorted(by_run.items())
    }

    def smallest(predicate, cell_filter):
        cands = [
            (f["size"], key)
            for key, f in facts.items()
            if predicate(f) and cell_filter(key[0])
        ]
        return min(cands)[1]

    clean = smallest(
        lambda f: f["verdict"] == "clean"
        and f["replay"]
        and f["zero_weight_pass"]
        and f["float"]
        and f["fraction_fail"],
        lambda c: True,
    )
    incomplete = smallest(
        lambda f: f["verdict"] == "incomplete" and f["blocked_by"] and f["outputs"],
        lambda c: c == clean[0],
    )
    zero = smallest(
        lambda f: f["verdict"] == "clean" and f["zero_mappings"],
        lambda c: c != clean[0],
    )
    chosen = [("clean", clean), ("incomplete", incomplete), ("zero_mappings", zero)]
    return provenance, by_run, chosen


def files_of_run(corpus, cell, run):
    """Corpus-relative paths of the files a real run vendors, in a stable order."""
    paths = []
    cache = os.path.join(corpus, cell, "_run_cache/with_skill", run)
    for dirpath, _, names in sorted(os.walk(cache)):
        for name in sorted(names):
            paths.append(os.path.relpath(os.path.join(dirpath, name), corpus))
    evals = sorted(
        d for d in os.listdir(os.path.join(corpus, cell)) if d.startswith("eval-") and
        os.path.isdir(os.path.join(corpus, cell, d))
    )
    for ev in evals:
        run_dir = os.path.join(corpus, cell, ev, "with_skill", run)
        for name in sorted(os.listdir(run_dir)):
            if ev == FIRST_EVAL or name == "eval_metadata.json":
                paths.append(os.path.relpath(os.path.join(run_dir, name), corpus))
    apple = os.path.join(
        corpus, "__MACOSX", cell, FIRST_EVAL, "with_skill", run, "._eval-10-grading.json"
    )
    if os.path.isfile(apple):
        paths.append(os.path.relpath(apple, corpus))
    return paths


def files_of_cell(corpus, cell, with_ds_store):
    paths = ["experiment.json", "progress.jsonl", "contamination_catalog.jsonl"]
    shared = os.path.join(corpus, cell, "shared_with_skill")
    for name in sorted(os.listdir(shared)):
        paths.append(os.path.join("shared_with_skill", name))
    if with_ds_store:
        paths.append(".DS_Store")
    return [os.path.join(cell, p) for p in paths]


def copy_file(corpus, out, rel):
    dst = os.path.join(out, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(os.path.join(corpus, rel), dst)


def write_derived(out, rel, data):
    dst = os.path.join(out, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    with open(dst, "wb") as f:
        f.write(data)


def toml_str(s):
    return json.dumps(s, ensure_ascii=False)


def build(corpus, out):
    provenance, by_run, chosen = select(corpus)
    if os.path.exists(out) and os.listdir(out):
        sys.exit(f"refusing: {out} is not empty")
    os.makedirs(out, exist_ok=True)
    cells = []
    for _, (cell, _) in chosen:
        if cell not in cells:
            cells.append(cell)

    entries = []  # (rel, source, run_name)
    for name, (cell, run) in chosen:
        for rel in files_of_run(corpus, cell, run):
            copy_file(corpus, out, rel)
            entries.append((rel, f"{CORPUS_SUBDIR}/{rel}", f"{name}"))
    for i, cell in enumerate(cells):
        for rel in files_of_cell(corpus, cell, with_ds_store=(i == 0)):
            copy_file(corpus, out, rel)
            entries.append((rel, f"{CORPUS_SUBDIR}/{rel}", "cell"))

    keys = {key for _, key in chosen}
    expectations = b"".join(
        (line + "\n").encode("utf-8")
        for key in sorted(by_run)
        if key in keys
        for line, _ in by_run[key]
    )
    write_derived(out, "expectations.jsonl", expectations)
    entries.append(
        ("expectations.jsonl", f"derived: {CORPUS_SUBDIR}/expectations.jsonl rows of the vendored runs", "corpus")
    )

    trimmed = {k: provenance[k] for k in KEPT_PROVENANCE_KEYS if k in provenance}
    corpus_block = dict(provenance["corpus"])
    corpus_block["cells"] = cells
    trimmed["corpus"] = corpus_block
    trimmed["runs"] = [r for r in provenance["runs"] if (r["cell"], r["run"]) in keys]
    write_derived(
        out,
        "PROVENANCE.json",
        (json.dumps(trimmed, indent=2, ensure_ascii=False) + "\n").encode("utf-8"),
    )
    entries.append(
        ("PROVENANCE.json", f"derived: {CORPUS_SUBDIR}/PROVENANCE.json without status_counts_by_expectation and caveats, runs[] and corpus.cells trimmed to the vendored runs", "corpus")
    )

    runs = [
        {"cell": cell, "run": run, "name": f"real_{name}", "synthetic": False}
        for name, (cell, run) in chosen
    ]
    write_provenance_toml(out, runs, entries)
    for name, (cell, run) in chosen:
        print(f"{name}: {cell} {run}")


def write_provenance_toml(out, runs, entries):
    lines = [
        "# Source path and sha256 of every vendored file.",
        "# Real files are copied byte for byte from the golden corpus by select_mini.py;",
        "# synthetic files are written by make_synthetic.py.",
        'source_tag = "candler-m2a-golden"',
        f"source_root = {toml_str(CORPUS_SUBDIR)}",
        "",
    ]
    for r in runs:
        lines += [
            "[[runs]]",
            f"name = {toml_str(r['name'])}",
            f"cell = {toml_str(r['cell'])}",
            f"run = {toml_str(r['run'])}",
            f"synthetic = {'true' if r['synthetic'] else 'false'}",
            "",
        ]
    for rel, source, owner in sorted(entries):
        lines += [
            "[[files]]",
            f"path = {toml_str(rel)}",
            f"source = {toml_str(source)}",
            f"sha256 = {toml_str(sha256_file(os.path.join(out, rel)))}",
            f"owner = {toml_str(owner)}",
            "",
        ]
    with open(os.path.join(out, "PROVENANCE.toml"), "w", encoding="utf-8") as f:
        f.write("\n".join(lines))


def check(mini, corpus):
    with open(os.path.join(mini, "PROVENANCE.toml"), "rb") as f:
        doc = tomllib.load(f)
    failures = []
    listed = set()
    for entry in doc["files"]:
        rel = entry["path"]
        listed.add(rel)
        path = os.path.join(mini, rel)
        if not os.path.isfile(path):
            failures.append(f"missing: {rel}")
            continue
        if sha256_file(path) != entry["sha256"]:
            failures.append(f"sha256 differs: {rel}")
            continue
        if corpus and entry["source"].startswith(CORPUS_SUBDIR + "/"):
            src = os.path.join(corpus, entry["source"][len(CORPUS_SUBDIR) + 1 :])
            if not os.path.isfile(src) or sha256_file(src) != entry["sha256"]:
                failures.append(f"differs from the corpus: {rel}")
    present = set()
    for dirpath, _, names in os.walk(mini):
        for name in names:
            present.add(os.path.relpath(os.path.join(dirpath, name), mini))
    for rel in sorted(present - listed - {"PROVENANCE.toml", "README.md"}):
        failures.append(f"unlisted: {rel}")
    synthetic = [r["name"] for r in doc["runs"] if r["synthetic"]]
    real = [r["name"] for r in doc["runs"] if not r["synthetic"]]
    if len(real) != 3:
        failures.append(f"expected 3 real runs, found {len(real)}")
    for name in ("synthetic_contaminated", "synthetic_text_value", "synthetic_not_applicable"):
        if name not in synthetic:
            failures.append(f"missing synthetic run {name}")
    if failures:
        print("\n".join(failures), file=sys.stderr)
        sys.exit(1)
    print(f"ok: {len(listed)} files, real runs {real}, synthetic runs {synthetic}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus")
    ap.add_argument("--out")
    ap.add_argument("--check")
    args = ap.parse_args()
    if args.check:
        check(args.check, args.corpus)
    elif args.corpus and args.out:
        build(args.corpus, args.out)
    else:
        ap.error("give --corpus and --out, or --check")


if __name__ == "__main__":
    main()
