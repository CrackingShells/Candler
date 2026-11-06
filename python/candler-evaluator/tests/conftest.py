"""Shared fixtures: run the helper as the core does, in its own process."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
VECTORS = ROOT.parents[1] / "crates/candler-schemas/tests/fixtures/rpc"


@pytest.fixture
def vectors() -> Path:
    """The shared wire vectors, hand-written and read by Rust and Python alike."""
    return VECTORS


@pytest.fixture
def run_helper():
    """Run `python -m candler_evaluator <module>` on JSON lines; returns the process."""

    def run(
        plugin: str, lines: list[dict], timeout: float = 60
    ) -> subprocess.CompletedProcess:
        env = {
            **os.environ,
            "PYTHONPATH": str(ROOT / "examples" / plugin),
            "PYTHONDONTWRITEBYTECODE": "1",
        }
        stdin = "".join(json.dumps(line) + "\n" for line in lines)
        return subprocess.run(
            [sys.executable, "-m", "candler_evaluator", plugin],
            check=False,
            input=stdin,
            capture_output=True,
            text=True,
            env=env,
            timeout=timeout,
        )

    return run
