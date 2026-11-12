"""The kinds of the pool guards: `interval`, `stray` and `ok`.

`interval` marks when it ran, so that a guard can tell whether two requests overlapped: it
appends `<pid> start <monotonic>` to the file its `file` param names, sleeps `sleep_s` seconds
(an optional param, 0.5 by default), then appends `<pid> end <monotonic>`. `stray` leaves a
process behind that ignores SIGTERM and has a session of its own, appends its pid to the file its
`pidfile` param names and passes.
"""

import os
import subprocess
import time

from synthetic_plugin import ok

NAME = "pool"
VERSION = "0.1.0"

DEFAULT_SLEEP_S = 0.5


def _mark(path, what):
    with open(path, "a") as handle:
        handle.write(f"{os.getpid()} {what} {time.monotonic():.6f}\n")


def interval(params, context, prior):
    path = params["file"]
    _mark(path, "start")
    time.sleep(float(params.get("sleep_s", DEFAULT_SLEEP_S)))
    _mark(path, "end")
    return {"status": "pass"}


def stray(params, context, prior):
    child = subprocess.Popen(
        ["sh", "-c", 'trap "" TERM; sleep 300'],
        start_new_session=True,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    with open(params["pidfile"], "a") as handle:
        handle.write(f"{child.pid}\n")
    return {"status": "pass"}


KINDS = {"ok": ok, "interval": interval, "stray": stray}
