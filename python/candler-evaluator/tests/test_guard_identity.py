import json

import pytest

from candler_evaluator.identity import IdentityError, identity

COMMIT = "7aee5f5b9b0a3a8a54b3b2d7ed2dbd3a6d43ca31"
# The direct_url.json examples of PEP 610: a git install, an editable install, and none.
CASES = [
    (
        "gh-git",
        {
            "url": "file:///home/user/pip",
            "vcs_info": {
                "vcs": "git",
                "requested_revision": "1.3.1",
                "commit_id": COMMIT,
            },
        },
        (f"git:{COMMIT}", False),
    ),
    (
        "gh-edit",
        {"url": "file:///home/user/project", "dir_info": {"editable": True}},
        (None, True),
    ),
    ("gh-index", None, None),
    ("gh-dir", {"url": "file:///home/user/project", "dir_info": {}}, None),
    ("gh-dir-off", {"url": "file:///x", "dir_info": {"editable": False}}, None),
]


@pytest.mark.parametrize(("name", "direct_url", "expected"), CASES)
def test_identity_is_what_the_install_records(
    tmp_path, monkeypatch, name, direct_url, expected
):
    """Guards: R7 — a clean git install reports git:<sha>, an editable install reports dirty, and an install that records nothing is refused. Oracle: the direct_url.json examples of PEP 610."""
    info = tmp_path / f"{name.replace('-', '_')}-1.2.3.dist-info"
    info.mkdir()
    (info / "METADATA").write_text(
        f"Metadata-Version: 2.1\nName: {name}\nVersion: 1.2.3\n"
    )
    if direct_url is not None:
        (info / "direct_url.json").write_text(json.dumps(direct_url))
    monkeypatch.syspath_prepend(str(tmp_path))

    if expected is None:
        with pytest.raises(IdentityError):
            identity(name)
        return
    got = identity(name)
    digest, dirty = expected
    assert (got.name, got.version, got.dirty) == (name, "1.2.3", dirty)
    if digest is not None:
        assert got.digest == digest
    else:
        assert got.digest.startswith("sha256:")
