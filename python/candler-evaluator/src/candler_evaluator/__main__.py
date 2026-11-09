"""`python -m candler_evaluator <plugin-module>`."""

import os
import sys

from .loop import main

code = main()
# Every frame was written with `os.write` before `main` returned, and stdin has ended. What is
# left is Python's own teardown, about 20 ms per process: flush the text streams and leave.
for stream in (sys.stdout, sys.stderr):
    try:
        stream.flush()
    except OSError:
        pass
os._exit(code)
