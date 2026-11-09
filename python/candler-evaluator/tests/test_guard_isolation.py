import json

RID = "0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d8e9f"
CELL = "sha256:" + "ab" * 32
CONTEXT = {
    "run": {"cell": CELL, "replicate": 1, "attempt": 1},
    "root": "/runs/r1",
    "raw": [],
    "artifacts": [],
    "snapshots": [],
    "inputs": [],
    "metadata": {},
}
# One request mixing every synthetic kind with `ok` items: (id, kind, deadline_ms, status).
ITEMS = [
    ("A1", "ok", 10_000, "pass"),
    ("A2", "hang", 300, "not_measured"),
    ("A3", "print", 10_000, "pass"),
    ("A4", "exit", 10_000, "not_measured"),
    ("A5", "write1", 10_000, "pass"),
    ("A6", "subprocess_echo", 10_000, "pass"),
    ("A7", "ok", 10_000, "pass"),
]
STRAYS = [
    "stray print, not a frame",
    "stray os.write, not a frame",
    "stray child stdout, not a frame",
]
# Malformed evaluate requests the helper must refuse with -32602 and survive: (id, item edit).
BAD = [
    ("0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d0001", {"deadline_ms": "abc"}),
    ("0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d0002", {"deadline_ms": None}),
    ("0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d0003", {"deadline_ms": 10**400}),
    ("0192f4e0-7b3a-7c1d-8a2e-4f5b6c7d0004", {"kind": ["a"]}),
]
ENVELOPE = {"jsonrpc", "id", "method", "params", "result", "error"}


def is_frame(msg):
    """The envelope rules of the shared wire vectors, written out here from their README."""
    if not isinstance(msg, dict) or set(msg) - ENVELOPE:
        return False
    if msg.get("jsonrpc") != "2.0":
        return False
    if "id" in msg and not isinstance(msg["id"], str):
        return False
    if "method" in msg:
        return "result" not in msg and "error" not in msg and "params" in msg
    return "id" in msg and ("result" in msg) != ("error" in msg)


def load(line):
    try:
        return json.loads(line)
    except ValueError:
        return None


def test_one_fault_costs_one_item(vectors, run_helper):
    """Guards: R6 — a hang or an exit costs its own item and no other, a stray write costs nothing and never reaches the protocol pipe, and a malformed item is refused with -32602 while the helper serves the next request. Oracle: hand literals and the rules of the shared WIRE1 vectors."""
    # The checker is only an oracle if the shared vectors say so.
    for path in sorted((vectors / "valid").glob("*.json")):
        assert is_frame(json.loads(path.read_text())), path.name
    for path in sorted((vectors / "invalid").glob("*.json")):
        assert not is_frame(json.loads(path.read_text())), path.name

    params = {
        "context": CONTEXT,
        "items": [
            {"expectation_id": i, "kind": k, "params": {}, "deadline_ms": d}
            for i, k, d, _ in ITEMS
        ],
        "prior": [],
        "scratch": "/scratch/r1",
    }
    request = {"jsonrpc": "2.0", "id": RID, "method": "evaluate", "params": params}
    bad = []
    for bad_id, edit in BAD:
        item = {"expectation_id": "B1", "kind": "ok", "params": {}, "deadline_ms": 1}
        bad_params = {**params, "items": [{**item, **edit}]}
        bad.append(
            {"jsonrpc": "2.0", "id": bad_id, "method": "evaluate", "params": bad_params}
        )
    proc = run_helper("synthetic_plugin", [*bad, request])

    lines = proc.stdout.splitlines()
    frames = [load(line) for line in lines]
    assert [line for line, f in zip(lines, frames) if not is_frame(f)] == []
    assert not any(s in proc.stdout for s in STRAYS)
    assert all(s in proc.stderr for s in STRAYS)

    errors = [f for f in frames if "error" in f]
    assert [(f["id"], f["error"]["code"]) for f in errors] == [
        (i, -32602) for i, _ in BAD
    ]

    outcomes = [f["params"]["record"] for f in frames if f.get("method") == "outcome"]
    assert [(r["expectation_id"], r["status"]) for r in outcomes] == [
        (i, status) for i, _, _, status in ITEMS
    ]
    for record in outcomes:
        assert record["request_id"] == RID
        assert record["run"] == CONTEXT["run"]
        if record["status"] == "not_measured":
            assert record["cause"] == {"reason": "instrument_error"}
            assert record["evidence"][0]["note"]
        else:
            assert "cause" not in record
    assert frames[-1] == {"jsonrpc": "2.0", "id": RID, "result": {"reported": 7}}
    assert proc.returncode == 0
