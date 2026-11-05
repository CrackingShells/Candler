"""Add the synthetic cell to the mini-corpus built by select_mini.py.

    uv run python make_synthetic.py <mini-corpus>

The synthetic cell `synthetic_cell` holds three runs written from the legacy format as R01
describes it, never captured from a writer:

* `synthetic_contaminated` (run-1): original integrity verdict `contaminated`;
* `synthetic_text_value` (run-2): a text-valued row, an integer-literal row, a float-literal row
  and an exponent-literal row;
* `synthetic_not_applicable` (run-3): a row with the legacy status `not_applicable`.

Assumed shapes, where R01 leaves a spelling open: a `not_applicable` grading entry has the keys of
any other entry with `score` 0 and `max_score` 0; a `contaminated` verdict carries one reason with
code `input_leak`; its catalog line has the keys of an `incomplete` line.

The script is idempotent: it removes the synthetic cell and its rows before writing them.
"""

import gzip
import hashlib
import importlib.util
import json
import os
import shutil
import sys
import tomllib

sys.dont_write_bytecode = True

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("select_mini", os.path.join(HERE, "select_mini.py"))
select_mini = importlib.util.module_from_spec(spec)
spec.loader.exec_module(select_mini)

CELL = "synthetic_cell"
MODEL = "claude-synthetic-1"
VERSION = "v0.0.0-synthetic"
STARTED = "2026-08-06T00:00:00+00:00"
EVAL_DIRS = {
    1: "eval-1-tier-a-domain-structure",
    5: "eval-5-descriptive-annex-quantitative-metrics",
}
FIRST = EVAL_DIRS[1]


def entry(eid, tier, role, scale, status, score, max_score, evidence, value=None, extra=None):
    """One grading entry as text, so the number token of `value` is written exactly as given."""
    parts = [
        f'"id":{json.dumps(eid)}',
        f'"tier":{json.dumps(tier)}',
        f'"text":{json.dumps("synthetic expectation " + eid)}',
        f'"role":{json.dumps(role)}',
        f'"scale":{json.dumps(scale)}',
        f'"status":{json.dumps(status)}',
        f'"passed":{"true" if status == "pass" else "false"}',
        f'"evidence":{json.dumps(evidence)}',
    ]
    if extra:
        parts.append(extra)
    if value is not None:
        parts.append(f'"value":{value}')
    parts += [f'"score":{score}', f'"max_score":{max_score}']
    return "{" + ",".join(parts) + "}"


def grading(eval_id, title, tier, entries):
    return (
        f'{{"eval_id":{eval_id},"title":{json.dumps(title)},"tier":{json.dumps(tier)},'
        f'"expectations":[{",".join(entries)}],'
        f'"summary":{{"passed":0,"failed":0,"total":{len(entries)},"pass_rate":0.0}}}}\n'
    )


# (run number, name, verdict, {eval number: (title, tier, [entries])})
RUNS = [
    (
        1,
        "synthetic_contaminated",
        "contaminated",
        {
            1: (
                "tier-a-domain-structure",
                "structure",
                [
                    entry("A1", "structure", "gate", "binary", "pass", 1, 1, "exit_code=0"),
                    entry("A3", "structure", "quality", "binary", "pass", 1, 1, "match=found"),
                ],
            ),
        },
    ),
    (
        2,
        "synthetic_text_value",
        "clean",
        {
            1: (
                "tier-a-domain-structure",
                "structure",
                [entry("A1", "structure", "gate", "binary", "pass", 1, 1, "exit_code=0")],
            ),
            5: (
                "descriptive-annex-quantitative-metrics",
                "descriptive",
                [
                    entry("C1", "descriptive", "descriptive", "binary", "pass", 1, 1, "synthetic", '"three actions, two methods"'),
                    entry("C2", "descriptive", "descriptive", "binary", "pass", 1, 1, "synthetic", "14"),
                    entry("C3", "descriptive", "descriptive", "binary", "pass", 1, 1, "synthetic", "2.50"),
                    entry("C4", "descriptive", "descriptive", "binary", "pass", 1, 1, "synthetic", "1e2"),
                ],
            ),
        },
    ),
    (
        3,
        "synthetic_not_applicable",
        "clean",
        {
            1: (
                "tier-a-domain-structure",
                "structure",
                [
                    entry("A1", "structure", "gate", "binary", "pass", 1, 1, "exit_code=0"),
                    entry("A2", "structure", "gate", "binary", "not_applicable", 0, 0, "not applicable to this arm"),
                ],
            ),
        },
    ),
]


# The inputs every run of the cell copied, in the shape of the real `timing.json`.
COPIED_INPUTS = [
    {
        "relpath": "scripts/synthetic_check.py",
        "sha256": hashlib.sha256(b"synthetic script").hexdigest(),
        "role": "script",
    },
    {
        "relpath": ".claude/skills/synthetic/SKILL.md",
        "sha256": hashlib.sha256(b"synthetic skill").hexdigest(),
        "role": "skill",
    },
]


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    mode = "wb" if isinstance(data, bytes) else "w"
    with open(path, mode, **({} if mode == "wb" else {"encoding": "utf-8"})) as f:
        f.write(data)


def stream(n):
    lines = [
        {
            "type": "system",
            "subtype": "init",
            "cwd": "/tmp/synthetic",
            "session_id": f"00000000-0000-4000-8000-00000000000{n}",
            "tools": ["Bash", "Read", "Write"],
            "mcp_servers": [],
            "model": MODEL,
            "permissionMode": "bypassPermissions",
            "claude_code_version": "2.1.220",
        },
        {
            "type": "assistant",
            "message": {
                "model": MODEL,
                "id": f"msg_synthetic_{n}",
                "type": "message",
                "role": "assistant",
                "content": [{"type": "text", "text": "Synthetic run."}],
            },
            "session_id": f"00000000-0000-4000-8000-00000000000{n}",
        },
        {
            "type": "result",
            "subtype": "success",
            "is_error": False,
            "num_turns": 1,
            "session_id": f"00000000-0000-4000-8000-00000000000{n}",
        },
    ]
    raw = ("\n".join(json.dumps(x, separators=(",", ":")) for x in lines) + "\n").encode("utf-8")
    return gzip.compress(raw, mtime=0)


def build(mini):
    for rel in ("expectations.jsonl", "PROVENANCE.json", "PROVENANCE.toml"):
        if not os.path.isfile(os.path.join(mini, rel)):
            sys.exit(f"{mini} lacks {rel}: run select_mini.py first")
    cell_dir = os.path.join(mini, CELL)
    shutil.rmtree(cell_dir, ignore_errors=True)

    with open(os.path.join(mini, "PROVENANCE.toml"), "rb") as f:
        toml = tomllib.load(f)
    real_runs = [r for r in toml["runs"] if not r["synthetic"]]
    base_cell = real_runs[0]["cell"]
    with open(os.path.join(mini, base_cell, "experiment.json"), encoding="utf-8") as f:
        experiment = json.load(f)
    experiment["resolved_skill_version"] = VERSION
    experiment["provenance"]["skill_version"] = VERSION
    experiment["plan"]["skill_version"] = f"htn-creator@{VERSION}"
    experiment["plan"]["model"] = MODEL
    experiment["plan"]["runs"] = len(RUNS)
    experiment["plan"]["out_dir"] = f".claude/skills/htn-creator-workspace/results/{CELL}"
    write(os.path.join(cell_dir, "experiment.json"), json.dumps(experiment, indent=2) + "\n")

    with open(os.path.join(mini, base_cell, "experiment.json"), encoding="utf-8") as f:
        plan_prompt = json.load(f)["plan"]["prompt_domain_name"]

    progress = [{"kind": "cell_start", "elapsed_seconds": 0.5, "config": ["with_skill"], "model": MODEL, "runs": len(RUNS)}]
    catalog = []
    prov_runs = []
    rows = []
    entries = []

    for n, name, verdict, evals in RUNS:
        progress.append({"kind": "run_start", "elapsed_seconds": 10.0 * n, "config": "with_skill", "run_number": n})
        progress.append({"kind": "run_complete", "elapsed_seconds": 10.0 * n + 5, "config": "with_skill", "run_number": n})
        cache = os.path.join(CELL, "_run_cache/with_skill", f"run-{n}")
        write(os.path.join(mini, cache, "agent_stream.jsonl.gz"), stream(n))
        write(os.path.join(mini, cache, "agent_stderr.txt"), "")
        write(
            os.path.join(mini, cache, "run_complete.json"),
            json.dumps({"config": "with_skill", "run_number": n, "completed_at": f"2026-08-06T00:00:{10 + n:02d}+00:00"}, indent=2) + "\n",
        )
        for out_name in ("domain.py", "problems.py"):
            write(os.path.join(mini, cache, "outputs", out_name), f"# synthetic fixture {name}\n")
        for eval_n, title_dir in EVAL_DIRS.items():
            meta = {
                "eval_id": eval_n,
                "title": title_dir.split("-", 2)[2],
                "configuration": "with_skill",
                "skill_version": VERSION,
                "run_number": n,
                "model": MODEL,
                "prompt": "synthetic prompt",
                "timestamp": STARTED,
                "domain_name": plan_prompt,
                "base_url": None,
                "provenance": {
                    "skill_version": VERSION,
                    "eval_infra": experiment["provenance"]["eval_infra"],
                    "sandbox_contract": experiment["provenance"]["sandbox_contract"],
                    "planner": {
                        "runtime": experiment["provenance"]["planner"]["runtime"],
                        "sandbox": {"distribution": "gtpyhop", "version": "1.9.7"},
                    },
                },
            }
            write(
                os.path.join(mini, CELL, title_dir, "with_skill", f"run-{n}", "eval_metadata.json"),
                json.dumps(meta, separators=(",", ":")) + "\n",
            )
        write(os.path.join(mini, CELL, FIRST, "with_skill", f"run-{n}", "metrics.json"), '{"action_count":1}\n')
        write(
            os.path.join(mini, CELL, FIRST, "with_skill", f"run-{n}", "timing.json"),
            json.dumps({"copied_inputs": COPIED_INPUTS}, separators=(",", ":")) + "\n",
        )
        for eval_n, (title, tier, ents) in evals.items():
            fname = f"eval-{eval_n}-grading.json"
            write(os.path.join(mini, CELL, FIRST, "with_skill", f"run-{n}", fname), grading(eval_n, title, tier, ents))
            for e in ents:
                d = json.loads(e)
                rows.append(
                    json.dumps(
                        {
                            "cell": CELL,
                            "config": "with_skill",
                            "run": f"run-{n}",
                            "eval": fname,
                            "id": d["id"],
                            "role": d["role"],
                            "status": d["status"],
                            "value": d.get("value"),
                            "score": d["score"],
                            "max_score": d["max_score"],
                        }
                    )
                )
        reasons = (
            [{"code": "input_leak", "filename": "domain.py", "evidence": ["synthetic leak"]}]
            if verdict == "contaminated"
            else []
        )
        if verdict != "clean":
            catalog.append(
                {
                    "timestamp": f"2026-08-06T00:00:{10 + n:02d}+00:00",
                    "config": "with_skill",
                    "run_number": n,
                    "model": MODEL,
                    "domain_name": plan_prompt,
                    "verdict": verdict,
                    "reasons": reasons,
                }
            )
        prov_runs.append(
            {
                "cell": CELL,
                "config": "with_skill",
                "run": f"run-{n}",
                "model": MODEL,
                "skill_version": VERSION,
                "tier_rates": {"tier_A_pass_rate": 1.0},
                "instructed_coverage_status": "measured",
                "original_integrity": {"verdict": verdict, "reasons": reasons},
            }
        )

    write(os.path.join(cell_dir, "progress.jsonl"), "".join(json.dumps(p) + "\n" for p in progress))
    write(os.path.join(cell_dir, "contamination_catalog.jsonl"), "".join(json.dumps(c) + "\n" for c in catalog))
    for name in ("domain.py", "problems.py"):
        write(os.path.join(cell_dir, "shared_with_skill", name), "# synthetic fixture\n")

    # expectations.jsonl: drop earlier synthetic rows, append these.
    path = os.path.join(mini, "expectations.jsonl")
    with open(path, encoding="utf-8") as f:
        kept = [l for l in f.read().splitlines() if l and json.loads(l)["cell"] != CELL]
    write(path, "".join(l + "\n" for l in kept + rows))

    # PROVENANCE.json: drop earlier synthetic runs, append these.
    path = os.path.join(mini, "PROVENANCE.json")
    with open(path, encoding="utf-8") as f:
        prov = json.load(f)
    prov["runs"] = [r for r in prov["runs"] if r["cell"] != CELL] + prov_runs
    prov["corpus"]["cells"] = [c for c in prov["corpus"]["cells"] if c != CELL] + [CELL]
    write(path, json.dumps(prov, indent=2, ensure_ascii=False) + "\n")

    # PROVENANCE.toml: rewrite with the synthetic entries.
    files = [e for e in toml["files"] if not e["path"].startswith(CELL + "/")]
    out_entries = []
    for e in files:
        source = e["source"]
        if e["path"] in ("expectations.jsonl", "PROVENANCE.json") and "synthetic rows" not in source:
            source += "; plus the synthetic rows of make_synthetic.py"
        out_entries.append((e["path"], source, e["owner"]))
    for dirpath, _, names in os.walk(cell_dir):
        for name in names:
            rel = os.path.relpath(os.path.join(dirpath, name), mini)
            out_entries.append((rel, "synthetic: make_synthetic.py", "synthetic"))
    runs = [dict(r) for r in real_runs] + [
        {"name": name, "cell": CELL, "run": f"run-{n}", "synthetic": True}
        for n, name, _, _ in RUNS
    ]
    # the derived aggregates changed: record their new digests
    select_mini.write_provenance_toml(mini, runs, out_entries)
    print(f"wrote {CELL}: {len(rows)} rows, {len(RUNS)} runs")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    build(sys.argv[1])
