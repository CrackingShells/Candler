"""Development tests for identity; deleted by the last commit."""

import json

import pytest

from candler_evaluator.identity import IdentityError, identity

pytestmark = pytest.mark.scaffold

SHA = "7aee" * 10


def dist(tmp_path, monkeypatch, name, direct_url):
    info = tmp_path / f"{name.replace('-', '_')}-1.2.3.dist-info"
    info.mkdir()
    (info / "METADATA").write_text(
        f"Metadata-Version: 2.1\nName: {name}\nVersion: 1.2.3\n"
    )
    if direct_url is not None:
        (info / "direct_url.json").write_text(json.dumps(direct_url))
    monkeypatch.syspath_prepend(str(tmp_path))


def test_git(tmp_path, monkeypatch):
    dist(
        tmp_path,
        monkeypatch,
        "sc-git",
        {"url": "file:///r", "vcs_info": {"vcs": "git", "commit_id": SHA}},
    )
    got = identity("sc-git")
    assert (got.digest, got.dirty, got.version) == (f"git:{SHA}", False, "1.2.3")


def test_editable_and_refusals(tmp_path, monkeypatch):
    dist(
        tmp_path,
        monkeypatch,
        "sc-ed",
        {"url": "file:///p", "dir_info": {"editable": True}},
    )
    assert identity("sc-ed").dirty is True
    assert identity("sc-ed").digest.startswith("sha256:")
    dist(tmp_path, monkeypatch, "sc-none", None)
    dist(tmp_path, monkeypatch, "sc-dir", {"url": "file:///p", "dir_info": {}})
    dist(
        tmp_path,
        monkeypatch,
        "sc-hg",
        {"url": "x", "vcs_info": {"vcs": "hg", "commit_id": SHA}},
    )
    dist(
        tmp_path,
        monkeypatch,
        "sc-short",
        {"url": "x", "vcs_info": {"vcs": "git", "commit_id": "abc"}},
    )
    for name in ("sc-none", "sc-dir", "sc-hg", "sc-short", "sc-absent"):
        with pytest.raises(IdentityError):
            identity(name)
