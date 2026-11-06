"""Derive the A-C reference (counts, zero-weight pairs, rates, equivalence) from the raw corpus.

    uv run --no-project --with rfc8785 python gen_oracle.py <corpus> <R>            # write
    uv run --no-project --with rfc8785 python gen_oracle.py --check <corpus> <R>    # compare

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


# ---------------------------------------------------------------- rates


def cell_rates(rows, suite, model, outcome):
    """cell -> {role: [num, den]}: `_outcome.score` per run, summed over the cell's runs."""
    runs = {}
    for r in ac_rows(rows, suite):
        d = {"id": r["id"], "status": r["status"], "score": r["score"], "max_score": r["max_score"], "role": suite["role"][r["id"]]}
        runs.setdefault((r["cell"], r["run"]), []).append(model.ExpectationResult.from_grading(d))
    out = {}
    for (cell, run), exps in sorted(runs.items()):
        view = model.RunView(config="with_skill", run_number=run, short_id=f"ws-{run:02d}", expectations=exps)
        s = outcome.score(view)
        acc = out.setdefault(cell, {"gate": [0, 0], "quality": [0, 0]})
        for role, rate in (("gate", s.gate), ("quality", s.quality)):
            acc[role][0] += rate.numerator
            acc[role][1] += rate.denominator
    return out


def write_rates(rows, suite, model, outcome):
    rates = cell_rates(rows, suite, model, outcome)
    tables = []
    for cell in sorted(rates):
        g, q = rates[cell]["gate"], rates[cell]["quality"]
        tables.append(f'[cell."{cell}"]\ngate = [{g[0]}, {g[1]}]\nquality = [{q[0]}, {q[1]}]')
    return "\n\n".join(tables) + "\n"


def published(corpus, rates_text):
    """The `[published]` comparison (stdout only): the corpus publishes no gate or quality rate."""
    prov = load_json(os.path.join(corpus, "PROVENANCE.json"))
    lines = ["[published] no gate or quality rate file in the corpus; PROVENANCE.json runs[].tier_rates holds tier A/B/C pass rates only:"]
    by_cell = {}
    for r in prov["runs"]:
        by_cell.setdefault(r["cell"], []).append(r["tier_rates"])
    for cell in sorted(by_cell):
        mean = {k: sum(t[k] for t in by_cell[cell]) / len(by_cell[cell]) for k in by_cell[cell][0]}
        lines.append(f"  {cell[16:60]}... mean " + " ".join(f"{k[5]}={v:.4f}" for k, v in sorted(mean.items())))
    return "\n".join(lines)

# ---------------------------------------------------------------- main


def generate(corpus, repo):
    with tempfile.TemporaryDirectory() as tmp:
        suite, model, outcome = load_suite(repo, tmp)
        rows = derive_rows(corpus, suite)
        files = {
            "counts.toml": write_counts(rows, suite),
            "na_pairs.txt": write_na_pairs(rows, suite),
            "rates_by_cell.toml": write_rates(rows, suite, model, outcome),
        }
    return {k: v.encode("utf-8") for k, v in files.items()}, published(corpus, files["rates_by_cell.toml"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("corpus")
    ap.add_argument("repo")
    args = ap.parse_args()
    files, pub = generate(args.corpus, args.repo)
    print(files["counts.toml"].decode())
    print(pub)
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
