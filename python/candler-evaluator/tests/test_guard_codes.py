"""Guards for the helper's refusal codes: every raise of the helper's source carries a code of
`[helper]` in `docs/contract/codes.toml`, and every code of `[helper]` has a raise site."""

import ast
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # Python 3.10
    import tomli as tomllib

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "src" / "candler_evaluator"
CODES = ROOT.parents[1] / "docs" / "contract" / "codes.toml"

# The exception classes that take a `refusal_code` keyword.
CODED = frozenset(
    {
        "OutcomeError",
        "IdentityError",
        "ProtocolError",
        "Refused",
        "PluginError",
        "_ParamsError",
    }
)

# The raises that are not an author's, as (file, exception class): each with its reason.
ALLOWED = {
    (
        "__init__.py",
        "AttributeError",
    ): "the module's `__getattr__` answers an unknown attribute",
    (
        "frames.py",
        "RuntimeError",
    ): "the fd guard is installed twice: a fault of the host program",
}


def helper_codes() -> set[str]:
    """The codes of the `[helper]` section."""
    with CODES.open("rb") as handle:
        return set(tomllib.load(handle)["helper"])


def raises():
    """Every `raise` of the helper's source: (file, line, exception name, code or None)."""
    found = []
    for path in sorted(SOURCE.glob("*.py")):
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if not isinstance(node, ast.Raise) or node.exc is None:
                continue
            call = node.exc
            if not isinstance(call, ast.Call):
                found.append((path.name, node.lineno, ast.unparse(call), None))
                continue
            name = ast.unparse(call.func)
            code = None
            for keyword in call.keywords:
                if keyword.arg == "refusal_code" and isinstance(
                    keyword.value, ast.Constant
                ):
                    code = keyword.value.value
            found.append((path.name, node.lineno, name, code))
    return found


def test_d4_every_raise_carries_a_listed_code():
    """Guards: D4 — every raise in the helper's source is of a class that takes a code and passes
    a string literal that is a key of `[helper]`, or is in the allowlist, or re-raises bare.
    Oracle: `[helper]` of `docs/contract/codes.toml`, written by hand.
    Bite: a bare `raise ValueError(...)` added to `plugin.py`; a raise with a code `[helper]`
    lacks.
    """
    listed = helper_codes()
    faults = []
    for file, line, name, code in raises():
        if (file, name) in ALLOWED:
            continue
        if name not in CODED:
            faults.append(f"{file}:{line}: `{name}` takes no code")
        elif code not in listed:
            faults.append(
                f"{file}:{line}: `{name}` passes {code!r}, not a row of [helper]"
            )
    assert not faults, "\n".join(faults)


def test_d4_every_helper_code_is_raised():
    """Guards: D4 — every row of `[helper]` has a raise site that names its code.
    Oracle: `[helper]` of `docs/contract/codes.toml`, written by hand.
    Bite: a row added to `[helper]` that no raise uses.
    """
    used = {code for _, _, _, code in raises() if code}
    unused = sorted(helper_codes() - used)
    assert not unused, f"no raise site uses: {unused}"
