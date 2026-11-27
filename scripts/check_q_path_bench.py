#!/usr/bin/env python3
"""Compare the Q-path bench medians of two criterion baselines.

The CI job runs the evaluate_gum bench twice in one job: once on the
pre-Scalar tree (0cfb3a8, the "before" tree of #31) saved as baseline
"base", and once on the PR tree saved as baseline "head". This script
reads both medians and fails when head/base is above the tolerance.

Usage: check_q_path_bench.py <criterion dir> <max ratio>
"""

import json
import sys
from pathlib import Path


def median_ns(criterion_dir: Path, baseline: str) -> float:
    found = sorted(criterion_dir.glob(f"*/{baseline}/estimates.json"))
    if len(found) != 1:
        sys.exit(f"expected one {baseline}/estimates.json under {criterion_dir}, found {len(found)}")
    estimates = json.loads(found[0].read_text())
    return float(estimates["median"]["point_estimate"])


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    criterion_dir = Path(sys.argv[1])
    max_ratio = float(sys.argv[2])
    base = median_ns(criterion_dir, "base")
    head = median_ns(criterion_dir, "head")
    ratio = head / base
    print(f"base (0cfb3a8, pre-Scalar) median: {base / 1e6:.2f} ms")
    print(f"head median: {head / 1e6:.2f} ms")
    print(f"ratio head/base: {ratio:.3f} (limit {max_ratio:.3f}; #31 measured 0.975)")
    if ratio > max_ratio:
        sys.exit(f"Q-path regression: ratio {ratio:.3f} is above {max_ratio:.3f}")
    print("Q-path bench gate: pass")


if __name__ == "__main__":
    main()
