"""The second engine of the golden design: the expected comparisons of a store of 3 models x 4
htn-creator versions x 20 runs, from scipy and statsmodels alone. It imports nothing of Candler
and reads none of the golden runs. `python second_engine.py` writes expected.toml; `--check`
exits non-zero when the file on disk differs from what the engines give.

What it matches: for the binary expectations the battery, p, odds ratio and interval, per-block
differences and Holm (per comparison, per role over the gates). For the weighted expectation the
battery, the block count, the per-block differences and the point estimate (the block-size-weighted
difference of Sum num / Sum den); the permutation draws and the BCa interval are the generator's
SplitMix64 stream and are matched in the core crate, not here. The quality family's Holm is
therefore not written: it needs the weighted p."""
import math
import sys
import tomllib
from fractions import Fraction
from functools import lru_cache
from pathlib import Path

import numpy as np
from scipy.stats import hypergeom
from statsmodels.stats.contingency_tables import StratifiedTable
from statsmodels.stats.multitest import multipletests

HERE = Path(__file__).resolve().parent
ALPHA = 0.05
MIDP_TIE = 1e-7
MANTEL_FLEISS_MINIMUM = 5
BINARY = [("g1", "gate"), ("g2", "gate"), ("q1", "quality")]


def build_store(design):
    """{(model, version): [run, ...]}, a run being {'g1','g2','q1': bool, 'w': (num, den)}."""
    store = {}
    for cell in design["cell"]:
        runs = []
        for r in range(1, design["replicates"] + 1):
            num = design["weight_full"] if r <= cell["w"] else design["weight_low"]
            runs.append({
                "g1": r <= cell["g1"], "g2": r <= cell["g2"], "q1": r <= cell["q1"],
                "w": (num, design["weight_den"]),
            })
        store[(cell["m"], cell["v"])] = runs
    return store


def blocks_of(store, design, factor, a, b):
    """The blocks of a comparison: one per level of the other factor, as (label, arm A, arm B)."""
    out = []
    if factor == "request.treatment":
        for m in range(len(design["models"])):
            out.append((design["models"][m], store[(m, a)], store[(m, b)]))
    else:
        for v in range(len(design["tags"])):
            out.append((design["tags"][v], store[(a, v)], store[(b, v)]))
    return out


def counts(runs, name):
    return sum(1 for run in runs if run[name]), len(runs)


@lru_cache(maxsize=None)
def block_pmf(a_n, b_n, t):
    first, last = max(0, t - b_n), min(a_n, t)
    if first == last:
        return first, (1.0,)
    return first, tuple(float(hypergeom.pmf(x, a_n + b_n, t, a_n)) for x in range(first, last + 1))


def convolution(margins):
    offset, pmf = 0, np.array([1.0])
    for a_n, b_n, t in margins:
        first, block = block_pmf(a_n, b_n, t)
        offset += first
        pmf = np.convolve(pmf, np.array(block))
    return offset, [float(p) for p in pmf]


def midp_of(pmf, index):
    observed = pmf[index]
    less = [p for p in pmf if abs(p - observed) > MIDP_TIE * observed and p < observed]
    equal = [p for p in pmf if abs(p - observed) <= MIDP_TIE * observed]
    return min(1.0, math.fsum(less) + math.fsum(equal) / 2)


def midp(ka, na, kb, nb):
    """scipy's hypergeometric mid-p of one 2x2 table (A's passes the statistic)."""
    first, pmf = block_pmf(na, nb, ka + kb)
    return midp_of(list(pmf), ka - first)


def exact_conditional(tables):
    """The exact conditional mid-p over the blocks: the convolution of their hypergeometrics."""
    margins = [(na, nb, ka + kb) for (ka, na), (kb, nb) in tables]
    offset, pmf = convolution(margins)
    return midp_of(pmf, sum(ka for (ka, _), _ in tables) - offset)


def informative(a, b, c, d):
    return a + b > 0 and c + d > 0 and a + c > 0 and b + d > 0


def mantel_fleiss(blocks):
    total_e, lo, hi = Fraction(0), 0, 0
    for a, b, c, d in blocks:
        n1, n2, m1, n = a + b, c + d, a + c, a + b + c + d
        total_e += Fraction(n1 * m1, n)
        lo += max(0, m1 - n2)
        hi += min(n1, m1)
    return min(total_e - lo, hi - total_e) >= MANTEL_FLEISS_MINIMUM


def cmh(tables):
    """tables: per block ((ka, na), (kb, nb)). Rows are B's passes and fails, then A's."""
    blocks = [(kb, nb - kb, ka, na - ka) for (ka, na), (kb, nb) in tables]
    kept = [blk for blk in blocks if informative(*blk)]
    table = StratifiedTable([np.array([[a, b], [c, d]], dtype=float) for a, b, c, d in kept])
    null = table.test_null_odds(correction=False)
    lo, hi = table.oddsratio_pooled_confint(alpha=ALPHA)
    return {
        "p": float(null.pvalue), "or_mh": float(table.oddsratio_pooled),
        "or_lo": float(lo), "or_hi": float(hi), "mf": mantel_fleiss(kept),
    }


def holm(ps):
    return [float(x) for x in multipletests(ps, alpha=ALPHA, method="holm")[1]]


def weighted_row(blocks):
    total = sum(len(a) + len(b) for _, a, b in blocks)
    per_block, statistic = [], 0.0
    for label, a, b in blocks:
        ra = sum(r["w"][0] for r in a) / sum(r["w"][1] for r in a)
        rb = sum(r["w"][0] for r in b) / sum(r["w"][1] for r in b)
        per_block.append({"key": label, "diff": rb - ra})
        statistic += (len(a) + len(b)) / total * (rb - ra)
    return {"battery": "permutation", "blocks": len(blocks), "diff": statistic, "per_block": per_block}


def binary_row(blocks, name):
    tables = [(counts(a, name), counts(b, name)) for _, a, b in blocks]
    per_block = [
        {"key": label, "a_k": ka, "a_n": na, "b_k": kb, "b_n": nb, "diff": kb / nb - ka / na}
        for (label, _, _), ((ka, na), (kb, nb)) in zip(blocks, tables)
    ]
    if len(blocks) == 1:
        (ka, na), (kb, nb) = tables[0]
        p = midp(ka, na, kb, nb)
        assert abs(p - exact_conditional(tables)) < 1e-12
        return {"battery": "mid_p", "blocks": 1, "fallback": False, "p": p, "per_block": per_block}
    result = cmh(tables)
    row = {"blocks": len(blocks), "per_block": per_block}
    if result["mf"]:
        row.update(battery="cmh", fallback=False, p=result["p"])
        row.update(or_mh=result["or_mh"], or_lo=result["or_lo"], or_hi=result["or_hi"])
        row["reading"] = "effect" if result["or_lo"] > 1 or result["or_hi"] < 1 else "inconclusive"
    else:
        row.update(battery="exact_strata", fallback=True, p=exact_conditional(tables))
        row.update(or_mh=result["or_mh"], or_lo=result["or_lo"], or_hi=result["or_hi"])
        row["reading"] = "effect" if result["or_lo"] > 1 or result["or_hi"] < 1 else "inconclusive"
    return row


def comparisons(design):
    n_models, n_versions = len(design["models"]), len(design["tags"])
    for a in range(n_versions):
        for b in range(a + 1, n_versions):
            yield "request.treatment", a, b, design["tags"][a], design["tags"][b]
    for a in range(n_models):
        for b in range(a + 1, n_models):
            yield "model.id", a, b, design["models"][a], design["models"][b]


def expected_rows(design):
    store = build_store(design)
    rows = []
    for factor, a, b, la, lb in comparisons(design):
        blocks = blocks_of(store, design, factor, a, b)
        made = [dict(binary_row(blocks, name), expectation=name, role=role) for name, role in BINARY]
        gates = [r for r in made if r["role"] == "gate"]
        for r, adjusted in zip(gates, holm([r["p"] for r in gates])):
            r["p_holm"] = adjusted
        weighted = weighted_row(blocks)
        weighted.update(expectation="w", role="quality")
        for row in made + [weighted]:
            row.update(factor=factor, a=la, b=lb)
            rows.append(row)
    return rows


def scalar(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, str):
        return '"' + value + '"'
    if isinstance(value, int):
        return str(value)
    return repr(float(value))


def inline(table):
    return "{" + ", ".join(f"{k} = {scalar(v)}" for k, v in table.items()) + "}"


def render(rows):
    out = ["# Written by second_engine.py; do not edit.", ""]
    for row in rows:
        out.append("[[row]]")
        for key, value in row.items():
            if key == "per_block":
                out.append("per_block = [" + ", ".join(inline(t) for t in value) + "]")
            else:
                out.append(f"{key} = {scalar(value)}")
        out.append("")
    return "\n".join(out)


def main():
    design = tomllib.loads((HERE / "design.toml").read_text())
    text = render(expected_rows(design))
    path = HERE / "expected.toml"
    if "--check" in sys.argv:
        if not path.exists() or path.read_text() != text:
            print("expected.toml drifts from the engines")
            return 1
        print("rows match")
        return 0
    path.write_text(text)
    print(f"wrote {path.name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
