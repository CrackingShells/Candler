"""The protocol loop: read requests on stdin, answer on the private frame descriptor."""

from __future__ import annotations

import importlib
import importlib.metadata
import sys
from collections import deque
from collections.abc import Mapping, Sequence
from types import ModuleType
from typing import Any

from pydantic import ValidationError

from .frames import (
    FrameWriter,
    LineReader,
    ProtocolError,
    Refused,
    install_fd_guard,
    parse_message,
    summarise,
)
from .identity import Identity, IdentityError, identity
from .isolate import Abandoned, Done, make_runner, run_item
from .plugin import Plugin, Prior, RunContext

__all__ = ["main", "serve"]

PROTOCOL = {"major": 1, "minor": 0}
CANCELLED = -32800
INVALID_PARAMS = -32602
SERVER_ERROR = -32000
DEFAULT_DEADLINE_MS = 60_000
MAX_DEADLINE_MS = 2**53 - 1
# The helper knows neither the role nor the digests of a suite entry or a context, and the
# record the wire carries requires all three. The core, which owns them, overwrites these.
_ROLE = "descriptive"
_UNSET_DIGEST = "sha256:" + "0" * 64
_EMPTY_NEEDS: dict[str, Any] = {
    "trace": False,
    "artifacts": [],
    "snapshots": [],
    "inputs": [],
    "metadata": [],
}


class Inbox:
    """Parses stdin lines into messages and notices a `cancel` of the request in flight."""

    def __init__(self, reader: LineReader) -> None:
        self._reader = reader
        self._pending: deque[Any] = deque()
        self._current: str | None = None
        self._early_cancels: set[str] = set()
        self.cancelled = False

    def fileno(self) -> int | None:
        """The descriptor to wait on while an item runs, None once stdin has ended."""
        return self._reader.fileno()

    def pump(self) -> None:
        """Read whatever stdin holds without waiting."""
        self._reader.fill(0)
        self._drain()

    def begin(self, request_id: str) -> None:
        """Mark `request_id` as the request in flight."""
        self._current = request_id
        self.cancelled = request_id in self._early_cancels
        self._early_cancels.discard(request_id)

    def end(self) -> None:
        """Mark no request in flight."""
        self._current = None
        self.cancelled = False

    def next_message(self) -> Any:
        """The next request model, or a `Refused`, waiting for it; None at end of input."""
        while True:
            self._drain()
            if self._pending:
                return self._pending.popleft()
            if self._reader.eof:
                return None
            self._reader.fill(None)

    def _drain(self) -> None:
        while (line := self._reader.pop_line()) is not None:
            if not line.strip():
                continue
            try:
                msg = parse_message(line)
            except Refused as err:
                self._pending.append(err)
                continue
            except ProtocolError as err:
                _log(f"dropped a malformed line: {err}")
                continue
            method = getattr(msg, "method", None)
            if method == "cancel":
                target = msg.params.request_id.root
                if target == self._current:
                    self.cancelled = True
                else:
                    self._early_cancels.add(target)
            elif method in ("describe", "evaluate"):
                self._pending.append(msg)
            # Other notifications and responses carry nothing the helper acts on.


def _log(text: str) -> None:
    print(f"candler-evaluator: {text}", file=sys.stderr, flush=True)


def _error(request_id: str, code: int, message: str) -> dict[str, Any]:
    return {
        "jsonrpc": "2.0",
        "id": request_id,
        "error": {"code": code, "message": message},
    }


def _result(request_id: str, result: Mapping[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "id": request_id, "result": result}


def _fault_fields(note: str) -> dict[str, Any]:
    return {
        "status": "not_measured",
        "cause": {"reason": "instrument_error"},
        "evidence": [{"note": note}],
    }


def _checked(record: Mapping[str, Any]) -> dict[str, Any]:
    # Imported on first use: a process that only describes never builds the record model.
    from ._models.outcome_record import OutcomeRecord

    return OutcomeRecord.model_validate(record).model_dump(
        mode="json", exclude_none=True
    )


def _notify(method: str, params: Mapping[str, Any]) -> dict[str, Any]:
    return {"jsonrpc": "2.0", "method": method, "params": params}


def _packages() -> dict[str, str]:
    found: dict[str, str] = {}
    for dist in importlib.metadata.distributions():
        name = dist.metadata["Name"]
        if name:
            found.setdefault(name, dist.version)
    return dict(sorted(found.items()))


def _kind_specs(plugin: Plugin) -> list[dict[str, Any]]:
    specs = []
    for kind in sorted(plugin.kinds):
        given = plugin.specs.get(kind, {})
        specs.append(
            {
                "kind": kind,
                "params_schema": given.get("params_schema", {"type": "object"}),
                "needs": {**_EMPTY_NEEDS, **given.get("needs", {})},
                "default_deadline_ms": given.get(
                    "default_deadline_ms", DEFAULT_DEADLINE_MS
                ),
            }
        )
    return specs


class _ParamsError(ValueError):
    pass


def _parse_evaluate(
    params: Any,
) -> tuple[RunContext, list[dict[str, Any]], list[Any]]:
    """The context, items and prior records of validated `evaluate` params, as plain data.

    The model has checked every member; the one limit it does not carry is the largest
    deadline the helper can wait for.
    """
    wire = params.model_dump(mode="json", exclude_none=True)
    for item in wire["items"]:
        if item["deadline_ms"] > MAX_DEADLINE_MS:
            raise _ParamsError("`deadline_ms` is outside 0 to 2**53 - 1")
    context = RunContext.from_wire(wire["context"], wire["scratch"])
    return context, wire["items"], wire["prior"]


class _Loop:
    def __init__(self, plugin: Plugin, writer: FrameWriter, inbox: Inbox) -> None:
        self.plugin = plugin
        self.writer = writer
        self.inbox = inbox
        self.runner = make_runner([writer.fd])
        self._instruments: list[Identity] | None = None

    def _identities(self) -> list[Identity]:
        """The instruments' identities, read once, so every item carries the same ones."""
        if self._instruments is None:
            self._instruments = [identity(dist) for dist in self.plugin.instruments]
        return self._instruments

    def run(self) -> int:
        try:
            return self._serve()
        finally:
            self.runner.reap()

    def _serve(self) -> int:
        while (msg := self.inbox.next_message()) is not None:
            if isinstance(msg, Refused):
                self.writer.write(_error(msg.request_id, msg.code, str(msg)))
            elif msg.method == "describe":
                self.describe(msg.id.root)
            else:
                self.evaluate(msg.id.root, msg.params)
        return 0

    def _evaluator(self) -> tuple[str, str]:
        name, version = self.plugin.name, self.plugin.version
        if self.plugin.dist is not None:
            meta = importlib.metadata.distribution(self.plugin.dist)
            name = name or meta.metadata["Name"]
            version = version or meta.version
        if not name or not version:
            raise ValueError("the plug-in has no name or version")
        return name, version

    def describe(self, rid: str) -> None:
        try:
            name, version = self._evaluator()
            instruments = [i.as_instrument() for i in self._identities()]
        except (
            ValueError,
            importlib.metadata.PackageNotFoundError,
            IdentityError,
        ) as err:
            self.writer.write(_error(rid, SERVER_ERROR, str(err)))
            return
        result: Any = {
            "protocol": dict(PROTOCOL),
            "name": name,
            "version": version,
            "instruments": instruments,
            "environment": {"packages": _packages()},
            "kind": "deterministic" if self.plugin.deterministic else "probabilistic",
            "kinds": _kind_specs(self.plugin),
        }
        # Imported on first use: a process that only evaluates never builds this model.
        from ._models.evaluator_describe_result import DescribeResult

        try:
            result = DescribeResult.model_validate(result).model_dump(
                mode="json", exclude_none=True
            )
        except ValidationError as err:
            self.writer.write(_error(rid, SERVER_ERROR, summarise(err)))
            return
        self.writer.write(
            _notify(
                "log",
                {
                    "level": "info",
                    "message": f"isolation: {self.runner.mode}",
                    "isolation": self.runner.mode,
                },
            )
        )
        self.writer.write(_result(rid, result))

    def evaluate(self, rid: str, params: Any) -> None:
        try:
            context, items, prior = _parse_evaluate(params)
            name, version = self._evaluator()
            instruments = [i.as_instrument() for i in self._identities()]
        except _ParamsError as err:
            self.writer.write(_error(rid, INVALID_PARAMS, str(err)))
            return
        except (
            ValueError,
            importlib.metadata.PackageNotFoundError,
            IdentityError,
        ) as err:
            self.writer.write(_error(rid, SERVER_ERROR, str(err)))
            return
        self.inbox.begin(rid)
        records = list(prior)
        reported = 0
        for item in items:
            self.inbox.pump()
            if self.inbox.cancelled:
                break
            fields = self._run_item(item, context, records)
            if fields is None:
                break
            record = self._record(
                rid, f"{name}@{version}", instruments, context, item, fields
            )
            self.writer.write(_notify("outcome", {"record": record}))
            records.append(record)
            reported += 1
        cancelled = self.inbox.cancelled
        self.inbox.end()
        if cancelled:
            self.writer.write(_error(rid, CANCELLED, "request cancelled"))
        else:
            self.writer.write(_result(rid, {"reported": reported}))

    def _run_item(
        self,
        item: dict[str, Any],
        context: RunContext,
        records: Sequence[Mapping[str, Any]],
    ) -> dict[str, Any] | str | None:
        """The item's outcome fields, a fault reason, or None when the request was cancelled."""
        kind = self.plugin.kinds.get(item["kind"])
        if kind is None:
            return f"unknown kind {item['kind']!r}"
        priors = [Prior(r["expectation_id"], r) for r in records]
        result = run_item(
            self.runner,
            kind,
            item["params"],
            context,
            priors,
            item["deadline_ms"],
            self.inbox,
        )
        if isinstance(result, Abandoned):
            return None
        return result.outcome if isinstance(result, Done) else result.reason

    def _record(
        self,
        rid: str,
        evaluator: str,
        instruments: list[dict[str, Any]],
        context: RunContext,
        item: dict[str, Any],
        fields: dict[str, Any] | str,
    ) -> dict[str, Any]:
        record = self._build_record(rid, evaluator, instruments, context, item, fields)
        try:
            return _checked(record)
        except ValidationError as err:
            # The kind's outcome fits the protocol's rules but not the record's: the item
            # is not measured, and the fault is what the models said of it.
            fault = {
                **record,
                **_fault_fields(f"outcome record refused: {summarise(err)}"),
            }
            for key in ("value", "weight", "probability"):
                fault.pop(key, None)
            return _checked(fault)

    def _build_record(
        self,
        rid: str,
        evaluator: str,
        instruments: list[dict[str, Any]],
        context: RunContext,
        item: dict[str, Any],
        fields: dict[str, Any] | str,
    ) -> dict[str, Any]:
        if isinstance(fields, str):
            fields = _fault_fields(fields)
        return {
            "run": dict(context.run),
            "expectation_id": item["expectation_id"],
            "role": _ROLE,
            "evaluator": evaluator,
            "instruments": instruments,
            "suite_entry_digest": _UNSET_DIGEST,
            "context_digest": _UNSET_DIGEST,
            "request_id": rid,
            **fields,
        }


def serve(
    plugin: Plugin | ModuleType, *, writer: FrameWriter | None = None, stdin_fd: int = 0
) -> int:
    """Serve the evaluator protocol for `plugin` until stdin ends; returns the exit code.

    Without a `writer`, the fd guard is installed here: call `serve` (or `main`) before
    anything else may write to stdout.
    """
    if writer is None:
        writer = install_fd_guard()
    if isinstance(plugin, ModuleType):
        plugin = Plugin.from_module(plugin)
    loop = _Loop(plugin, writer, Inbox(LineReader(stdin_fd)))
    try:
        return loop.run()
    except BrokenPipeError:
        return 1


def main(argv: Sequence[str] | None = None) -> int:
    """`python -m candler_evaluator <plugin-module>`: guard the fds, import, serve."""
    args = list(sys.argv[1:] if argv is None else argv)
    if len(args) != 1:
        print("usage: python -m candler_evaluator <plugin-module>", file=sys.stderr)
        return 2
    writer = install_fd_guard()
    return serve(importlib.import_module(args[0]), writer=writer)
