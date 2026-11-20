#!/usr/bin/env python3
"""Enforce a minimum line-coverage gate from a cargo-llvm-cov lcov report.

Usage:
    python3 scripts/coverage_gate.py --lcov lcov.info --min 90 [--exclude src/test.rs]

Prints a summary line and exits non-zero when coverage is below the gate.
"""

import argparse
import sys

DEFAULT_EXCLUDES = ("src/test.rs",)


def parse_lcov(path):
    """Return {file: (lines_hit, lines_found)} from an lcov tracefile."""
    files = {}
    current = None
    hit = found = 0
    with open(path, encoding="utf-8") as handle:
        for raw in handle:
            line = raw.strip()
            if line.startswith("SF:"):
                current = line[3:]
                hit = found = 0
            elif line.startswith("DA:"):
                # DA:<line>,<count>[,<checksum>]
                parts = line[3:].split(",")
                if len(parts) >= 2:
                    found += 1
                    if int(parts[1]) > 0:
                        hit += 1
            elif line == "end_of_record":
                if current is not None:
                    prev_hit, prev_found = files.get(current, (0, 0))
                    files[current] = (prev_hit + hit, prev_found + found)
                current = None
    return files


def summarize(files, excludes):
    hit = found = 0
    for name, (file_hit, file_found) in files.items():
        if any(name.endswith(ex) for ex in excludes):
            continue
        hit += file_hit
        found += file_found
    percent = (100.0 * hit / found) if found else 0.0
    return hit, found, percent


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lcov", default="lcov.info", help="path to lcov tracefile")
    parser.add_argument("--min", type=float, required=True, help="minimum line coverage %%")
    parser.add_argument(
        "--exclude",
        action="append",
        default=list(DEFAULT_EXCLUDES),
        help="path suffix to exclude (repeatable)",
    )
    args = parser.parse_args(argv)

    files = parse_lcov(args.lcov)
    if not files:
        print(f"error: no coverage records found in {args.lcov}", file=sys.stderr)
        return 2

    hit, found, percent = summarize(files, args.exclude)
    print(
        f"line coverage: {percent:.2f}% ({hit}/{found} lines, "
        f"excluding {', '.join(args.exclude)})"
    )
    if percent + 1e-9 < args.min:
        print(
            f"error: coverage {percent:.2f}% is below the {args.min:.2f}% gate",
            file=sys.stderr,
        )
        return 1
    print(f"ok: coverage meets the {args.min:.2f}% gate")
    return 0


if __name__ == "__main__":
    sys.exit(main())
