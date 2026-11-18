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


# --- exact_tests ---

from functools import lru_cache
from itertools import product

import numpy as np

# Candler's two-sided rule calls two table probabilities equal when they differ by at most this
# relative amount (the tolerance of `fisher_two_sided`). The generator proves the tolerance
# decides nothing: no table in any row has a probability within it that is not exactly equal.
MIDP_TIE = 1e-7

# Zero margins. A block with no runs in one arm, or with no passes, or with only passes, has a
# one-point null (A's passes are forced), so it contributes a point mass. A table whose whole null
# is one point has mid-p = 1/2 (the observed table is the only, hence tied, table); a block that is
# a point mass inside a convolution moves the support and leaves the other blocks' pmf unchanged.


def support(a_n: int, b_n: int, t: int) -> tuple[int, int]:
    return max(0, t - b_n), min(a_n, t)


@lru_cache(maxsize=None)
def scipy_block(a_n: int, b_n: int, t: int) -> tuple[int, tuple[float, ...]]:
    """scipy's pmf of A's passes in one block, as (first value, probabilities)."""
    first, last = support(a_n, b_n, t)
    if first == last:
        return first, (1.0,)
    pmf = tuple(float(hypergeom.pmf(x, a_n + b_n, t, a_n)) for x in range(first, last + 1))
    return first, pmf


@lru_cache(maxsize=None)
def exact_block(a_n: int, b_n: int, t: int) -> tuple[int, tuple[Fraction, ...]]:
    first, last = support(a_n, b_n, t)
    total = math.comb(a_n + b_n, a_n)
    return first, tuple(
        Fraction(math.comb(t, x) * math.comb(a_n + b_n - t, a_n - x), total)
        for x in range(first, last + 1)
    )


def scipy_null(margins) -> tuple[int, list[float]]:
    """The convolution of scipy's block pmfs: (first value of the sum, probabilities)."""
    offset, pmf = 0, np.array([1.0])
    for a_n, b_n, t in margins:
        first, block = scipy_block(a_n, b_n, t)
        offset += first
        pmf = np.convolve(pmf, np.array(block))
    return offset, [float(p) for p in pmf]


def exact_null(margins) -> tuple[int, list[Fraction]]:
    offset, pmf = 0, [Fraction(1)]
    for a_n, b_n, t in margins:
        first, block = exact_block(a_n, b_n, t)
        offset += first
        out = [Fraction(0)] * (len(pmf) + len(block) - 1)
        for i, p in enumerate(pmf):
            for j, q in enumerate(block):
                out[i + j] += p * q
        pmf = out
    return offset, pmf


def enumerated_null(margins) -> tuple[int, list[Fraction]]:
    """Brute force over every table set with these margins. The conditional probability of a set
    of tables is proportional to the product of 1/(cell!) over all cells, so this uses neither the
    hypergeometric formula nor a convolution."""
    ranges = [range(*(lambda f, l: (f, l + 1))(*support(a_n, b_n, t))) for a_n, b_n, t in margins]
    weight: dict[int, Fraction] = {}
    for xs in product(*ranges):
        w = Fraction(1)
        for x, (a_n, b_n, t) in zip(xs, margins):
            w /= (math.factorial(x) * math.factorial(a_n - x)
                  * math.factorial(t - x) * math.factorial(b_n - t + x))
        weight[sum(xs)] = weight.get(sum(xs), Fraction(0)) + w
    total = sum(weight.values())
    first = min(weight)
    return first, [weight.get(first + i, Fraction(0)) / total for i in range(max(weight) - first + 1)]


def float_mid_p(pmf: list[float], index: int) -> float:
    observed = pmf[index]
    less = [p for p in pmf if abs(p - observed) > MIDP_TIE * observed and p < observed]
    equal = [p for p in pmf if abs(p - observed) <= MIDP_TIE * observed]
    return min(1.0, math.fsum(less) + math.fsum(equal) / 2)


def exact_mid_p(pmf: list[Fraction], index: int) -> tuple[Fraction, bool]:
    """The exact mid-p, and whether another table is exactly as likely as the observed one.
    Raises if a table is unequal yet inside the tolerance, where the float rule would differ."""
    observed = pmf[index]
    for p in pmf:
        assert p == observed or abs(p - observed) > Fraction(MIDP_TIE) * observed, "near tie"
    less = sum((p for p in pmf if p < observed), Fraction(0))
    equal = [p for p in pmf if p == observed]
    return less + sum(equal, Fraction(0)) / 2, len(equal) > 1


def checked_p(margins, s: int, brute: bool) -> tuple[float, bool]:
    """scipy's mid-p of observed sum s, asserted against exact rational arithmetic."""
    offset, pmf = scipy_null(margins)
    index = s - offset
    p = float_mid_p(pmf, index)
    e_off, e_pmf = exact_null(margins)
    assert e_off == offset and len(e_pmf) == len(pmf), margins
    exact, tie = exact_mid_p(e_pmf, index)
    assert math.isclose(p, float(exact), rel_tol=1e-12, abs_tol=1e-15), (margins, s, p, exact)
    if brute:
        b_off, b_pmf = enumerated_null(margins)
        assert b_off == offset and b_pmf == e_pmf, margins
    return p, tie


def midp_rows() -> list[dict]:
    rows = []
    # Every table with at most 12 runs per arm, empty arms and zero or full passes included.
    for a_n in range(13):
        for b_n in range(13):
            for a_k in range(a_n + 1):
                for b_k in range(b_n + 1):
                    margin = [(a_n, b_n, a_k + b_k)]
                    p, tie = checked_p(margin, a_k, brute=a_n + b_n <= 12)
                    rows.append({"a_k": a_k, "a_n": a_n, "b_k": b_k, "b_n": b_n, "p": p,
                                 "tie": tie})
    rng = random.Random(SEED + 1)
    for _ in range(150):
        a_n, b_n = rng.randint(13, 200), rng.randint(13, 200)
        a_k, b_k = rng.randint(0, a_n), rng.randint(0, b_n)
        p, tie = checked_p([(a_n, b_n, a_k + b_k)], a_k, brute=False)
        rows.append({"a_k": a_k, "a_n": a_n, "b_k": b_k, "b_n": b_n, "p": p, "tie": tie})
    # Equal arms make mirror tables equally likely: ties at sizes where floats drift.
    for n in (40, 100, 200):
        for a_k, b_k in ((0, 1), (1, 0), (2, 5), (5, 2), (n // 2, n // 2 + 3), (n // 2 + 3, n // 2)):
            p, tie = checked_p([(n, n, a_k + b_k)], a_k, brute=False)
            rows.append({"a_k": a_k, "a_n": n, "b_k": b_k, "b_n": n, "p": p, "tie": tie})
    return rows


def margin_sets(blocks: int, total: int):
    """Every (a_n, b_n, t) tuple of `blocks` blocks with at most `total` runs overall."""
    def sizes(left: int, remaining: int):
        if remaining == 0:
            yield ()
            return
        for a_n in range(left + 1):
            for b_n in range(left - a_n + 1):
                for rest in sizes(left - a_n - b_n, remaining - 1):
                    yield ((a_n, b_n),) + rest
    for sized in sizes(total, blocks):
        for ts in product(*(range(a_n + b_n + 1) for a_n, b_n in sized)):
            yield tuple((a_n, b_n, t) for (a_n, b_n), t in zip(sized, ts))


def row_of(margins, s: int, rng: random.Random) -> dict:
    """A table set with these margins whose A passes sum to s, spread over the blocks at random."""
    while True:
        xs = [rng.randint(*support(*m)) for m in margins[:-1]]
        last = s - sum(xs)
        if support(*margins[-1])[0] <= last <= support(*margins[-1])[1]:
            xs.append(last)
            break
    p, tie = checked_p(margins, s, brute=False)
    return {"a_k": xs, "a_n": [m[0] for m in margins], "b_k": [m[2] - x for m, x in zip(margins, xs)],
            "b_n": [m[1] for m in margins], "p": p, "tie": tie}


def exact_rows() -> list[dict]:
    rows = []
    rng = random.Random(SEED + 2)
    # Brute force over every table set up to a total of 12 runs (K = 2), 8 (K = 3) and 6 (K = 4):
    # each margin set's null is enumerated and every observed sum is checked against scipy's
    # convolution. A sample of those sets is written out.
    for blocks, total, keep in ((2, 12, 0.003), (3, 8, 0.004), (4, 6, 0.004)):
        for margins in margin_sets(blocks, total):
            offset, pmf = scipy_null(margins)
            b_off, b_pmf = enumerated_null(margins)
            e_off, e_pmf = exact_null(margins)
            assert b_off == e_off == offset and b_pmf == e_pmf, margins
            for index in range(len(pmf)):
                p = float_mid_p(pmf, index)
                exact, _ = exact_mid_p(e_pmf, index)
                assert math.isclose(p, float(exact), rel_tol=1e-12, abs_tol=1e-15), margins
                if rng.random() < keep:
                    rows.append(row_of(margins, offset + index, rng))
    # Zero margins by hand: a block with no passes, with only passes, with an empty arm, and an
    # all-empty block, each next to ordinary blocks.
    for margins in (
        ((4, 4, 0), (5, 3, 4)), ((4, 4, 8), (5, 3, 4)), ((0, 4, 2), (5, 3, 4)), ((4, 0, 2), (5, 3, 4)),
        ((0, 0, 0), (5, 3, 4)), ((0, 0, 0), (4, 0, 0)), ((3, 3, 0), (4, 4, 8), (2, 6, 3)),
        ((5, 5, 5), (0, 3, 1), (4, 0, 1), (0, 0, 0)),
    ):
        offset, pmf = scipy_null(margins)
        for index in range(len(pmf)):
            rows.append(row_of(margins, offset + index, rng))
    # Larger sets, exact rational convolution as the oracle.
    for _ in range(120):
        blocks = rng.randint(2, 4)
        margins = []
        for _ in range(blocks):
            a_n, b_n = rng.randint(5, 60), rng.randint(5, 60)
            margins.append((a_n, b_n, rng.randint(0, a_n + b_n)))
        margins = tuple(margins)
        offset, pmf = scipy_null(margins)
        rows.append(row_of(margins, offset + rng.randrange(len(pmf)), rng))
    # Equal blocks, symmetric: ties between mirror tables.
    for margins in (((10, 10, 10),) * 2, ((20, 20, 20),) * 3, ((15, 15, 15),) * 4):
        offset, pmf = scipy_null(margins)
        for index in (0, 1, 2, len(pmf) // 2 - 2, len(pmf) // 2 + 1):
            rows.append(row_of(margins, offset + index, rng))
    return rows


def make_midp() -> dict:
    return {"engine": engine() | {"tie_tolerance": MIDP_TIE}, "rows": midp_rows()}


def make_exact() -> dict:
    return {"engine": engine() | {"tie_tolerance": MIDP_TIE}, "rows": exact_rows()}


FILES |= {"midp.toml": make_midp, "exact.toml": make_exact}


# --- cmh ---

import numpy as np
from statsmodels.stats.contingency_tables import StratifiedTable

MANTEL_FLEISS_MINIMUM = 5


def cmh_blocks(pa, na, pb, nb):
    """Per block `(a, b, c, d)`: arm B's passes and fails, then arm A's, so the odds ratio is the
    odds of passing in B over the odds in A."""
    return [(x, m - x, y, n - y) for y, n, x, m in zip(pa, na, pb, nb)]


def cmh_informative(block) -> bool:
    a, b, c, d = block
    return a + b > 0 and c + d > 0 and a + c > 0 and b + d > 0


def cmh_table(blocks) -> StratifiedTable:
    return StratifiedTable([np.array([[a, b], [c, d]], dtype=float) for a, b, c, d in blocks])


def mantel_fleiss(blocks) -> tuple[float, bool]:
    """The Mantel-Fleiss quantity from the margins, in exact rationals: the lower bound of a_k is
    max(0, m1 - n2) and the upper bound min(n1, m1), with n1 the row (arm B) runs, n2 the arm A
    runs and m1 the passes."""
    total_e, total_lo, total_hi = Fraction(0), 0, 0
    for a, b, c, d in blocks:
        n1, n2, m1, n = a + b, c + d, a + c, a + b + c + d
        total_e += Fraction(n1 * m1, n)
        total_lo += max(0, m1 - n2)
        total_hi += min(n1, m1)
    value = min(total_e - total_lo, total_hi - total_e)
    return float(value), value >= MANTEL_FLEISS_MINIMUM


def cmh_row(label, pa, na, pb, nb) -> dict:
    """Candler's convention: blocks with a zero margin are dropped; statsmodels is the oracle on
    the rest, and wherever it also returns a finite value on every block of at least two runs,
    that value is asserted equal."""
    blocks = cmh_blocks(pa, na, pb, nb)
    informative = [blk for blk in blocks if cmh_informative(blk)]
    wide = [blk for blk in blocks if sum(blk) >= 2]
    row = {"label": label, "pa": pa, "na": na, "pb": pb, "nb": nb, "informative": len(informative)}
    row["mf"], row["mf_holds"] = mantel_fleiss(informative)
    with np.errstate(all="ignore"):
        if wide:
            wide_table = cmh_table(wide)
            wide_null = wide_table.test_null_odds(correction=False)
            if not informative:
                assert not math.isfinite(float(wide_null.statistic)), (label, "all degenerate")
        if informative:
            table = cmh_table(informative)
            null = table.test_null_odds(correction=False)
            row["statistic"], row["p"] = float(null.statistic), float(null.pvalue)
            assert math.isclose(row["statistic"], float(wide_null.statistic), rel_tol=1e-12), label
            r_sum = sum(a * d / sum(blk) for blk in informative for a, _, _, d in [blk])
            s_sum = sum(b * c / sum(blk) for blk in informative for _, b, c, _ in [blk])
            odds = float(table.oddsratio_pooled)
            if s_sum == 0:
                assert not math.isfinite(odds), (label, odds)
            else:
                row["or_mh"] = odds
                if math.isfinite(float(wide_table.oddsratio_pooled)):
                    assert math.isclose(odds, float(wide_table.oddsratio_pooled), rel_tol=1e-12)
                if r_sum == 0:
                    assert odds == 0.0, (label, odds)
            if r_sum > 0 and s_sum > 0:
                lo, hi = table.oddsratio_pooled_confint(alpha=ALPHA)
                row["or_lo"], row["or_hi"] = float(lo), float(hi)
                assert 0 < row["or_lo"] < odds < row["or_hi"], label
                if len(informative) >= 2:
                    assert abs(odds - 1) > 1e-9, (label, "statsmodels divides by 1 - OR")
                    equal = table.test_equal_odds(adjust=False)
                    # statsmodels writes the p-value as 1 - cdf, which loses the tail; the
                    # survival function of the same statistic is exact.
                    row["hom_stat"] = float(equal.statistic)
                    row["hom_p"] = float(scipy.stats.chi2.sf(row["hom_stat"], len(informative) - 1))
                    assert abs(row["hom_p"] - float(equal.pvalue)) < 1e-12, label
    return row


def make_cmh() -> dict:
    fixed = [
        ("one block, Mantel-Fleiss exactly five", [3], [10], [7], [10]),
        ("one block, Mantel-Fleiss four", [2], [8], [6], [8]),
        ("zero cells in two blocks", [0, 7, 3], [10, 10, 10], [4, 0, 8], [10, 10, 10]),
        ("zero margin: no passes in a block", [0, 6, 4], [10, 10, 10], [0, 3, 8], [10, 10, 10]),
        ("zero margin: no fails in a block", [10, 6, 4], [10, 10, 10], [10, 3, 8], [10, 10, 10]),
        ("empty arm A in a block", [0, 6, 4], [0, 10, 10], [2, 3, 8], [5, 10, 10]),
        ("one informative block among degenerate ones", [0, 5, 10], [5, 10, 10], [0, 8, 10], [5, 10, 10]),
        ("all blocks degenerate", [0, 5], [5, 5], [0, 5], [5, 5]),
        ("no runs at all", [0], [0], [0], [0]),
        ("sum R zero: arm B never passes", [5, 6], [10, 10], [0, 0], [10, 10]),
        ("sum S zero: arm A never passes", [0, 0], [10, 10], [5, 6], [10, 10]),
        ("homogeneity rejected: opposite effects", [2, 18], [20, 20], [18, 3], [20, 20]),
    ]
    rng = random.Random(SEED)
    rows = [cmh_row(*spec) for spec in fixed]
    for k in range(2, 7):
        for repeat in range(3):
            na = [rng.randint(5, 200) for _ in range(k)]
            nb = [rng.randint(5, 200) for _ in range(k)]
            base = [rng.uniform(0.2, 0.8) for _ in range(k)]
            shift = rng.uniform(-0.25, 0.25)
            pa = [round(n * p) for n, p in zip(na, base)]
            pb = [round(n * min(0.97, max(0.03, p + shift))) for n, p in zip(nb, base)]
            rows.append(cmh_row(f"random K={k} #{repeat}", pa, na, pb, nb))
    assert any(row["mf"] < MANTEL_FLEISS_MINIMUM for row in rows), "no row fails the criterion"
    assert any(row["mf"] == MANTEL_FLEISS_MINIMUM for row in rows), "no row on the threshold"
    assert any(3 <= row["mf"] < MANTEL_FLEISS_MINIMUM for row in rows), "no row between 3 and 5"
    return {"engine": engine(), "rows": rows}


FILES |= {"cmh.toml": make_cmh}


# --- resampling ---

import itertools

import numpy as np
from scipy.stats import bootstrap, norm, permutation_test

U64 = (1 << 64) - 1
RESAMPLE_SEED = 20261118
MC_SIGMAS = 3.0


class SplitMix64:
    """The twin of `analysis::splitmix::SplitMix64` (Steele, Lea and Flood 2014)."""

    def __init__(self, seed: int):
        self.state = seed & U64
        self.rejected = 0

    def next_u64(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & U64
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & U64
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & U64
        return z ^ (z >> 31)

    def below(self, n: int) -> int:
        threshold = ((1 << 64) - n) % n  # 2^64 mod n
        while True:
            x = self.next_u64()
            if x >= threshold:
                return x % n
            self.rejected += 1

    def shuffle_within(self, items: list) -> None:
        for i in range(len(items) - 1, 0, -1):
            j = self.below(i + 1)
            items[i], items[j] = items[j], items[i]


def make_splitmix() -> dict:
    # The published first outputs of seed 0 (Vigna's splitmix64.c reference): a twin that
    # misreads the algorithm fails here before it can write a vector.
    first = SplitMix64(0)
    if (first.next_u64(), first.next_u64()) != (0xE220A8397B1DCDAF, 0x6E789E6AA1B965F4):
        sys.exit("gen_stats: the SplitMix64 twin does not reproduce the published seed-0 outputs")
    rows = []
    for seed in (0, 1, 42, 2**63 - 1, SEED):
        rng = SplitMix64(seed)
        rows.append({"kind": "next", "seed": seed,
                     "outputs": [f"{rng.next_u64():016x}" for _ in range(1000)]})
    below_ns = [1, 2, 3, 7, 10, 1000, 2**32 + 1, 2**62 + 1, 3 * 2**62, 2**63 + 1, 2**64 - 1]
    rejected_on_large = 0
    for n in below_ns:
        rng = SplitMix64(SEED)
        draws = [rng.below(n) for _ in range(200)]
        if n > 2**62:
            rejected_on_large += rng.rejected
        rows.append({"kind": "below", "seed": SEED, "n": str(n), "draws": [str(d) for d in draws]})
    if rejected_on_large == 0:
        sys.exit("gen_stats: no rejection on the large moduli, so `below` rows would not tell "
                 "rejection from a plain modulo")
    for length in (1, 2, 5, 17, 100):
        rng = SplitMix64(SEED)
        order = list(range(length))
        rng.shuffle_within(order)
        rows.append({"kind": "shuffle", "seed": SEED, "len": length, "order": order})
    return {"engine": {"python": platform.python_version()}, "rows": rows}


def arm_sums(runs):
    num = den = 0.0
    for n, d in runs:
        num += n
        den += d
    return num, den


def statistic_of(summaries) -> float:
    total = float(sum(n for _, _, n in summaries))
    t = 0.0
    for sa, sb, n in summaries:
        t += (n / total) * (sb[0] / sb[1] - sa[0] / sa[1])
    return t


def summarise(blocks):
    return [(arm_sums(a), arm_sums(b), len(a) + len(b)) for a, b in blocks]


def perm_cutoff(observed: float) -> float:
    return abs(observed) - 1e-12 * max(1.0, abs(observed))


def stratified_permutation(blocks, b: int, seed: int):
    observed = statistic_of(summarise(blocks))
    cutoff = perm_cutoff(observed)
    pools = [(len(a), list(a) + list(bb)) for a, bb in blocks]
    rng = SplitMix64(seed)
    extreme = 0
    for _ in range(b):
        summaries = []
        for n_a, runs in pools:
            order = list(range(len(runs)))
            rng.shuffle_within(order)
            summaries.append((arm_sums(runs[i] for i in order[:n_a]),
                              arm_sums(runs[i] for i in order[n_a:]), len(runs)))
        if abs(statistic_of(summaries)) >= cutoff:
            extreme += 1
    return observed, (extreme + 1) / (b + 1), extreme


def exact_permutation_p(blocks) -> float:
    """Exact p over every within-block relabelling (the observed one included)."""
    observed = statistic_of(summarise(blocks))
    cutoff = perm_cutoff(observed)
    per_block = []
    for a, bb in blocks:
        pool = list(a) + list(bb)
        options = []
        for in_a in itertools.combinations(range(len(pool)), len(a)):
            chosen = set(in_a)
            options.append((arm_sums(pool[i] for i in in_a),
                            arm_sums(pool[i] for i in range(len(pool)) if i not in chosen),
                            len(pool)))
        per_block.append(options)
    hits = total = 0
    for combo in itertools.product(*per_block):
        total += 1
        if abs(statistic_of(list(combo))) >= cutoff:
            hits += 1
    return hits / total


def pooled_shuffle_p(blocks, b: int, seed: int) -> float:
    """The wrong test: labels shuffled over all runs of all blocks. Used as a negative control."""
    observed = statistic_of(summarise(blocks))
    cutoff = perm_cutoff(observed)
    pool = [(k, run) for k, (a, bb) in enumerate(blocks) for run in list(a) + list(bb)]
    n_a = [len(a) for a, _ in blocks]
    sizes = [len(a) + len(bb) for a, bb in blocks]
    rng = SplitMix64(seed)
    extreme = 0
    for _ in range(b):
        order = list(range(len(pool)))
        rng.shuffle_within(order)
        labels = ["B"] * len(pool)
        for i in order[:sum(n_a)]:
            labels[i] = "A"
        summaries = []
        for k, size in enumerate(sizes):
            runs_a = [pool[i][1] for i in range(len(pool)) if pool[i][0] == k and labels[i] == "A"]
            runs_b = [pool[i][1] for i in range(len(pool)) if pool[i][0] == k and labels[i] == "B"]
            if not runs_a or not runs_b:
                summaries = None
                break
            summaries.append((arm_sums(runs_a), arm_sums(runs_b), size))
        if summaries is not None and abs(statistic_of(summaries)) >= cutoff:
            extreme += 1
    return (extreme + 1) / (b + 1)


def synth_block(rng, n_a: int, n_b: int, rate_a: float, rate_b: float):
    """Runs as (num, den): den expectations of weight 1 to 4, each passing with the arm's rate."""
    def arm(n, rate):
        runs = []
        for _ in range(n):
            den = rng.randint(2, 5)
            runs.append((float(sum(rng.random() < rate for _ in range(den))), float(den)))
        return runs
    return arm(n_a, rate_a), arm(n_b, rate_b)


def agree(estimate: float, reference: float, se: float) -> bool:
    return abs(estimate - reference) <= MC_SIGMAS * se


def permutation_designs():
    rng = random.Random(SEED)
    mild = [synth_block(rng, 3, 3, 0.4, 0.5), synth_block(rng, 4, 3, 0.6, 0.6)]
    one = [synth_block(rng, 12, 12, 0.4, 0.6)]
    two = [synth_block(rng, 8, 8, 0.3, 0.5), synth_block(rng, 10, 6, 0.7, 0.7)]
    three = [synth_block(rng, 6, 6, 0.2, 0.2), synth_block(rng, 9, 4, 0.5, 0.7),
             synth_block(rng, 5, 12, 0.9, 0.8)]
    equal = [([(1.0, 2.0)] * 4, [(1.0, 2.0)] * 4), ([(2.0, 2.0)] * 3, [(2.0, 2.0)] * 5)]
    # Blocks of rates 0 and 1; B differs from A by one run in each block.
    gap = [([(0.0, 1.0)] * 6, [(0.0, 1.0)] * 5 + [(1.0, 1.0)]),
           ([(1.0, 1.0)] * 5 + [(0.0, 1.0)], [(1.0, 1.0)] * 6)]
    simpson = [([(0.0, 1.0)] * 3, [(1.0, 1.0)] * 3), ([(1.0, 2.0)] * 3, [(2.0, 2.0)] * 3)]
    # Ratios over 3 and 7: permuted |T*| equals |T| only up to rounding, so the tolerance counts.
    r = random.Random(2)
    tie = [([(float(r.randint(0, 3)), 3.0) for _ in range(4)],
            [(float(r.randint(0, 3)), 3.0) for _ in range(4)]),
           ([(float(r.randint(0, 7)), 7.0) for _ in range(5)],
            [(float(r.randint(0, 7)), 7.0) for _ in range(5)])]
    return [("tie", tie, 2000), ("mild", mild, 10000), ("one", one, 10000), ("two", two, 2000),
            ("three", three, 2000), ("equal", equal, 500), ("gap", gap, 10000),
            ("simpson", simpson, 2000)]


def flat(blocks) -> dict:
    return {
        "a_sizes": [len(a) for a, _ in blocks], "b_sizes": [len(b) for _, b in blocks],
        "a_num": [n for a, _ in blocks for n, _ in a], "a_den": [d for a, _ in blocks for _, d in a],
        "b_num": [n for _, b in blocks for n, _ in b], "b_den": [d for _, b in blocks for _, d in b],
    }


def make_permutation() -> dict:
    rows = []
    results = {}
    for index, (name, blocks, b) in enumerate(permutation_designs()):
        seed = RESAMPLE_SEED + index
        statistic, p, extreme = stratified_permutation(blocks, b, seed)
        results[name] = (blocks, b, p)
        rows.append({"name": name, "seed": seed, "b": b, **flat(blocks),
                     "statistic": statistic, "p": p, "extreme": extreme})
    # The tie tolerance must matter on the `tie` row, or no row could tell it from none.
    blocks, b, _ = results["tie"]
    seed = RESAMPLE_SEED
    cutoff = perm_cutoff
    try:
        globals()["perm_cutoff"] = abs
        strict = stratified_permutation(blocks, b, seed)[2]
    finally:
        globals()["perm_cutoff"] = cutoff
    if strict == stratified_permutation(blocks, b, seed)[2]:
        sys.exit("gen_stats: the `tie` row does not depend on the tie tolerance")
    # Monte Carlo agreement 1: the exact p over every within-block relabelling.
    for name in ("mild", "gap"):
        blocks, b, p = results[name]
        exact = exact_permutation_p(blocks)
        se = math.sqrt(exact * (1 - exact) / b)
        if not agree(p, exact, se):
            sys.exit(f"gen_stats: permutation `{name}`: p {p} is not within {MC_SIGMAS} SE "
                     f"({se:.4g}) of the exact {exact}")
    # Negative control: the same check must reject a pooled shuffle, or it could not fail.
    blocks, b, _ = results["gap"]
    exact = exact_permutation_p(blocks)
    pooled = pooled_shuffle_p(blocks, 2000, RESAMPLE_SEED)
    if agree(pooled, exact, math.sqrt(exact * (1 - exact) / 2000)):
        sys.exit("gen_stats: the exact-p check cannot tell a pooled shuffle from the stratified "
                 "one")
    # Monte Carlo agreement 2: scipy's permutation_test for one block.
    blocks, b, p = results["one"]
    a_runs, b_runs = blocks[0]
    num = np.array([n for n, _ in a_runs + b_runs])
    den = np.array([d for _, d in a_runs + b_runs])

    def stat(ia, ib):
        return num[ib].sum() / den[ib].sum() - num[ia].sum() / den[ia].sum()

    ia = np.arange(len(a_runs))
    ib = np.arange(len(a_runs), len(a_runs) + len(b_runs))
    scipy_p = permutation_test((ia, ib), stat, permutation_type="independent", vectorized=False,
                               n_resamples=b, alternative="two-sided",
                               random_state=np.random.default_rng(RESAMPLE_SEED)).pvalue
    se = math.sqrt(2 * scipy_p * (1 - scipy_p) / b)
    if not agree(p, float(scipy_p), se):
        sys.exit(f"gen_stats: permutation `one`: p {p} is not within {MC_SIGMAS} SE ({se:.4g}) "
                 f"of scipy's {scipy_p}")
    return {"engine": engine(), "rows": rows}


def jackknife_acceleration(blocks) -> float:
    base = summarise(blocks)
    thetas = []
    for k, (a, bb) in enumerate(blocks):
        for is_b, runs in ((False, a), (True, bb)):
            for num, den in runs:
                s = list(base)
                sa, sb, n = s[k]
                if is_b:
                    sb = (sb[0] - num, sb[1] - den)
                else:
                    sa = (sa[0] - num, sa[1] - den)
                s[k] = (sa, sb, n)
                thetas.append(statistic_of(s))
    if all(t == thetas[0] for t in thetas):
        return 0.0
    total = 0.0
    for t in thetas:
        total += t
    mean = total / len(thetas)
    cubes = squares = 0.0
    for t in thetas:
        d = mean - t
        cubes += d * d * d
        squares += d * d
    return cubes / (6.0 * squares * math.sqrt(squares))


def interpolated_quantile(sorted_values, q: float) -> float:
    position = min(max(q, 0.0), 1.0) * (len(sorted_values) - 1)
    lower = math.floor(position)
    fraction = position - lower
    if lower + 1 < len(sorted_values):
        return sorted_values[lower] + fraction * (sorted_values[lower + 1] - sorted_values[lower])
    return sorted_values[lower]


def stratified_bca(blocks, b: int, level: float, seed: int):
    base = summarise(blocks)
    observed = statistic_of(base)
    rng = SplitMix64(seed)
    thetas = []
    for _ in range(b):
        summaries = []
        for a, bb in blocks:
            sums = []
            for runs in (a, bb):
                drawn = [runs[rng.below(len(runs))] for _ in range(len(runs))]
                sums.append(arm_sums(drawn))
            summaries.append((sums[0], sums[1], len(a) + len(bb)))
        thetas.append(statistic_of(summaries))
    thetas.sort()
    if thetas[0] == thetas[-1] == observed:
        return observed, observed
    below = sum(1 for t in thetas if t < observed)
    below = min(max(below, 1), b - 1)
    z0 = float(norm.ppf(below / b))
    accel = jackknife_acceleration(blocks)
    alpha = 1.0 - level
    ends = []
    for tail in (alpha / 2.0, 1.0 - alpha / 2.0):
        z = z0 + float(norm.ppf(tail))
        denominator = 1.0 - accel * z
        if not denominator > 0.0:
            sys.exit("gen_stats: BCa acceleration leaves the quantile undefined")
        ends.append(interpolated_quantile(thetas, float(norm.cdf(z0 + z / denominator))))
    return ends[0], ends[1]


def bca_designs():
    rng = random.Random(SEED + 1)
    one = [synth_block(rng, 15, 15, 0.4, 0.6)]
    two = [synth_block(rng, 8, 8, 0.3, 0.5), synth_block(rng, 10, 6, 0.7, 0.7)]
    three = [synth_block(rng, 6, 6, 0.2, 0.2), synth_block(rng, 9, 4, 0.5, 0.7),
             synth_block(rng, 5, 12, 0.9, 0.8)]
    binary = [([(float(i % 2), 1.0) for i in range(8)], [(float(i % 3 == 0), 1.0) for i in range(9)])]
    # One block with every run equal, beside one that varies.
    degenerate_mix = [([(1.0, 2.0)] * 4, [(1.0, 2.0)] * 5), two[0]]
    degenerate = [([(1.0, 2.0)] * 4, [(1.0, 2.0)] * 5)]
    # Heavy-tailed runs: the acceleration is about 0.09, so scipy's interval separates a wrong one.
    r = random.Random(0)
    skew = [([(r.expovariate(1 / 0.3) ** 2, 1.0) for _ in range(12)],
             [(r.expovariate(1 / 0.5) ** 2, 1.0) for _ in range(12)])]
    return [("skew", skew, 2000, 0.95), ("one", one, 2000, 0.95), ("two", two, 2000, 0.9), ("three", three, 2000, 0.95),
            ("binary", binary, 2000, 0.95), ("degenerate_mix", degenerate_mix, 2000, 0.95),
            ("degenerate", degenerate, 200, 0.95)]


def make_bca() -> dict:
    rows = []
    results = {}
    for index, (name, blocks, b, level) in enumerate(bca_designs()):
        seed = RESAMPLE_SEED + 100 + index
        low, high = stratified_bca(blocks, b, level, seed)
        results[name] = (blocks, b, level, low, high)
        rows.append({"name": name, "seed": seed, "b": b, "level": level, **flat(blocks),
                     "low": low, "high": high})
    # Monte Carlo agreement: scipy's BCa over 20 seeds for one block; the twin's endpoints must lie
    # within 3 SE of scipy's mean, SE being the seed-to-seed SD of scipy's own endpoints with the
    # twin's one draw added in quadrature. On `skew` the same check must reject a twin whose
    # acceleration has the wrong sign or is 0, or it could not tell them from the right one.
    def disagrees(blocks, b, level, ends, low, high) -> bool:
        flags = []
        for which, mine in enumerate((low, high)):
            values = np.array([e[which] for e in ends])
            se = float(values.std(ddof=1)) * math.sqrt(1 + 1 / len(values))
            flags.append(not agree(mine, float(values.mean()), se))
        return any(flags)

    for name in ("one", "skew"):
        blocks, b, level, low, high = results[name]
        a_runs, b_runs = blocks[0]
        num = np.array([n for n, _ in a_runs + b_runs])
        den = np.array([d for _, d in a_runs + b_runs])

        def stat(ia, ib, axis=-1):
            return (num[ib].sum(axis=axis) / den[ib].sum(axis=axis)
                    - num[ia].sum(axis=axis) / den[ia].sum(axis=axis))

        ia = np.arange(len(a_runs))
        ib = np.arange(len(a_runs), len(a_runs) + len(b_runs))
        ends = []
        for seed in range(20):
            ci = bootstrap((ia, ib), stat, vectorized=True, method="BCa", n_resamples=b,
                           confidence_level=level, random_state=np.random.default_rng(seed)
                           ).confidence_interval
            ends.append((float(ci.low), float(ci.high)))
        if disagrees(blocks, b, level, ends, low, high):
            sys.exit(f"gen_stats: BCa `{name}`: the twin's ({low}, {high}) is not within "
                     f"{MC_SIGMAS} SE of scipy's mean")
        if name == "skew":
            true_accel = jackknife_acceleration
            for label, wrong in (("flipped", lambda bl: -true_accel(bl)), ("zero", lambda bl: 0.0)):
                globals()["jackknife_acceleration"] = wrong
                try:
                    bad_low, bad_high = stratified_bca(blocks, b, level, RESAMPLE_SEED + 100)
                finally:
                    globals()["jackknife_acceleration"] = true_accel
                if not disagrees(blocks, b, level, ends, bad_low, bad_high):
                    sys.exit(f"gen_stats: scipy's check cannot tell a {label} acceleration "
                             "from the right one")
    return {"engine": engine(), "rows": rows}


FILES |= {"splitmix.toml": make_splitmix, "permutation.toml": make_permutation,
          "bca.toml": make_bca}


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
