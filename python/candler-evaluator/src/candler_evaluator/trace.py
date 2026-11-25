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
    or that the schema refuses, an unknown field included, raises `pydantic.ValidationError`
    when the iterator reaches it; the records before it have been yielded.
    """
    from ._models.normalised_trace import TraceRecord

    path = trace.abs if isinstance(trace, ServedFile) else Path(trace)
    with path.open(encoding="utf-8") as lines:
        for line in lines:
            yield TraceRecord.model_validate_json(line, strict=True).root
