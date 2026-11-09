"""Helper for writing Candler evaluator plug-ins in Python."""

from typing import Any

from .frames import FrameWriter
from .loop import main, serve
from .plugin import Outcome, Plugin

__all__ = ["FrameWriter", "Outcome", "Plugin", "__version__", "main", "serve"]


def __getattr__(name: str) -> Any:
    """Read `__version__` from the installed metadata on first use, not at import.

    The one literal of the version is `version` in pyproject.toml.
    """
    if name == "__version__":
        from importlib.metadata import version

        return version("candler-evaluator")
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
