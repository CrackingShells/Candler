from candler_evaluator.plugin import RunContext

CELL = "sha256:" + "ab" * 32


def wire_context():
    return {
        "run": {"cell": CELL, "replicate": 1, "attempt": 1},
        "root": "/runs/r1",
        "raw": [],
        "artifacts": [],
        "inputs": [],
        "metadata": {},
    }


def test_a_context_without_snapshots_is_accepted():
    """Guards: the helper reads `snapshots` as optional, so a context that omits the key builds.

    Oracle: the context is written by hand with every member of the wire but `snapshots`.
    Bite: read `context["snapshots"]` again and this raises KeyError.
    """
    context = RunContext.from_wire(wire_context(), "/scratch")
    assert context.snapshots == ()


def test_a_context_with_snapshots_keeps_them():
    wire = wire_context()
    wire["snapshots"] = [{"kind": "catalog", "path": "/s/c", "digest": CELL}]
    context = RunContext.from_wire(wire, "/scratch")
    assert [entry["kind"] for entry in context.snapshots] == ["catalog"]
