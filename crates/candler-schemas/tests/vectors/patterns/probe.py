# /// script
# requires-python = ">=3.11"
# ///
"""Replays cases.toml under Python `re`, the engine Python `jsonschema` uses.

A JSON Schema `pattern` matches when the regular expression is found anywhere in the string
(`re.search`). Run it with `uv run probe.py`; it exits 1 on any disagreement.
"""
import re
import sys
import tomllib
from pathlib import Path

cases = tomllib.loads((Path(__file__).parent / "cases.toml").read_text(encoding="utf-8"))
bad = 0
for name, table in cases.items():
    pattern = re.compile(table["pattern"])
    for expected, key in ((True, "accept"), (False, "refuse")):
        for text in table[key]:
            if bool(pattern.search(text)) != expected:
                bad += 1
                print(f"MISMATCH {name} {key} {text!r}")
    print(f"python re {sys.version.split()[0]}: {name}: {len(table['accept'])} accept, {len(table['refuse'])} refuse")
sys.exit(1 if bad else 0)
