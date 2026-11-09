"""The plug-in API: what a plug-in module exposes and what a kind receives and returns."""

from __future__ import annotations

import json
import re
from collections.abc import Callable, Mapping, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from types import ModuleType
from typing import Any

__all__ = [
    "Kind",
    "Outcome",
    "OutcomeError",
    "Plugin",
    "Prior",
    "RunContext",
    "normalise_outcome",
]

Kind = Callable[[Mapping[str, Any], Any, Sequence[Any]], "Outcome | Mapping[str, Any]"]

_STATUSES = frozenset({"pass", "fail", "blocked", "not_measured", "not_applicable"})
_NEEDS_CAUSE = frozenset({"blocked", "not_measured", "not_applicable"})
_CAUSE_KEYS = frozenset({"expectation", "artifact", "reason"})
_CORE_REASONS = frozenset({"deadline", "not_applicable_to_arm"})
_REASON = re.compile(r"[a-z][a-z0-9_]*")
_VALUE_KINDS = frozenset({"number", "count", "fraction", "text", "counts"})
_OUTCOME_KEYS = frozenset({"status", "value", "weight", "cause", "evidence"})


class OutcomeError(ValueError):
    """A kind returned something that is not an outcome the protocol can carry."""


@dataclass(frozen=True)
class Outcome:
    """The fields of one outcome; a kind may return a plain mapping with the same keys."""

    status: str
    value: Mapping[str, Any] | None = None
    weight: Mapping[str, int] | None = None
    cause: Mapping[str, str] | None = None
    evidence: Sequence[Mapping[str, str]] = ()

    def to_mapping(self) -> dict[str, Any]:
        """The mapping form: unset optional fields are left out."""
        out: dict[str, Any] = {"status": self.status}
        if self.value is not None:
            out["value"] = self.value
        if self.weight is not None:
            out["weight"] = self.weight
        if self.cause is not None:
            out["cause"] = self.cause
        if self.evidence:
            out["evidence"] = list(self.evidence)
        return out


@dataclass(frozen=True)
class Prior:
    """One outcome record already made for the run: the expectation and its record."""

    expectation: str
    outcome: Mapping[str, Any]


@dataclass(frozen=True)
class RunContext:
    """What a kind may read of one run; kinds never import this class, they only read it."""

    root: Path
    artifacts: tuple[str, ...]
    metadata: Mapping[str, Any]
    scratch: Path
    run: Mapping[str, Any] = field(default_factory=dict)
    raw: tuple[Mapping[str, Any], ...] = ()
    trace: Path | None = None
    artifact_files: tuple[Mapping[str, Any], ...] = ()
    snapshots: tuple[Mapping[str, Any], ...] = ()
    inputs: tuple[Mapping[str, Any], ...] = ()

    @classmethod
    def from_wire(cls, context: Mapping[str, Any], scratch: str) -> RunContext:
        """Build the duck-typed context from the `context` and `scratch` of an evaluate request."""
        files = tuple(context["artifacts"])
        trace = context.get("trace")
        return cls(
            root=Path(context["root"]),
            artifacts=tuple(sorted(f["path"] for f in files)),
            metadata=context["metadata"],
            scratch=Path(scratch),
            run=context["run"],
            raw=tuple(context["raw"]),
            trace=None if trace is None else Path(trace),
            artifact_files=files,
            snapshots=tuple(context["snapshots"]),
            inputs=tuple(context["inputs"]),
        )


@dataclass(frozen=True)
class Plugin:
    """A plug-in as `serve` needs it.

    `name` and `version` default to the metadata of `dist`, the plug-in's distribution.
    `instruments` are the distributions whose installed identity `describe` reports.
    `specs` maps a kind to any of `params_schema`, `needs` and `default_deadline_ms`.
    """

    kinds: Mapping[str, Kind]
    name: str | None = None
    version: str | None = None
    dist: str | None = None
    instruments: Sequence[str] = ()
    specs: Mapping[str, Mapping[str, Any]] = field(default_factory=dict)
    deterministic: bool = True

    @classmethod
    def from_module(cls, module: ModuleType) -> Plugin:
        """Read a plug-in module: `KINDS`, plus the optional `NAME`, `VERSION`, `DIST`,
        `INSTRUMENTS`, `KIND_SPECS` and `DETERMINISTIC`."""
        try:
            kinds = module.KINDS
        except AttributeError as err:
            raise TypeError(f"{module.__name__} defines no KINDS") from err
        return cls(
            kinds=kinds,
            name=getattr(module, "NAME", None),
            version=getattr(module, "VERSION", None),
            dist=getattr(module, "DIST", None),
            instruments=tuple(getattr(module, "INSTRUMENTS", ())),
            specs=getattr(module, "KIND_SPECS", {}),
            deterministic=getattr(module, "DETERMINISTIC", True),
        )


def _is_int(x: Any) -> bool:
    return isinstance(x, int) and not isinstance(x, bool)


def normalise_outcome(raw: Outcome | Mapping[str, Any]) -> dict[str, Any]:
    """Check a kind's result against the outcome rules and return it as a plain dict.

    The rules are the schema's: a pass or fail carries no cause, a blocked, not_measured
    or not_applicable carries exactly one, not_measured and not_applicable carry no
    weight, and a weight has `num` at most `den`. The result is JSON-serialisable.
    """
    if isinstance(raw, Outcome):
        raw = raw.to_mapping()
    if not isinstance(raw, Mapping):
        raise OutcomeError(
            f"a kind returns an Outcome or a mapping, not {type(raw).__name__}"
        )
    extra = sorted(set(raw) - _OUTCOME_KEYS)
    if extra:
        raise OutcomeError(f"unknown outcome key {extra[0]!r}")
    status = raw.get("status")
    if status not in _STATUSES:
        raise OutcomeError(f"status {status!r} is not one of the five statuses")
    out: dict[str, Any] = {"status": status}
    weight = raw.get("weight")
    if weight is not None:
        _check_weight(status, weight)
        out["weight"] = dict(weight)
    cause = raw.get("cause")
    if status in _NEEDS_CAUSE:
        out["cause"] = _check_cause(cause)
    elif cause is not None:
        raise OutcomeError(f"a {status} carries no cause")
    value = raw.get("value")
    if value is not None:
        if not isinstance(value, Mapping) or value.get("kind") not in _VALUE_KINDS:
            raise OutcomeError(
                "value is an object whose `kind` is one of the five value kinds"
            )
        out["value"] = dict(value)
    out["evidence"] = _check_evidence(raw.get("evidence", ()))
    try:
        json.dumps(out, allow_nan=False)
    except (TypeError, ValueError) as err:
        raise OutcomeError(f"outcome is not JSON: {err}") from err
    return out


def _check_weight(status: str, weight: Any) -> None:
    if status in ("not_measured", "not_applicable"):
        raise OutcomeError(f"a {status} carries no weight")
    if not isinstance(weight, Mapping) or set(weight) != {"num", "den"}:
        raise OutcomeError("weight is an object with `num` and `den`")
    num, den = weight["num"], weight["den"]
    if not (_is_int(num) and _is_int(den)) or den < 1 or not 0 <= num <= den:
        raise OutcomeError("weight has integers with 0 <= num <= den and den >= 1")
    if status == "pass" and num != den:
        raise OutcomeError("a pass has num equal to den")
    if status == "fail" and num >= den:
        raise OutcomeError("a fail has num below den")
    if status == "blocked" and num != 0:
        raise OutcomeError("a blocked has num 0")


def _check_cause(cause: Any) -> dict[str, str]:
    if not isinstance(cause, Mapping) or len(cause) != 1:
        raise OutcomeError("cause is an object with exactly one key")
    ((key, val),) = cause.items()
    if key not in _CAUSE_KEYS or not isinstance(val, str) or not val:
        raise OutcomeError(
            "cause key is one of expectation, artifact, reason, with a string"
        )
    if key == "reason" and (not _REASON.fullmatch(val) or val in _CORE_REASONS):
        raise OutcomeError(f"reason {val!r} is not a code an evaluator may emit")
    return {key: val}


def _check_evidence(evidence: Any) -> list[dict[str, str]]:
    if isinstance(evidence, (str, bytes)) or not isinstance(evidence, Sequence):
        raise OutcomeError("evidence is a list")
    out = []
    for item in evidence:
        if not isinstance(item, Mapping) or not item or set(item) - {"ref", "note"}:
            raise OutcomeError("evidence is an object with `ref` and `note`")
        if not all(isinstance(v, str) and v for v in item.values()):
            raise OutcomeError("evidence members are non-empty strings")
        out.append(dict(item))
    return out
