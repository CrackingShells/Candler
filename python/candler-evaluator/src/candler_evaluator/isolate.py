"""Run one item: in a forked child under its own deadline, or in-process as a fallback.

Nothing here signals a process. The child arms its own `ITIMER_REAL`, so the kernel
delivers `SIGALRM` to it at the item deadline; the parent only waits and reads a pipe.
"""

from __future__ import annotations

import json
import os
import select
import signal
import sys
import traceback
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass
from typing import Any, Protocol

from .plugin import Kind, normalise_outcome

__all__ = [
    "Abandoned",
    "Done",
    "Faulted",
    "ForkRunner",
    "InlineRunner",
    "Interrupt",
    "make_runner",
    "run_item",
]

NO_FORK_ENV = "CANDLER_EVALUATOR_NO_FORK"
_MAX_DEADLINE_S = 10_000_000.0
_POLL_S = 0.2


@dataclass(frozen=True)
class Done:
    """The kind returned a valid outcome."""

    outcome: dict[str, Any]


@dataclass(frozen=True)
class Faulted:
    """The item ended without an outcome; `reason` names the cause."""

    reason: str


@dataclass(frozen=True)
class Abandoned:
    """The request was cancelled while the item ran; its child is left to its own alarm."""


class Interrupt(Protocol):
    """What a runner needs to notice a cancel while it waits for a child."""

    cancelled: bool

    def fileno(self) -> int | None: ...

    def pump(self) -> None: ...


def _flush_std() -> None:
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.flush()
        except (OSError, ValueError):
            pass


def _call(fn: Callable[[], Any]) -> dict[str, Any] | str:
    """Run the kind and check its result; a fault comes back as the reason string."""
    try:
        return normalise_outcome(fn())
    except SystemExit as err:
        return f"kind exited via SystemExit({err.code!r})"
    except Exception as err:  # noqa: BLE001 - whatever a kind raises costs that item only
        traceback.print_exc()
        return f"kind raised {type(err).__name__}: {err}"


class InlineRunner:
    """Runs the kind in this process, under the same fd guard and with no item deadline."""

    mode = "inline"

    def reap(self) -> None:
        """There is no child to collect."""

    def run(
        self,
        fn: Callable[[], Any],
        deadline_s: float,
        interrupt: Interrupt | None = None,
    ) -> Done | Faulted | Abandoned:
        """Run `fn` and return its checked outcome or the fault."""
        result = _call(fn)
        return Faulted(result) if isinstance(result, str) else Done(result)


class ForkRunner:
    """Runs each kind in a forked child that arms its own deadline."""

    mode = "fork"

    def __init__(self, child_closes: Sequence[int] = ()) -> None:
        self._child_closes = tuple(child_closes)
        self._abandoned: list[int] = []

    def reap(self) -> None:
        """Collect abandoned children that have ended; those still running stay listed.

        This only calls `waitpid` with `WNOHANG`; nothing is signalled.
        """
        still_running = []
        for pid in self._abandoned:
            try:
                done, _ = os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                continue
            if not done:
                still_running.append(pid)
        self._abandoned = still_running

    def run(
        self,
        fn: Callable[[], Any],
        deadline_s: float,
        interrupt: Interrupt | None = None,
    ) -> Done | Faulted | Abandoned:
        """Fork, run `fn` in the child, and read its outcome from a pipe."""
        self.reap()
        _flush_std()
        read_fd, write_fd = os.pipe()
        pid = os.fork()
        if pid == 0:
            code = 1
            try:
                os.close(read_fd)
                code = self._child(fn, deadline_s, write_fd)
            finally:
                os._exit(code)
        os.close(write_fd)
        try:
            result = self._await(pid, read_fd, deadline_s, interrupt)
        finally:
            os.close(read_fd)
        if isinstance(result, Abandoned):
            self._abandoned.append(pid)
        return result

    def _child(self, fn: Callable[[], Any], deadline_s: float, write_fd: int) -> int:
        try:
            for fd in self._child_closes:
                os.close(fd)
            signal.pthread_sigmask(signal.SIG_SETMASK, set())
            signal.signal(signal.SIGALRM, signal.SIG_DFL)
            null = os.open(os.devnull, os.O_RDONLY)
            os.dup2(null, 0)
            os.close(null)
            sys.stdin = open(os.devnull)  # noqa: SIM115 - lives as long as the child
            signal.setitimer(
                signal.ITIMER_REAL, min(max(deadline_s, 0.001), _MAX_DEADLINE_S)
            )
            result = _call(fn)
            payload = (
                {"fault": result} if isinstance(result, str) else {"outcome": result}
            )
            data = json.dumps(payload, allow_nan=False).encode("ascii")
            _flush_std()
            view = memoryview(data)
            while view:
                view = view[os.write(write_fd, view) :]
            return 0
        except BaseException:  # noqa: BLE001 - the child must leave through os._exit
            return 1

    def _await(
        self, pid: int, read_fd: int, deadline_s: float, interrupt: Interrupt | None
    ) -> Done | Faulted | Abandoned:
        os.set_blocking(read_fd, False)
        chunks: list[bytes] = []
        status: int | None = None
        while True:
            watch = interrupt.fileno() if interrupt is not None else None
            ready = select.select(
                [read_fd] + ([watch] if watch is not None else []), [], [], _POLL_S
            )[0]
            if watch is not None and watch in ready and interrupt is not None:
                interrupt.pump()
                if interrupt.cancelled:
                    return Abandoned()
            if read_fd in ready:
                try:
                    data = os.read(read_fd, 65536)
                except BlockingIOError:
                    continue
                if not data:
                    break
                chunks.append(data)
            elif not ready:
                # A descendant of the child may hold the pipe open after the child is gone.
                done, status = os.waitpid(pid, os.WNOHANG)
                if done:
                    break
                status = None
        if status is None:
            _, status = os.waitpid(pid, 0)
        return _decide(status, b"".join(chunks), deadline_s)


def _decide(status: int, data: bytes, deadline_s: float) -> Done | Faulted:
    if os.WIFSIGNALED(status):
        sig = signal.Signals(os.WTERMSIG(status))
        if sig is signal.SIGALRM:
            return Faulted(
                f"item deadline of {deadline_s * 1000:g} ms exceeded (SIGALRM)"
            )
        return Faulted(f"item process ended by signal {sig.name}")
    code = os.WEXITSTATUS(status)
    if code != 0:
        return Faulted(f"item process exited with status {code}")
    if not data:
        return Faulted("item process exited without an outcome")
    try:
        payload = json.loads(data)
    except ValueError:
        return Faulted("item process left a partial outcome")
    if not isinstance(payload, dict):
        return Faulted("item process left an unreadable outcome")
    if "fault" in payload:
        return Faulted(str(payload["fault"]))
    if isinstance(payload.get("outcome"), dict):
        return Done(payload["outcome"])
    return Faulted("item process left an unreadable outcome")


def make_runner(child_closes: Sequence[int] = ()) -> ForkRunner | InlineRunner:
    """The fork runner, or the inline one where `fork` is missing or the env var is `1`."""
    if not hasattr(os, "fork") or os.environ.get(NO_FORK_ENV) == "1":
        return InlineRunner()
    return ForkRunner(child_closes)


def run_item(
    runner: ForkRunner | InlineRunner,
    kind: Kind,
    params: Mapping[str, Any],
    context: Any,
    prior: Sequence[Any],
    deadline_ms: int,
    interrupt: Interrupt | None = None,
) -> Done | Faulted | Abandoned:
    """Run `kind(params, context, prior)` under `runner` with the item's deadline."""
    return runner.run(
        lambda: kind(params, context, prior), deadline_ms / 1000, interrupt
    )
