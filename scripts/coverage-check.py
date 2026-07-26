#!/usr/bin/env python3
"""Coverage threshold gate for andre.

Reads `cargo llvm-cov report --json` from stdin and enforces two line-coverage
floors:

    andre-core >= 90%  (raised M09 S5 from 88%; measured 91.78%)
    andre      >= 38%  (M42 baseline; measured 54.17%)

See `FLOORS` below for the history. The JSON schema is what `cargo-llvm-cov`
exports (one entry per binary / artifact under data[*].files). We aggregate
line counts per crate by detecting the crate root path segment.

Exit codes:
    0 — both crates at or above floor (or absent on a no-source crate)
    1 — at least one crate is below floor (FAIL message printed to stderr)
    2 — JSON parse error or missing crates
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# (crate marker, threshold)
#
# History:
#   M42 — established at andre-core 88% / andre 38%
#   M09  — raised andre-core 88 -> 90 (+2pp; measured 91.78%, headroom 3.78pp).
#          andre left at 38% (measured 54.17%, +16pp headroom); the TUI surface
#          has many render/interactive paths that are hard to cover without a
#          UI harness, so raising the floor without coverage growth would
#          create CI flakiness.
FLOORS = [
    ("andre-core", 90.0),
    ("andre", 38.0),
]


def aggregate(payload: dict) -> dict[str, dict[str, int]]:
    totals: dict[str, dict[str, int]] = {}
    for entry in payload.get("data", []):
        for f in entry.get("files", []):
            path = f.get("filename", "")
            crate = None
            for marker, _ in FLOORS:
                # Match `/<marker>/src/` segment; rejects `andre-core` when
                # scanning for `andre`.
                seg = f"/{marker}/src/"
                if seg in path or path.endswith(f"/{marker}/src/main.rs"):
                    crate = marker
                    break
            if crate is None:
                continue
            summary = f.get("summary", {})
            lines = summary.get("lines", {})
            slot = totals.setdefault(
                crate, {"count": 0, "covered": 0}
            )
            slot["count"] += int(lines.get("count", 0))
            slot["covered"] += int(lines.get("covered", 0))
    return totals


def main() -> int:
    raw = sys.stdin.read()
    try:
        payload = json.loads(raw)
    except json.JSONDecodeError as exc:
        print(f"coverage-check: invalid JSON: {exc}", file=sys.stderr)
        return 2

    totals = aggregate(payload)
    missing = [m for m, _ in FLOORS if m not in totals]
    if missing:
        print(
            f"coverage-check: missing crate(s) in coverage report: "
            f"{', '.join(missing)}",
            file=sys.stderr,
        )
        return 2

    failures = 0
    for crate, floor in FLOORS:
        slot = totals[crate]
        covered = slot["covered"]
        total = slot["count"]
        pct = (covered / total * 100.0) if total else 0.0
        if pct + 1e-9 < floor:
            print(
                f"FAIL: {crate} coverage {pct:.2f}% < {floor:.0f}% "
                f"({covered}/{total} lines)",
                file=sys.stderr,
            )
            failures += 1

    if failures:
        print("Coverage thresholds: FAIL", file=sys.stderr)
        return 1

    for crate, floor in FLOORS:
        slot = totals[crate]
        covered = slot["covered"]
        total = slot["count"]
        pct = (covered / total * 100.0) if total else 0.0
        print(
            f"{crate}: {pct:.2f}% (>= {floor:.0f}%, {covered}/{total} lines)"
        )
    print("Coverage thresholds: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
