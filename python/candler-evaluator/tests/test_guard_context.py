import json
import os
import subprocess
import sys
from pathlib import Path

import pytest
from pydantic import ValidationError

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


DIGEST_A = "sha256:" + "11" * 32
DIGEST_B = "sha256:" + "22" * 32
DIGEST_C = "sha256:" + "33" * 32


def served_wire():
    wire = wire_context()
    wire["trace"] = {
        "path": "trace.jsonl",
        "abs": "/runs/r1/trace.jsonl",
        "sha256": DIGEST_A,
    }
    wire["manifest"] = {
        "path": "manifest.json",
        "abs": "/runs/r1/manifest.json",
        "sha256": DIGEST_B,
    }
    wire["raw"] = [
        {"stream": "stdout", "path": "/runs/r1/raw/stdout.jsonl.gz", "sha256": DIGEST_C}
    ]
    return wire


def test_a_context_keeps_the_trace_raw_and_manifest_with_their_digests():
    """Guards: `from_wire` hands a kind each served part with its path and digest.

    Oracle: the wire context is written by hand from the `RunContext`, `ContextArtifact` and
    `ContextRaw` definitions of evaluator-evaluate-params.schema.json.
    Bite: `from_wire` drops `manifest`, or reads `trace` as a bare path.
    """
    context = RunContext.from_wire(served_wire(), "/scratch")
    assert context.trace is not None and context.manifest is not None
    assert (context.trace.path, context.trace.abs, context.trace.sha256) == (
        "trace.jsonl",
        Path("/runs/r1/trace.jsonl"),
        DIGEST_A,
    )
    assert (context.manifest.path, context.manifest.abs, context.manifest.sha256) == (
        "manifest.json",
        Path("/runs/r1/manifest.json"),
        DIGEST_B,
    )
    assert [(r["stream"], r["path"], r["sha256"]) for r in context.raw] == [
        ("stdout", "/runs/r1/raw/stdout.jsonl.gz", DIGEST_C)
    ]


def test_a_context_without_the_parts_has_none_and_nothing():
    """Guards: a part the run does not serve is `None`, and no raw stream is an empty tuple."""
    context = RunContext.from_wire(wire_context(), "/scratch")
    assert context.trace is None
    assert context.manifest is None
    assert context.raw == ()


HEADER = {
    "kind": "header",
    "schema": "normalised-trace/1",
    "reader": "claude-code-reader@0.1.0",
    "raw": [{"stream": "stdout", "digest": DIGEST_C}],
}
SESSION = {
    "kind": "session",
    "id": "s1",
    "source": {
        "harness": "claude-code",
        "harness_version": "2.1.0",
        "reader": "claude-code-reader",
        "reader_version": "0.1.0",
    },
    "ended_by": {"kind": "completed"},
    "raw_ref": {"stream": "stdout", "frame": 0},
}
CALL = {
    "kind": "event",
    "session": "s1",
    "event": {
        "seq": 0,
        "kind": {
            "kind": "tool_call",
            "name": "mcp__fs__read",
            "origin": {"kind": "mcp", "server": "fs", "tool": "read"},
            "argv": ["cat", "a.txt"],
            "wrapped_calls": [],
        },
        "raw_ref": {"stream": "stdout", "frame": 1},
        "ext": {},
    },
}
RESULT = {
    "kind": "event",
    "session": "s1",
    "event": {
        "seq": 1,
        "kind": {
            "kind": "tool_result",
            "is_error": True,
            "exit": {"kind": "code", "code": 2},
        },
        "raw_ref": {"stream": "stdout", "frame": 2},
        "ext": {},
    },
}


def write_trace(tmp_path, records):
    path = tmp_path / "trace.jsonl"
    path.write_text("".join(json.dumps(r) + "\n" for r in records))
    return path


def test_the_reader_refuses_a_null_on_an_optional_field(tmp_path):
    """Guards: the schema refuses `null` on an optional field, so the reader does.

    Oracle: `exit` of `ToolResult` in normalised-trace.schema.json is an `ExitStatus` object
    that is left out when there is none; the wire has no `null`, and Candler's reader refuses it.
    Bite: the generated models read `null` as absent.
    """
    from candler_evaluator import read_trace

    bad = json.loads(json.dumps(RESULT))
    bad["event"]["kind"]["exit"] = None
    path = write_trace(tmp_path, [HEADER, SESSION, bad])
    with pytest.raises(ValidationError, match="exit"):
        list(read_trace(path))
    absent = json.loads(json.dumps(RESULT))
    del absent["event"]["kind"]["exit"]
    path = write_trace(tmp_path, [HEADER, SESSION, absent])
    assert len(list(read_trace(path))) == 3


def test_the_reader_yields_typed_records_with_exit_origin_and_argv(tmp_path):
    """Guards: the trace reader types each line from normalised-trace.schema.json.

    Oracle: the lines are written by hand from the schema's `TraceRecord`, `EventKind`,
    `ToolOrigin` and `ExitStatus`.
    """
    from candler_evaluator import read_trace

    path = write_trace(tmp_path, [HEADER, SESSION, CALL, RESULT])
    records = list(read_trace(path))
    assert [r.kind for r in records] == ["header", "session", "event", "event"]
    call, result = records[2].event.kind.root, records[3].event.kind.root
    assert call.argv == ["cat", "a.txt"]
    assert (call.origin.root.kind, call.origin.root.server, call.origin.root.tool) == (
        "mcp",
        "fs",
        "read",
    )
    assert (result.exit.root.kind, result.exit.root.code) == ("code", 2)


def test_the_reader_takes_the_served_trace_of_a_context(tmp_path):
    """Guards: a kind passes `context.trace` itself to the reader."""
    from candler_evaluator import read_trace

    path = write_trace(tmp_path, [HEADER, SESSION])
    wire = served_wire()
    wire["trace"]["abs"] = str(path)
    context = RunContext.from_wire(wire, "/scratch")
    assert len(list(read_trace(context.trace))) == 2


def test_the_reader_refuses_a_line_with_an_unknown_field(tmp_path):
    """Guards: the schema refuses an unknown field, so the reader does.

    Oracle: `additionalProperties: false` on every object of normalised-trace.schema.json.
    Bite: the generated models accept extra fields.
    """
    bad = json.loads(json.dumps(RESULT))
    bad["event"]["kind"]["surprise"] = 1
    from candler_evaluator import read_trace

    path = write_trace(tmp_path, [HEADER, SESSION, bad])
    with pytest.raises(ValidationError):
        list(read_trace(path))


DESCRIBE_PLUGIN = """
NAME = "demo"
VERSION = "0.1.0"
KINDS = {"k": lambda params, context, prior: {"status": "pass"}}
KIND_SPECS = {"k": {"needs": NEEDS}}
"""


def describe_frames(tmp_path, needs, vectors):
    (tmp_path / "demo_plugin.py").write_text(f"NEEDS = {needs!r}\n" + DESCRIBE_PLUGIN)
    request = json.dumps(
        json.loads((vectors / "valid" / "request_describe.json").read_text())
    )
    out = subprocess.run(
        [sys.executable, "-m", "candler_evaluator", "demo_plugin"],
        input=request + "\n",
        capture_output=True,
        text=True,
        check=True,
        env={**os.environ, "PYTHONPATH": str(tmp_path), "PYTHONDONTWRITEBYTECODE": "1"},
        timeout=60,
    ).stdout
    return [json.loads(line) for line in out.splitlines()]


def described_needs(tmp_path, needs, vectors):
    (result,) = [f for f in describe_frames(tmp_path, needs, vectors) if "result" in f]
    return result["result"]["kinds"][0]["needs"]


def test_a_describe_result_never_carries_null_for_an_unset_need(tmp_path, vectors):
    """Guards: `raw` and `manifest` are written as booleans or left out, never `null`; a plug-in
    that declares `null` for one is refused with error -32000 naming it.

    Oracle: response_describe_needs_raw_manifest.json shows the wire form, and
    invalid/needs_raw_not_boolean.json shows the reader refusing anything but a boolean.
    Bite: the writer emits `null` for an unset need, or reads a declared `null` as absent.
    """
    oracle = json.loads(
        (vectors / "valid" / "response_describe_needs_raw_manifest.json").read_text()
    )["result"]["kinds"][0]["needs"]
    assert oracle["raw"] is True and oracle["manifest"] is True
    for needs in ({}, {"raw": True, "manifest": True}):
        wrote = described_needs(tmp_path, needs, vectors)
        assert None not in wrote.values()
        for key in ("raw", "manifest"):
            assert wrote.get(key, False) is bool(needs.get(key))
    (refusal,) = describe_frames(tmp_path, {"raw": None}, vectors)
    assert refusal["error"]["code"] == -32000
    assert "needs.raw" in refusal["error"]["message"]
