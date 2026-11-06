"""Development tests for the runners; deleted by the last commit."""

import os
import time

import pytest

from candler_evaluator.isolate import (
    Done,
    Faulted,
    ForkRunner,
    InlineRunner,
    make_runner,
    run_item,
)

pytestmark = pytest.mark.scaffold


def ok(params, context, prior):
    return {"status": "pass"}


def hang(params, context, prior):
    time.sleep(60)


def leave(params, context, prior):
    os._exit(3)


def silent(params, context, prior):
    os._exit(0)


def boom(params, context, prior):
    raise RuntimeError("boom")


def bad(params, context, prior):
    return {"status": "maybe"}


def test_fork_outcomes():
    r = ForkRunner()
    assert run_item(r, ok, {}, None, [], 1000) == Done(
        {"status": "pass", "evidence": []}
    )
    start = time.monotonic()
    res = run_item(r, hang, {}, None, [], 200)
    assert isinstance(res, Faulted) and "SIGALRM" in res.reason
    assert time.monotonic() - start < 5
    assert "status 3" in run_item(r, leave, {}, None, [], 1000).reason
    assert "without an outcome" in run_item(r, silent, {}, None, [], 1000).reason
    assert "RuntimeError: boom" in run_item(r, boom, {}, None, [], 1000).reason
    assert "OutcomeError" in run_item(r, bad, {}, None, [], 1000).reason


def test_inline_and_env(monkeypatch):
    assert isinstance(run_item(InlineRunner(), ok, {}, None, [], 10), Done)
    assert "boom" in run_item(InlineRunner(), boom, {}, None, [], 10).reason
    monkeypatch.setenv("CANDLER_EVALUATOR_NO_FORK", "1")
    assert make_runner().mode == "inline"
    monkeypatch.delenv("CANDLER_EVALUATOR_NO_FORK")
    assert make_runner().mode == "fork"
