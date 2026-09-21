#!/usr/bin/env python3
"""Require actual passing test events for every offline P039 acceptance row."""
import argparse
import json
import re
from pathlib import Path


def check(mapping, log):
    expected = {f"CF-{index:02}" for index in range(1, 30)}
    if not isinstance(mapping, dict) or set(mapping) != expected:
        raise ValueError("coverage map must contain exactly CF-01 through CF-29")
    events = {}
    for name, result in re.findall(r"^test ([A-Za-z0-9_:]+) \.\.\. (ok|FAILED|ignored)\b", log, re.M):
        events.setdefault(name, []).append(result)
    failures = []
    for case, names in sorted(mapping.items()):
        if not isinstance(names, list) or not names or not all(isinstance(name, str) and name for name in names):
            raise ValueError(f"{case}: nonempty exact test-name list required")
        if len(names) != len(set(names)):
            raise ValueError(f"{case}: duplicate mapped test")
        for name in names:
            if events.get(name) != ["ok"]:
                failures.append(f"{case}: {name}: expected one executed pass, found {events.get(name, [])}")
    if failures:
        raise ValueError("\n".join(failures))
    return len(expected), len({name for names in mapping.values() for name in names})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--map", type=Path, default=Path(__file__).with_name("p039-coverage.json"))
    args = parser.parse_args()
    try:
        cases, tests = check(json.loads(args.map.read_text()), args.log.read_text())
    except (ValueError, OSError) as error:
        parser.exit(1, f"P039 execution coverage FAILED: {error}\n")
    print(f"P039 execution coverage: {cases} cases, {tests} distinct mapped tests passed; live acceptance is separate")


if __name__ == "__main__":
    main()
