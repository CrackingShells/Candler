#!/usr/bin/env python3
"""Rewrite the version numbers the contract pages quote from the three manifests.

Every version number of a page under docs/contract/ sits inside a block

    <!-- versions -->
    ... `v0.6.0` ... `schemas@0.6.0` ... `candler-evaluator@0.1.0` ...
    <!-- /versions -->

and this script rewrites exactly those tokens from Cargo.toml (the workspace version, tagged
`v<version>`), crates/candler-schemas/Cargo.toml (`schemas@<version>`) and
python/candler-evaluator/pyproject.toml (`candler-evaluator@<version>`). Text outside the blocks is
never touched.

    python3 scripts/contract_versions.py            write the blocks
    python3 scripts/contract_versions.py --check    write nothing, report a block that differs

Exit status: 0 blocks written or already equal; 1 --check found a block that differs; 2 a manifest
or a block marker is missing or malformed. Python 3.11 or later, standard library only; no build.
"""

import argparse
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OPEN = "<!-- versions -->"
CLOSE = "<!-- /versions -->"
TOKEN = re.compile(r"`(v|schemas@|candler-evaluator@)(\d+\.\d+\.\d+[0-9A-Za-z.+-]*)`")


class Malformed(Exception):
    """A manifest or a block marker is missing or malformed."""


def manifest_version(file: str, keys: tuple[str, ...]) -> str:
    path = ROOT / file
    try:
        value = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise Malformed(f"{file}: {error}") from error
    for key in keys:
        if not isinstance(value, dict) or key not in value:
            raise Malformed(f"{file}: no `{key}`")
        value = value[key]
    if not isinstance(value, str):
        raise Malformed(f"{file}: the version is not a string")
    return value


def read_versions() -> dict[str, str]:
    return {
        "v": manifest_version("Cargo.toml", ("workspace", "package", "version")),
        "schemas@": manifest_version("crates/candler-schemas/Cargo.toml", ("package", "version")),
        "candler-evaluator@": manifest_version(
            "python/candler-evaluator/pyproject.toml", ("project", "version")
        ),
    }


def rewrite(name: str, text: str, versions: dict[str, str]) -> str:
    """The text with every token inside a block set to its manifest's version."""
    out: list[str] = []
    rest = text
    while (start := rest.find(OPEN)) != -1:
        out.append(rest[: start + len(OPEN)])
        rest = rest[start + len(OPEN) :]
        end = rest.find(CLOSE)
        if end == -1:
            raise Malformed(f"{name}: a `{OPEN}` block is never closed")
        inside = rest[:end]
        if OPEN in inside:
            raise Malformed(f"{name}: a `{OPEN}` block opens inside another")
        out.append(TOKEN.sub(lambda m: f"`{m[1]}{versions[m[1]]}`", inside))
        rest = rest[end:]
        out.append(CLOSE)
        rest = rest[len(CLOSE) :]
    if CLOSE in rest:
        raise Malformed(f"{name}: a `{CLOSE}` marker has no opening")
    out.append(rest)
    return "".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="write nothing; exit 1 on a difference")
    args = parser.parse_args()
    try:
        versions = read_versions()
        differing = []
        for page in sorted((ROOT / "docs" / "contract").glob("*.md")):
            text = page.read_text(encoding="utf-8")
            new = rewrite(page.name, text, versions)
            if new != text:
                differing.append(page.name)
                if not args.check:
                    page.write_text(new, encoding="utf-8")
    except Malformed as error:
        print(f"contract_versions: {error}", file=sys.stderr)
        return 2
    if args.check and differing:
        print("contract_versions: a versions block differs from its manifest in: " + ", ".join(differing), file=sys.stderr)
        return 1
    for name in differing:
        print(f"updated docs/contract/{name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
