"""Helper for writing Candler evaluator plug-ins in Python."""

from importlib.metadata import version

from .frames import FrameWriter
from .loop import main, serve
from .plugin import Outcome, Plugin

# The one literal of the version is `version` in pyproject.toml.
__version__ = version("candler-evaluator")

__all__ = ["FrameWriter", "Outcome", "Plugin", "__version__", "main", "serve"]
