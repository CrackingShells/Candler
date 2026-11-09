"""The protocol pipe: frames out, lines in, and the guard that keeps stdout clean."""

from __future__ import annotations

import json
import os
import select
import sys
from collections.abc import Mapping
from typing import Any

__all__ = [
    "FrameWriter",
    "LineReader",
    "ProtocolError",
    "install_fd_guard",
    "parse_message",
]

_ENVELOPE_KEYS = frozenset({"jsonrpc", "id", "method", "params", "result", "error"})
_guard_installed = False


class ProtocolError(ValueError):
    """A line that is not a JSON-RPC 2.0 message of the evaluator protocol."""


class FrameWriter:
    """Writes one JSON object per line on a private file descriptor, unbuffered."""

    def __init__(self, fd: int) -> None:
        self._fd = fd

    @property
    def fd(self) -> int:
        """The private descriptor the frames go to."""
        return self._fd

    def write(self, frame: Mapping[str, Any]) -> None:
        """Send one frame; it reaches the pipe before this returns."""
        text = json.dumps(frame, separators=(",", ":"), allow_nan=False)
        view = memoryview((text + "\n").encode("ascii"))
        while view:
            view = view[os.write(self._fd, view) :]


def install_fd_guard() -> FrameWriter:
    """Move the protocol to a private descriptor and point descriptor 1 at stderr.

    After this, `print`, `os.write(1, ...)`, C extensions and inherited children all
    land on stderr; only the returned writer reaches the real stdout. It runs once.
    """
    global _guard_installed
    if _guard_installed:
        raise RuntimeError("the fd guard is already installed")
    sys.stdout.flush()
    frame_fd = os.dup(1)
    os.dup2(2, 1)
    _guard_installed = True
    return FrameWriter(frame_fd)


def parse_message(line: bytes | str) -> dict[str, Any]:
    """Check one line against the envelope rules of JSON-RPC 2.0 as the protocol uses it.

    The members are those of the shared wire vectors: `jsonrpc` exactly "2.0", a string
    `id` when present (never null, never a number), a request or notification carries
    `method` and no `result` or `error`, and a response carries `id` and one of them.
    """
    try:
        msg = json.loads(line)
    except (ValueError, RecursionError) as err:
        raise ProtocolError(f"not JSON: {err}") from err
    if not isinstance(msg, dict):
        raise ProtocolError("a message is a JSON object")
    unknown = sorted(set(msg) - _ENVELOPE_KEYS)
    if unknown:
        raise ProtocolError(f"unknown member {unknown[0]!r}")
    if msg.get("jsonrpc") != "2.0":
        raise ProtocolError('`jsonrpc` is exactly "2.0"')
    if "id" in msg and not isinstance(msg["id"], str):
        raise ProtocolError("`id` is a string")
    if "params" in msg and not isinstance(msg["params"], dict):
        raise ProtocolError("`params` is an object")
    if "method" in msg:
        if not isinstance(msg["method"], str):
            raise ProtocolError("`method` is a string")
        if "result" in msg or "error" in msg:
            raise ProtocolError("`result` and `error` belong to a response")
    else:
        if "id" not in msg or ("result" in msg) == ("error" in msg):
            raise ProtocolError(
                "a response has an `id` and one of `result` and `error`"
            )
    return msg


class LineReader:
    """Splits a descriptor into lines without Python-level buffering.

    A subprocess or a forked child can never find unread requests in a buffer, and the
    parent can wait on the descriptor with `select` while an item runs.
    """

    def __init__(self, fd: int) -> None:
        self._fd = fd
        self._buf = b""
        self._eof = False

    @property
    def eof(self) -> bool:
        """True once the descriptor has reported end of input."""
        return self._eof

    def fileno(self) -> int | None:
        """The descriptor to wait on, or None once it is exhausted."""
        return None if self._eof else self._fd

    def fill(self, timeout: float | None) -> None:
        """Read what is available: block when `timeout` is None, else wait that long."""
        if self._eof:
            return
        if timeout is not None and not select.select([self._fd], [], [], timeout)[0]:
            return
        data = os.read(self._fd, 65536)
        if data:
            self._buf += data
        else:
            self._eof = True

    def pop_line(self) -> bytes | None:
        """The next complete line without its newline, or at EOF the unterminated rest."""
        head, sep, rest = self._buf.partition(b"\n")
        if sep:
            self._buf = rest
            return head
        if self._eof and self._buf:
            head, self._buf = self._buf, b""
            return head
        return None
