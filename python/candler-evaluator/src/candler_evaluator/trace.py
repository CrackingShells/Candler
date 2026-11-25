"""Read a run's normalised trace as typed records."""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path
from typing import Any

from .plugin import ServedFile

__all__ = ["read_trace"]


def read_trace(trace: ServedFile | Path | str) -> Iterator[Any]:
    """Yield the records of a trace file, one per line, as the models of its schema.

    `trace` is `context.trace` or a path. A record is the header, a session or an event of
    `normalised-trace.schema.json`, held by the model of its `kind`. A line that is not JSON
    or that the models refuse (an unknown field, a `null` on an optional field, a wrong type)
    raises `pydantic.ValidationError` when the iterator reaches it; the records before it have
    been yielded.

    It checks lines, not the file's order: an empty file, a missing or second header, a
    `seq` gap, an event for an undeclared session and a `raw_ref` to a stream the header
    does not list all pass. Candler checks the order when it writes the trace, and the
    served copy is pinned by its sha256.
    """
    from ._models.normalised_trace import TraceRecord

    path = trace.abs if isinstance(trace, ServedFile) else Path(trace)
    with path.open(encoding="utf-8") as lines:
        for line in lines:
            yield TraceRecord.model_validate_json(line, strict=True).root
