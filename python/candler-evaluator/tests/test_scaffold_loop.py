"""Development tests for the loop; deleted by the last commit."""

import json
import os
import subprocess
import sys
import textwrap

import pytest

pytestmark = pytest.mark.scaffold

PLUGIN = textwrap.dedent(
    """
    import os
    NAME = "scaf"
    VERSION = "0.1.0"
    def ok(params, context, prior):
        print("stray")
        os.write(1, b"stray2\\n")
        return {"status": "pass"}
    def echo(params, context, prior):
        return {"status": "pass", "evidence": [{"note": f"{len(prior)}:{context.artifacts}"}]}
    KINDS = {"ok": ok, "echo": echo}
    """
)
ZERO = "sha256:" + "0" * 64
CTX = {
    "run": {"cell": "sha256:" + "ab" * 32, "replicate": 1, "attempt": 1},
    "root": "/r",
    "raw": [],
    "artifacts": [
        {"path": "b", "abs": "/r/b", "sha256": ZERO},
        {"path": "a", "abs": "/r/a", "sha256": ZERO},
    ],
    "snapshots": [],
    "inputs": [],
    "metadata": {},
}
RID = "0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d8e9f"


def item(i, kind):
    return {"expectation_id": i, "kind": kind, "params": {}, "deadline_ms": 1000}


def evaluate(items):
    params = {"context": CTX, "scratch": "/s", "prior": [], "items": items}
    return {"jsonrpc": "2.0", "id": RID, "method": "evaluate", "params": params}


def run(tmp_path, lines):
    (tmp_path / "scaf.py").write_text(PLUGIN)
    env = {**os.environ, "PYTHONPATH": str(tmp_path), "PYTHONDONTWRITEBYTECODE": "1"}
    text = "\n".join(x if isinstance(x, str) else json.dumps(x) for x in lines)
    return subprocess.run(
        [sys.executable, "-m", "candler_evaluator", "scaf"],
        check=False,
        input=text + "\n",
        capture_output=True,
        text=True,
        env=env,
        timeout=30,
    )


def test_describe_and_evaluate(tmp_path):
    d = {"jsonrpc": "2.0", "id": RID, "method": "describe", "params": {}}
    ev = evaluate([item("A1", "ok"), item("A2", "echo"), item("A3", "nope")])
    p = run(tmp_path, [d, "garbage", ev])
    frames = [json.loads(x) for x in p.stdout.splitlines()]
    assert frames[0]["params"]["isolation"] == "fork"
    assert frames[1]["result"]["name"] == "scaf"
    assert [f.get("method") for f in frames[2:5]] == ["outcome"] * 3
    assert frames[3]["params"]["record"]["evidence"][0]["note"] == "1:('a', 'b')"
    assert frames[4]["params"]["record"]["status"] == "not_measured"
    assert frames[5]["result"] == {"reported": 3}
    assert "stray2" in p.stderr


def test_cancel_answered(tmp_path):
    cancel = {"jsonrpc": "2.0", "method": "cancel", "params": {"request_id": RID}}
    p = run(tmp_path, [evaluate([item("A1", "ok")]), cancel])
    frames = [json.loads(x) for x in p.stdout.splitlines()]
    assert frames[-1]["error"]["code"] == -32800


def test_no_fork_mode(tmp_path, monkeypatch):
    monkeypatch.setenv("CANDLER_EVALUATOR_NO_FORK", "1")
    d = {"jsonrpc": "2.0", "id": RID, "method": "describe", "params": {}}
    p = run(tmp_path, [d, evaluate([item("A1", "ok")])])
    frames = [json.loads(x) for x in p.stdout.splitlines()]
    assert frames[0]["params"]["isolation"] == "inline"
    assert frames[-1]["result"] == {"reported": 1}
