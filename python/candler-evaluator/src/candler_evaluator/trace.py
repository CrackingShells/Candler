"""Read a run's normalised trace as typed records."""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path
from typing import Any

from .plugin import ServedFile

__all__ = ["TraceOrderError", "read_trace"]


class TraceOrderError(ValueError):
    """A trace file whose lines are not in the order Candler's reader accepts.

    `refusal_code` is the code of the refusal, a row of `[helper]` in `docs/contract/codes.toml`.
    """

    def __init__(self, message: str, *, refusal_code: str) -> None:
        super().__init__(message)
        self.refusal_code = refusal_code


def read_trace(trace: ServedFile | Path | str) -> Iterator[Any]:
    """Yield the records of a trace file, one per line, as the models of its schema.

    `trace` is `context.trace` or a path. A record is the header, a session or an event of
    `normalised-trace.schema.json`, held by the model of its `kind`. A line that is not JSON
    or that the models refuse (an unknown field, a `null` on an optional field, a wrong type)
    raises `pydantic.ValidationError` when the iterator reaches it; the records before it have
    been yielded.

    It refuses the file's order as Candler's reader does, with a `TraceOrderError`, a `ValueError`, naming the line:
    an empty file, a record before the header, a second header, a session after an event and
    a session declared twice. It does not check the rules that relate records: a `seq` gap, an
    event for an undeclared session, a child before its parent and a `raw_ref` to a stream the
    header does not list all pass.
    """
    from ._models.normalised_trace import TraceRecord

    path = trace.abs if isinstance(trace, ServedFile) else Path(trace)
    header_seen = False
    event_seen = False
    sessions: set[str] = set()
    with path.open(encoding="utf-8") as lines:
        number = 0
        for number, line in enumerate(lines, start=1):
            record = TraceRecord.model_validate_json(line, strict=True).root
            problem = None
            if record.kind == "header":
                if header_seen:
                    problem = "a second header"
                header_seen = True
            elif not header_seen:
                problem = f"{record.kind} before the header"
            elif record.kind == "event":
                event_seen = True
            elif event_seen:
                problem = f"session {record.id.root} after an event"
            elif record.id.root in sessions:
                problem = f"session {record.id.root} declared twice"
            else:
                sessions.add(record.id.root)
            if problem:
                raise TraceOrderError(
                    f"line {number}: {problem}",
                    refusal_code="helper.trace_out_of_order",
                )
            yield record
        if number == 0:
            raise TraceOrderError(
                "an empty trace file has no header",
                refusal_code="helper.trace_out_of_order",
            )
