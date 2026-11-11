#!/usr/bin/env python3
"""Writes, or checks, the reference values for Candler's default statistics.

Run it with the engines pinned, from any directory (it reads no Candler file):

    uv run --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 \
        python gen_stats.py            # writes the six TOML files beside this script
    uv run --no-project --with scipy==1.17.0 --with statsmodels==0.15.0 \
        python gen_stats.py --check    # regenerates in memory and compares

Every value comes from scipy or statsmodels. Floats are written with Python's repr, which
round-trips a double (up to 17 significant digits). `--check` compares each float with relative
tolerance 1e-12, so a patch release of an engine does not break it, and exits 1 naming the first
differing row.
"""

import argparse
from fractions import Fraction
import inspect
import math
import platform
import random
import re
import sys
import tomllib
from pathlib import Path

import scipy
import scipy.stats
import statsmodels
from scipy.stats import binom, binomtest, fisher_exact, hypergeom
from statsmodels.stats.multitest import multipletests
from statsmodels.stats.proportion import confint_proportions_2indep, proportion_confint

SEED = 20261111
ALPHA = 0.05
REL_TOL = 1e-12
ABS_TOL = 1e-15
HERE = Path(__file__).resolve().parent


def scipy_tie_tolerance() -> float:
    """The relative tolerance on equal probabilities in scipy's two-sided Fisher branch."""
    source = inspect.getsource(scipy.stats.fisher_exact)
    found = re.findall(r"^\s*epsilon\s*=\s*([0-9.eE+-]+)\s*$", source, flags=re.M)
    if len(found) != 1:
        sys.exit("gen_stats: cannot read the tie tolerance from scipy's fisher_exact source")
    return float(found[0])


def engine(with_tie: bool = False) -> dict:
    table = {
        "scipy": scipy.__version__,
        "statsmodels": statsmodels.__version__,
        "python": platform.python_version(),
    }
    if with_tie:
        table["tie_tolerance"] = scipy_tie_tolerance()
    return table


def wilson_limits(k: int, n: int) -> tuple[float, float]:
    lo, hi = proportion_confint(k, n, alpha=ALPHA, method="wilson")
    return float(lo), float(hi)


# ---- wilson -------------------------------------------------------------------------------


def make_wilson() -> dict:
    rows = []
    for n in (1, 2, 3, 5, 10, 20, 30, 100):
        for k in range(n + 1):
            lo, hi = wilson_limits(k, n)
            ci = binomtest(k, n).proportion_ci(confidence_level=1 - ALPHA, method="wilson")
            assert math.isclose(lo, ci.low, rel_tol=1e-12, abs_tol=1e-12), (k, n, lo, ci.low)
            assert math.isclose(hi, ci.high, rel_tol=1e-12, abs_tol=1e-12), (k, n, hi, ci.high)
            rows.append({"k": k, "n": n, "lo": lo, "hi": hi})
    return {"engine": engine(), "rows": rows}


# ---- fisher -------------------------------------------------------------------------------


def strict_two_sided(k1: int, n1: int, k2: int, n2: int) -> float:
    """Sum of the tables no more likely than the observed one, with no tolerance on ties."""
    big_n, big_k = n1 + n2, k1 + k2
    lo, hi = max(0, n1 + big_k - big_n), min(n1, big_k)
    pmf = {x: float(hypergeom.pmf(x, big_n, big_k, n1)) for x in range(lo, hi + 1)}
    observed = pmf[k1]
    return min(1.0, sum(p for p in pmf.values() if p <= observed))


def fisher_p(k1: int, n1: int, k2: int, n2: int) -> float:
    return float(fisher_exact([[k1, n1 - k1], [k2, n2 - k2]], alternative="two-sided").pvalue)


NEAR_LOWER = Fraction(1, 10**14)
NEAR_UPPER = Fraction(1, 10**7)


def near_ties(cells) -> int:
    """Pairs of tables, over the margins the grid compares, whose exact probabilities differ
    relatively by more than 1e-14 and less than 1e-7: the pairs on which a tie tolerance between
    those two bounds would decide differently from scipy's. Exact rationals, no float."""
    margins = {(n1, n2, k1 + k2) for k1, n1, k2, n2 in cells}
    count = 0
    for n1, n2, big_k in sorted(margins):
        big_n = n1 + n2
        lo, hi = max(0, n1 + big_k - big_n), min(n1, big_k)
        total = math.comb(big_n, n1)
        pmf = [Fraction(math.comb(big_k, x) * math.comb(big_n - big_k, n1 - x), total)
               for x in range(lo, hi + 1)]
        for i, a in enumerate(pmf):
            for b in pmf[i + 1:]:
                relative = abs(a - b) / max(a, b)
                if NEAR_LOWER < relative < NEAR_UPPER:
                    count += 1
    return count


def make_fisher() -> dict:
    cells = []
    for n1 in (1, 3, 5, 10):
        for n2 in (1, 3, 5, 10):
            for k1 in range(n1 + 1):
                for k2 in range(n2 + 1):
                    cells.append((k1, n1, k2, n2))
    rng = random.Random(SEED)
    for _ in range(40):
        n1, n2 = rng.choice((20, 30)), rng.choice((20, 30))
        cells.append((rng.randint(0, n1), n1, rng.randint(0, n2), n2))
    # Equal arms make mirror tables equally likely; at these sizes a float implementation that
    # computes the two probabilities separately can differ in the last bits, which is what the
    # tie tolerance is for.
    for n, k1, k2 in (
        (26, 0, 1),
        (26, 1, 0),
        (26, 1, 2),
        (26, 2, 1),
        (26, 2, 3),
        (26, 3, 2),
        (26, 5, 8),
        (26, 8, 5),
        (40, 0, 1),
        (40, 1, 0),
        (40, 1, 2),
        (40, 2, 1),
        (40, 2, 3),
        (40, 3, 2),
        (40, 5, 8),
        (40, 8, 5),
        (60, 0, 1),
        (60, 1, 0),
        (60, 1, 2),
        (60, 2, 1),
        (60, 2, 3),
        (60, 3, 2),
        (60, 5, 8),
        (60, 8, 5),
    ):
        cells.append((k1, n, k2, n))
    rows = []
    for k1, n1, k2, n2 in cells:
        p = fisher_p(k1, n1, k2, n2)
        tie = abs(strict_two_sided(k1, n1, k2, n2) - p) > 1e-9
        assert abs(p - ALPHA) > 1e-9, ("p too close to alpha", k1, n1, k2, n2, p)
        rows.append({"k1": k1, "n1": n1, "k2": k2, "n2": n2, "p": p, "tie": tie})
    ties = sum(1 for row in rows if row["tie"])
    if ties < 5:
        sys.exit(f"gen_stats: only {ties} tie rows; at least 5 are required")
    ties = {"lower": float(NEAR_LOWER), "upper": float(NEAR_UPPER), "near_ties": near_ties(cells)}
    return {"engine": engine(with_tie=True), "ties": ties, "rows": rows}


# ---- newcombe -----------------------------------------------------------------------------


def newcombe_from_wilson(k1: int, n1: int, k2: int, n2: int) -> tuple[float, float]:
    p1, p2 = k1 / n1, k2 / n2
    l1, u1 = wilson_limits(k1, n1)
    l2, u2 = wilson_limits(k2, n2)
    d = p1 - p2
    return (
        d - math.sqrt((p1 - l1) ** 2 + (u2 - p2) ** 2),
        d + math.sqrt((u1 - p1) ** 2 + (p2 - l2) ** 2),
    )


def make_newcombe() -> dict:
    def ks(n: int) -> list[int]:
        if n <= 10:
            return list(range(n + 1))
        return sorted({0, 1, n // 2, n - 1, n})

    rows = []
    for n1 in (1, 5, 10, 30):
        for n2 in (1, 5, 10, 30):
            for k1 in ks(n1):
                for k2 in ks(n2):
                    lo, hi = confint_proportions_2indep(
                        k1, n1, k2, n2, method="newcomb", compare="diff", alpha=ALPHA
                    )
                    lo, hi = float(lo), float(hi)
                    own_lo, own_hi = newcombe_from_wilson(k1, n1, k2, n2)
                    assert abs(lo - own_lo) < 1e-9 and abs(hi - own_hi) < 1e-9, (
                        k1, n1, k2, n2, lo, hi, own_lo, own_hi,
                    )
                    rows.append(
                        {"k1": k1, "n1": n1, "k2": k2, "n2": n2,
                         "diff": k1 / n1 - k2 / n2, "lo": lo, "hi": hi}
                    )
    return {"engine": engine(), "rows": rows}


# ---- holm ---------------------------------------------------------------------------------


def holm_adjusted(p: list[float]) -> list[float]:
    return [float(x) for x in multipletests(p, alpha=ALPHA, method="holm")[1]]


def make_holm() -> dict:
    lists = [
        [0.03],
        [0.02, 0.02],
        [0.01, 0.01, 0.01],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
        [0.9, 0.5, 0.2, 0.04, 0.001],
        [0.001, 0.04, 0.2, 0.5, 0.9],
        [0.5, 0.6, 0.9],
        # adjusted values land on 0.05 exactly (4 * 0.0125) and straddle it
        [0.0125, 0.0125, 0.0125, 0.9],
        [0.01, 0.04, 0.04],
        [0.02, 0.02, 0.03, 0.2],
        [0.001] + [0.5 + 0.02 * i for i in range(19)],
    ]
    rng = random.Random(SEED)
    for _ in range(200):
        m = rng.randint(2, 20)
        lists.append(
            [round(rng.random() ** rng.choice((1, 2, 4)), rng.choice((3, 6))) for _ in range(m)]
        )
    rows = [{"p": p, "adjusted": holm_adjusted(p)} for p in lists]
    return {"engine": engine(), "rows": rows}


# ---- coverage and power -------------------------------------------------------------------


def make_coverage() -> dict:
    rows = []
    for n in (5, 10, 30):
        limits = [wilson_limits(k, n) for k in range(n + 1)]
        for p in (0.1, 0.5, 0.9):
            total = 0.0
            for k, (lo, hi) in enumerate(limits):
                assert abs(lo - p) > 1e-9 and abs(hi - p) > 1e-9, ("limit at p", n, k, p)
                if lo <= p <= hi:
                    total += float(binom.pmf(k, n, p))
            rows.append({"n": n, "p": p, "coverage": total})
    return {"engine": engine(), "rows": rows}


def make_power() -> dict:
    n, p_a, p_b = 10, 0.2, 0.8
    power = 0.0
    for k1 in range(n + 1):
        for k2 in range(n + 1):
            if fisher_p(k1, n, k2, n) < ALPHA:
                power += float(binom.pmf(k1, n, p_a) * binom.pmf(k2, n, p_b))
    return {
        "engine": engine(with_tie=True),
        "rows": [{"n": n, "p_a": p_a, "p_b": p_b, "alpha": ALPHA, "power": power}],
    }


# ---- output -------------------------------------------------------------------------------


def scalar(value) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        if not math.isfinite(value):
            sys.exit("gen_stats: a non-finite value cannot be written")
        return repr(value)
    if isinstance(value, str):
        return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'
    if isinstance(value, list):
        return "[" + ", ".join(scalar(v) for v in value) + "]"
    raise TypeError(type(value))


def render(doc: dict) -> str:
    lines = ["[engine]"]
    lines += [f"{key} = {scalar(value)}" for key, value in doc["engine"].items()]
    if "ties" in doc:
        lines.append("")
        lines.append("[ties]")
        lines += [f"{key} = {scalar(value)}" for key, value in doc["ties"].items()]
    for row in doc["rows"]:
        lines.append("")
        lines.append("[[rows]]")
        lines += [f"{key} = {scalar(value)}" for key, value in row.items()]
    return "\n".join(lines) + "\n"


FILES = {
    "wilson.toml": make_wilson,
    "fisher.toml": make_fisher,
    "newcombe.toml": make_newcombe,
    "holm.toml": make_holm,
    "coverage.toml": make_coverage,
    "power.toml": make_power,
}


def differs(a, b) -> bool:
    if isinstance(a, bool) or isinstance(b, bool):
        return a != b
    if isinstance(a, float) or isinstance(b, float):
        return not math.isclose(a, b, rel_tol=REL_TOL, abs_tol=ABS_TOL)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) != len(b) or any(differs(x, y) for x, y in zip(a, b))
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() != b.keys() or any(differs(a[k], b[k]) for k in a)
    return a != b


def check() -> int:
    for name, make in FILES.items():
        fresh = tomllib.loads(render(make()))
        path = HERE / name
        if not path.exists():
            print(f"{name}: file is missing")
            return 1
        stored = tomllib.loads(path.read_text())
        for key, value in fresh["engine"].items():
            if key == "tie_tolerance" and differs(value, stored["engine"].get(key)):
                print(f"{name}: engine tie_tolerance {stored['engine'].get(key)} != {value}")
                return 1
        if differs(fresh.get("ties"), stored.get("ties")):
            print(f"{name}: [ties] stored {stored.get('ties')}, regenerated {fresh.get('ties')}")
            return 1
        if len(fresh["rows"]) != len(stored["rows"]):
            print(f"{name}: {len(stored['rows'])} stored rows, {len(fresh['rows'])} regenerated")
            return 1
        for index, (new, old) in enumerate(zip(fresh["rows"], stored["rows"])):
            for key, value in new.items():
                if key not in old or differs(value, old[key]):
                    print(f"{name}: row {index} differs at `{key}`: stored {old.get(key)!r}, "
                          f"regenerated {value!r}; row {old}")
                    return 1
        print(f"{name}: {len(stored['rows'])} rows match")
    return 0


def write() -> int:
    for name, make in FILES.items():
        (HERE / name).write_text(render(make()))
        print(f"wrote {name}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare, do not write")
    args = parser.parse_args()
    return check() if args.check else write()


if __name__ == "__main__":
    sys.exit(main())
