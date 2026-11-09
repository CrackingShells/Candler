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
import gzip
import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
import types

import rfc8785

HERE = os.path.dirname(os.path.abspath(__file__))
M1_VECTORS = os.path.join(HERE, "..", "..", "..", "candler-core", "tests", "oracle", "vectors", "cell_digest.toml")
PIN = "28be2c15"
INFRA = ".claude/skills/htn-creator-workspace/infrastructure"
EVAL_JSON_SHA = "ca7f12afee55152ed0e543a1d9b457c29fc5c02fd49e1ec6a417a9dcc1c97f9e"
GENERATOR = "crates/candler/tests/oracle/gen_oracle.py"
UNCOMPARED = ("D-E1", "D-E2", "D-E3", "D-E4", "D-E5", "D-E6")
STATUSES = ("pass", "fail", "blocked", "not_measured", "not_applicable")
# The one excused change, transcribed from Eliott's ruling of 2026-11-06 (R03 §Generic diff).
RULE_NAME = "legacy-zero-weight-pass"
RULE_FROM = {"status": "pass", "weight": {"num": 0, "den": 0}, "cause": {"legacy": "zero_weight"}}
RULE_TO = {"status": "not_applicable", "cause": {"reason": "empty_population"}}


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


def na_pairs(rows, suite, hexes):
    return sorted(f"{hexes[r['cell']]}\t{r['run']}\t{r['id']}" for r in ac_rows(rows, suite) if zero_weight(r))


def write_na_pairs(rows, suite, hexes):
    return "".join(line + "\n" for line in na_pairs(rows, suite, hexes))


def write_cells(hexes):
    """`cells.tsv`: `<cell hex>\\t<cell dir>`, sorted bytewise."""
    return "".join(f"{h}\t{c}\n" for h, c in sorted((h, c) for c, h in hexes.items()))


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


def write_rates(rows, suite, model, outcome, hexes):
    rates = {hexes[c]: v for c, v in cell_rates(rows, suite, model, outcome).items()}
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


# ---------------------------------------------------------------- cell digest and equivalence


def leaves(value, prefix, out):
    if isinstance(value, dict):
        for key, inner in value.items():
            leaves(inner, f"{prefix}.{key}" if prefix else key, out)
    else:
        out.append((prefix, value))


def cell_comparable(corpus, cell):
    """M1's ASSUMED 1 rule (`gen_vectors.py` mini_cell), re-written for the golden layout."""
    root = os.path.join(corpus, cell)
    experiment = load_json(os.path.join(root, "experiment.json"))
    catalog_sha = load_json(os.path.join(corpus, "PROVENANCE.json"))["catalog"]["sha256"]
    found = []
    leaves(experiment["plan"], "plan", found)
    leaves(experiment["provenance"], "provenance", found)
    found.append(("resolved_skill_version", experiment["resolved_skill_version"]))
    values = [{"path": p} if v is None else {"path": p, "value": v} for p, v in found if p != "plan.out_dir"]
    versions = set()
    for path in sorted(glob.glob(os.path.join(root, "_run_cache", "with_skill", "run-*", "agent_stream.jsonl.gz"))):
        with gzip.open(path, "rt", encoding="utf-8") as f:
            init = json.loads(f.readline())
        assert init["subtype"] == "init", path
        versions.add(init["claude_code_version"])
    assert len(versions) == 1, (cell, versions)
    inputs = set()
    for dirpath, _dirs, names in os.walk(root):
        if "timing.json" in names:
            copied = load_json(os.path.join(dirpath, "timing.json"))["copied_inputs"]
            inputs.add(jtext([{"kind": c["role"], "digest": "sha256:" + c["sha256"]} for c in copied]))
    assert len(inputs) == 1, (cell, len(inputs))
    name, tag = experiment["plan"]["skill_version"].rsplit("@", 1)
    c = {
        "values": sorted(values, key=lambda v: v["path"]),
        "harness": {"name": "claude-code", "version": versions.pop()},
        "treatment": {"name": name, "tag": tag},
        "snapshots": [{"kind": "catalog", "digest": "sha256:" + catalog_sha}],
        "inputs": sorted(json.loads(inputs.pop()), key=lambda r: (r["kind"], r["digest"])),
        "environments": [],
    }
    return json.loads(jtext(c))


def cell_digest(corpus, cell):
    """The 64 bare hex digits of sha256 over the RFC 8785 form of the cell's `comparable`."""
    return hashlib.sha256(rfc8785.dumps(cell_comparable(corpus, cell))).hexdigest()


def equivalence_rows(rows, suite, hexes):
    out = []
    for line in na_pairs(rows, suite, hexes):
        cell, run, exp = line.split("\t")
        out.append((f"{cell}-r{run}-a1", exp))
    return sorted(out)


def write_equivalence(rows, suite, hexes):
    rule = [f'      "name": {json.dumps(RULE_NAME)},', f'      "from": {jtext(RULE_FROM)},', f'      "to": {jtext(RULE_TO)},']
    body = ",\n".join(f'        {{"run": {json.dumps(r)}, "expectation": {json.dumps(e)}}}' for r, e in equivalence_rows(rows, suite, hexes))
    lines = ["{", f'  "generator": {json.dumps(GENERATOR)},', '  "rules": [', "    {", *rule, '      "rows": [', body, "      ]", "    }", "  ]", "}"]
    return "\n".join(lines) + "\n"


def check_m1_vectors(corpus):
    """Cross-check: the real cells of M1's `cell_digest.toml` get the digest recorded there."""
    with open(M1_VECTORS, "rb") as f:
        vectors = tomllib.load(f)["vector"]
    names = set(cells_of(corpus))
    hit = [v for v in vectors if v["cell"] in names]
    assert len(hit) == 2, [v["cell"] for v in hit]
    for v in hit:
        want = v["digest"]
        got = "sha256:" + cell_digest(corpus, v["cell"])
        if got != want or jtext(cell_comparable(corpus, v["cell"])) != v["comparable"]:
            sys.exit(f"cell digest of {v['cell']} is {got}, M1 recorded {want}: stop and report")
    return len(hit)


README = """# Reference oracle: expected A-C rows and rates

Everything here is generated by `gen_oracle.py` from the raw golden corpus and three files of the
HTN repo at commit 28be2c15 (`eval.json`, `_model.py`, `_outcome.py`). It imports nothing from
Candler and reads no importer output. Regenerate, or verify byte for byte:

    uv run --no-project --with rfc8785 python crates/candler/tests/oracle/gen_oracle.py "$GOLDEN" "$R"
    uv run --no-project --with rfc8785 python crates/candler/tests/oracle/gen_oracle.py --check "$GOLDEN" "$R"

- `counts.toml`: status counts of the 8,880 A-C rows as imported (`[imported]`, with `zero_weight`
  the passes whose raw `max_score` is 0), as expected live (`[live_expected]`: those passes become
  `not_applicable`), the uncompared D-E1..D-E6 counts, and `total` (all rows).
- `na_pairs.txt`: `<cell hex>\\t<run N>\\t<expectation id>` for each of those 209 passes, sorted
  bytewise. The cell hex is the 64 bare hex digits of the cell digest (see `equivalence.json`).
- `cells.tsv`: `<cell hex>\\t<cell dir>` for each of the 12 cells, sorted bytewise: the only place
  that names a cell by directory.
- `rates_by_cell.toml`: per cell hex (`[cell."<cell hex>"]`, sorted by hex), `gate` and `quality` as `[numerator, denominator]` sums of
  `_outcome.score` over the cell's runs (roles from `eval.json`; `not_measured` and
  `not_applicable` excluded, a 0/0 pass adds nothing). Tables are separated by one blank line,
  keys are sorted, and the file ends with one newline: `candler show --rates` must print it exactly.
- `equivalence.json`: one rule, `legacy-zero-weight-pass`, whose `rows` are the pairs of
  `na_pairs.txt` keyed `<cell hex>-r<N>-a1`. The cell hex is the sha256 of the RFC 8785 form of
  the cell's `comparable`, built from R01's text as M1's `gen_vectors.py` does (`--check` also
  verifies the two real cells of M1's `cell_digest.toml`). A wrong digest makes GP2 fail loudly as
  `equivalence_missing`.
- `README.md` is generated too.
"""


# ---------------------------------------------------------------- main


def generate(corpus, repo):
    with tempfile.TemporaryDirectory() as tmp:
        suite, model, outcome = load_suite(repo, tmp)
        rows = derive_rows(corpus, suite)
        hexes = {c: cell_digest(corpus, c) for c in cells_of(corpus)}
        assert len(set(hexes.values())) == len(hexes)
        files = {
            "counts.toml": write_counts(rows, suite),
            "na_pairs.txt": write_na_pairs(rows, suite, hexes),
            "cells.tsv": write_cells(hexes),
            "rates_by_cell.toml": write_rates(rows, suite, model, outcome, hexes),
            "equivalence.json": write_equivalence(rows, suite, hexes),
            "README.md": README,
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
        print(f"cell digests of {check_m1_vectors(args.corpus)} M1 vector cells match")
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
