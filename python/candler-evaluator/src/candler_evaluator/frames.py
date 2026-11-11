"""The protocol pipe: frames out, lines in, and the guard that keeps stdout clean.

Every message that crosses the pipe, in or out, passes the models generated from the
published schemas (`_models`), so the helper accepts and emits exactly what they decide.
"""

from __future__ import annotations

import json
import os
import select
import sys
from collections.abc import Mapping
from typing import Any

from pydantic import RootModel, ValidationError

from ._models.evaluator_message import Message, RequestId

__all__ = [
    "INVALID_PARAMS",
    "METHOD_NOT_FOUND",
    "FrameWriter",
    "LineReader",
    "ProtocolError",
    "Refused",
    "check_frame",
    "install_fd_guard",
    "parse_message",
    "summarise",
]

INVALID_PARAMS = -32602
METHOD_NOT_FOUND = -32601
_REQUEST_METHODS = frozenset({"describe", "evaluate"})
_guard_installed = False


class ProtocolError(ValueError):
    """A line that is not a message of the evaluator protocol and cannot be answered.

    `refusal_code` is the code of the refusal, a row of `[helper]` in `docs/contract/codes.toml`.
    """

    def __init__(self, message: str, *, refusal_code: str) -> None:
        super().__init__(message)
        self.refusal_code = refusal_code


class Refused(ProtocolError):
    """A request with an id that the models refuse; the id's owner is owed an error.

    `code` is the JSON-RPC error code; `refusal_code` is the registry's.
    """

    def __init__(
        self, request_id: str, code: int, message: str, *, refusal_code: str
    ) -> None:
        super().__init__(message, refusal_code=refusal_code)
        self.request_id = request_id
        self.code = code


def summarise(err: ValidationError, limit: int = 3) -> str:
    """The faults of a validation error in a few words each, branch mismatches left out.

    A union reports every branch it tried; a `literal_error` is a branch whose
    discriminating member (`method`, `jsonrpc`) did not match, which says nothing of the
    fault itself.
    """
    found = err.errors(include_url=False, include_input=False)
    real = [e for e in found if e["type"] != "literal_error"] or found
    return "; ".join(
        f"{'.'.join(str(part) for part in e['loc'][-4:])}: {e['msg']}"
        for e in real[:limit]
    )


def check_frame(frame: Mapping[str, Any]) -> dict[str, Any]:
    """Validate a frame against the message model and return it as the wire dict.

    Optional members that are unset are left out: the wire has no null.
    """
    return Message.model_validate(frame).model_dump(mode="json", exclude_none=True)


class FrameWriter:
    """Writes one JSON object per line on a private file descriptor, unbuffered."""

    def __init__(self, fd: int) -> None:
        self._fd = fd

    @property
    def fd(self) -> int:
        """The private descriptor the frames go to."""
        return self._fd

    def write(self, frame: Mapping[str, Any]) -> None:
        """Send one frame, which the message model must accept; it reaches the pipe first."""
        text = json.dumps(check_frame(frame), separators=(",", ":"), allow_nan=False)
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


def _unwrap(model: Any) -> Any:
    """The object inside the root models that the generator wraps each union in."""
    while isinstance(model, RootModel):
        model = model.root
    return model


def _is_request_id(value: Any) -> bool:
    try:
        RequestId.model_validate(value, strict=True)
    except ValidationError:
        return False
    return True


def parse_message(line: bytes | str) -> Any:
    """Validate one line as a message and return its model.

    The generated `Message` decides, in strict mode. A line it refuses is a
    `ProtocolError`, except a request whose id could still be answered (`jsonrpc` "2.0", an
    `id` the `RequestId` model accepts, a string `method`, no `result` or `error`): that is a `Refused`, carrying the
    error code the request is owed.
    """
    try:
        return _unwrap(Message.model_validate_json(line, strict=True))
    except ValidationError as err:
        reason = summarise(err)
    try:
        raw = json.loads(line)
    except (ValueError, RecursionError):
        raise ProtocolError(
            f"not JSON: {reason}", refusal_code="helper.line_not_json"
        ) from None
    if (
        isinstance(raw, dict)
        and raw.get("jsonrpc") == "2.0"
        and _is_request_id(raw.get("id"))
        and isinstance(raw.get("method"), str)
        and "result" not in raw
        and "error" not in raw
    ):
        if raw["method"] in _REQUEST_METHODS:
            raise Refused(
                raw["id"],
                INVALID_PARAMS,
                reason,
                refusal_code="helper.request_invalid",
            )
        raise Refused(
            raw["id"],
            METHOD_NOT_FOUND,
            f"no method {raw['method']}",
            refusal_code="helper.method_unknown",
        )
    raise ProtocolError(reason, refusal_code="helper.message_invalid")


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
