"""Derive the A-C reference (counts, zero-weight pairs, rates, equivalence) from the raw corpus.

    uv run --no-project python gen_oracle.py <corpus> <R>            # write
    uv run --no-project python gen_oracle.py --check <corpus> <R>    # compare

Inputs, and nothing else: the golden corpus's raw files (`eval-K-grading.json`, `experiment.json`,
`timing.json`, stream init lines, the root `PROVENANCE.json`) and three files of the HTN repo `<R>`
at commit 28be2c15 (`eval.json`, `_model.py`, `_outcome.py`), read with `git show` into a temp
dir. No Candler code, importer output, store or outcome file is read. `--check` also reads the
M1 vector file `cell_digest.toml`, only to cross-check the two cells it names.
"""

import argparse
import glob
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import types

HERE = os.path.dirname(os.path.abspath(__file__))
PIN = "28be2c15"
INFRA = ".claude/skills/htn-creator-workspace/infrastructure"
EVAL_JSON_SHA = "ca7f12afee55152ed0e543a1d9b457c29fc5c02fd49e1ec6a417a9dcc1c97f9e"
UNCOMPARED = ("D-E1", "D-E2", "D-E3", "D-E4", "D-E5", "D-E6")
STATUSES = ("pass", "fail", "blocked", "not_measured", "not_applicable")


def jtext(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), allow_nan=False)


def load_json(path):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


# ---------------------------------------------------------------- the pinned sources


def git_show(repo, path):
    """`git -C <repo> show <PIN>:<path>`: read only."""
    return subprocess.run(["git", "-C", repo, "show", f"{PIN}:{path}"], check=True, capture_output=True).stdout


def load_suite(repo, tmp):
    """eval.json (sha checked), and `_outcome.py` loaded under a bare package stub."""
    raw = git_show(repo, f"{INFRA}/evals/eval.json")
    got = hashlib.sha256(raw).hexdigest()
    if got != EVAL_JSON_SHA:
        sys.exit(f"eval.json at {PIN} has sha256 {got}, expected {EVAL_JSON_SHA}")
    pkg = os.path.join(tmp, "htn_creator_eval")
    os.makedirs(pkg)
    for name in ("_model.py", "_outcome.py"):
        with open(os.path.join(pkg, name), "wb") as f:
            f.write(git_show(repo, f"{INFRA}/htn_creator_eval/{name}"))
    stub = types.ModuleType("htn_creator_eval")
    stub.__path__ = [pkg]
    sys.modules["htn_creator_eval"] = stub
    mods = {}
    for name in ("_model", "_outcome"):
        spec = importlib.util.spec_from_file_location(f"htn_creator_eval.{name}", os.path.join(pkg, f"{name}.py"))
        mod = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = mod
        spec.loader.exec_module(mod)
        mods[name] = mod
    evals = json.loads(raw)["evals"]
    suite = {"eval_ids": {}, "role": {}, "ac": set(), "d": set()}
    for ev in evals:
        ids = [x["id"] for x in ev["expectations"]]
        suite["eval_ids"][ev["id"]] = set(ids)
        for x in ev["expectations"]:
            suite["role"][x["id"]] = x["role"]
            (suite["d"] if ev["tier"] == "executability" else suite["ac"]).add(x["id"])
    assert sorted(suite["d"]) == list(UNCOMPARED), sorted(suite["d"])
    return suite, mods["_model"], mods["_outcome"]


# ---------------------------------------------------------------- rows


def cells_of(corpus):
    names = sorted(n for n in os.listdir(corpus) if re.match(r"20\d{6}-", n) and os.path.isfile(os.path.join(corpus, n, "experiment.json")))
    assert names, corpus
    return names


def derive_rows(corpus, suite):
    """Every grading row of the corpus: dicts of cell, run (int), id, status, score, max_score."""
    rows = []
    for cell in cells_of(corpus):
        seen = {}
        pattern = os.path.join(corpus, cell, "eval-*", "with_skill", "run-*", "eval-*-grading.json")
        for path in sorted(glob.glob(pattern)):
            run = int(re.fullmatch(r"run-(\d+)", os.path.basename(os.path.dirname(path))).group(1))
            k = int(re.fullmatch(r"eval-(\d+)-grading\.json", os.path.basename(path)).group(1))
            assert (run, k) not in seen, path
            ids = []
            for e in load_json(path)["expectations"]:
                assert e["status"] in STATUSES, (path, e["status"])
                ids.append(e["id"])
                rows.append({"cell": cell, "run": run, "id": e["id"], "status": e["status"], "score": e["score"], "max_score": e["max_score"]})
            assert len(ids) == len(set(ids)) and set(ids) == suite["eval_ids"][k], path
            seen[(run, k)] = path
        runs = sorted({r for r, _ in seen})
        assert len(runs) == 20 and all((r, k) in seen for r in runs for k in suite["eval_ids"]), cell
    return rows


def ac_rows(rows, suite):
    return [r for r in rows if r["id"] in suite["ac"]]


def zero_weight(r):
    return r["status"] == "pass" and r["max_score"] == 0


def count(rows, status):
    return sum(1 for r in rows if r["status"] == status)


def write_counts(rows, suite):
    ac = ac_rows(rows, suite)
    zw = sum(1 for r in ac if zero_weight(r))
    assert all(r["score"] == 0 for r in ac if zero_weight(r))
    n = {s: count(ac, s) for s in STATUSES}
    assert n["not_applicable"] == 0
    out = [f"total = {len(rows)}", "", "[imported]", f"pass = {n['pass']}", f"zero_weight = {zw}"]
    out += [f"fail = {n['fail']}", f"blocked = {n['blocked']}", f"not_measured = {n['not_measured']}", ""]
    out += ["[live_expected]", f"pass = {n['pass'] - zw}", f"not_applicable = {zw}", f"fail = {n['fail']}"]
    out += [f"blocked = {n['blocked']}", f"not_measured = {n['not_measured']}", "", "[uncompared]"]
    out += [f"{i} = {sum(1 for r in rows if r['id'] == i)}" for i in UNCOMPARED]
    return "\n".join(out) + "\n"


def na_pairs(rows, suite):
    return sorted(f"{r['cell']}\t{r['run']}\t{r['id']}" for r in ac_rows(rows, suite) if zero_weight(r))


def write_na_pairs(rows, suite):
    return "".join(line + "\n" for line in na_pairs(rows, suite))


# ---------------------------------------------------------------- main


def generate(corpus, repo):
    with tempfile.TemporaryDirectory() as tmp:
        suite, _model, _outcome = load_suite(repo, tmp)
        rows = derive_rows(corpus, suite)
        files = {"counts.toml": write_counts(rows, suite), "na_pairs.txt": write_na_pairs(rows, suite)}
    return {k: v.encode("utf-8") for k, v in files.items()}

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("corpus")
    ap.add_argument("repo")
    args = ap.parse_args()
    files = generate(args.corpus, args.repo)
    print(files["counts.toml"].decode())
    if args.check:
        bad = [n for n, b in files.items() if not os.path.isfile(os.path.join(HERE, n)) or open(os.path.join(HERE, n), "rb").read() != b]
        if bad:
            sys.exit(f"not regenerated byte for byte: {bad}")
        print("PASS: every generated file matches")
    else:
        for name, data in files.items():
            with open(os.path.join(HERE, name), "wb") as f:
                f.write(data)


if __name__ == "__main__":
    main()
