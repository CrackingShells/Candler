"""A synthetic plug-in: one kind per way an item can misbehave."""

import os
import subprocess
import sys
import time

NAME = "synthetic"
VERSION = "0.1.0"


def _passed(note):
    return {"status": "pass", "evidence": [{"note": note}]}


def ok(params, context, prior):
    return _passed("ok")


def hang(params, context, prior):
    time.sleep(3600)


def leave(params, context, prior):
    os._exit(3)


def say(params, context, prior):
    print("stray print, not a frame")
    return _passed("print")


def write1(params, context, prior):
    os.write(1, b"stray os.write, not a frame\n")
    return _passed("write1")


def subprocess_echo(params, context, prior):
    code = "print('stray child stdout, not a frame')"
    subprocess.run([sys.executable, "-c", code], check=True)
    return _passed("subprocess_echo")


KINDS = {
    "ok": ok,
    "hang": hang,
    "exit": leave,
    "print": say,
    "write1": write1,
    "subprocess_echo": subprocess_echo,
}
