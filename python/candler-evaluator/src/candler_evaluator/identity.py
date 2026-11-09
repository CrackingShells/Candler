"""The identity of an installed distribution, read from its PEP 610 `direct_url.json`.

The helper never runs `git` and never looks at a source tree. What an install records is
all it knows, and an install that records nothing is refused.
"""

from __future__ import annotations

import hashlib
import importlib.metadata
import json
import re
from dataclasses import dataclass
from typing import Any

__all__ = ["Identity", "IdentityError", "identity"]

_COMMIT = re.compile(r"(?:[0-9a-f]{40}|[0-9a-f]{64})")
_SEMVER = re.compile(
    r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
    r"(?:-(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*)?"
    r"(?:\+[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*)?"
)


class IdentityError(Exception):
    """The install does not say what it is, so the helper will not say it either."""


@dataclass(frozen=True)
class Identity:
    """The name, version and instrument digest of one installed distribution.

    `digest` is `git:<commit>` for a clean git install. An editable install has no
    commit to name, so its digest is the `sha256:` of its `direct_url.json` and `dirty`
    is true.
    """

    name: str
    version: str
    digest: str
    dirty: bool

    def as_instrument(self) -> dict[str, Any]:
        """The instrument object of `describe` and of an outcome record."""
        return {
            "name": self.name,
            "version": self.version,
            "digest": self.digest,
            "dirty": self.dirty,
        }


def identity(dist_name: str) -> Identity:
    """Read `dist_name`'s identity from its installed metadata, or raise `IdentityError`."""
    try:
        dist = importlib.metadata.distribution(dist_name)
    except importlib.metadata.PackageNotFoundError as err:
        raise IdentityError(f"{dist_name}: no such distribution is installed") from err
    name = dist.metadata["Name"] or dist_name
    if not _SEMVER.fullmatch(dist.version):
        raise IdentityError(f"{name}: version {dist.version!r} is not SemVer 2.0")
    text = dist.read_text("direct_url.json")
    if text is None:
        raise IdentityError(
            f"{name}: no direct_url.json, so the install names no commit"
            " (an index install is refused)"
        )
    try:
        data = json.loads(text)
    except ValueError as err:
        raise IdentityError(f"{name}: direct_url.json is not JSON") from err
    if not isinstance(data, dict):
        raise IdentityError(f"{name}: direct_url.json is not an object")
    vcs, local = data.get("vcs_info"), data.get("dir_info")
    if isinstance(vcs, dict) and local is None:
        return _from_git(name, dist.version, vcs)
    if isinstance(local, dict) and vcs is None and local.get("editable") is True:
        digest = "sha256:" + hashlib.sha256(text.encode("utf-8")).hexdigest()
        return Identity(name, dist.version, digest, dirty=True)
    raise IdentityError(
        f"{name}: direct_url.json is neither a git install nor an editable one"
    )


def _from_git(name: str, version: str, vcs: dict[str, Any]) -> Identity:
    if vcs.get("vcs") != "git":
        raise IdentityError(f"{name}: vcs {vcs.get('vcs')!r} is not git")
    commit = vcs.get("commit_id")
    if not isinstance(commit, str) or not _COMMIT.fullmatch(commit):
        raise IdentityError(f"{name}: commit_id is not a full lowercase hex hash")
    return Identity(name, version, f"git:{commit}", dirty=False)
