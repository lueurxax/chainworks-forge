#!/usr/bin/env python3
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

VALID_SUFFIXES = {'.md', '.markdown'}


def next_report_path(proposal_path: Path) -> Path:
    proposal_path = proposal_path.resolve()
    parent = proposal_path.parent
    stem = proposal_path.stem
    pattern = re.compile(rf"^{re.escape(stem)}_IMPLEMENTATION_AUDIT_R(\d+)\.md$")

    max_revision = 0
    for sibling in parent.iterdir():
        match = pattern.match(sibling.name)
        if match:
            max_revision = max(max_revision, int(match.group(1)))

    return parent / f"{stem}_IMPLEMENTATION_AUDIT_R{max_revision + 1}.md"


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            'Return the next versioned implementation audit report path for a proposal markdown file.'
        )
    )
    parser.add_argument('proposal_path', help='Path to the proposal markdown file')
    return parser.parse_args(argv)


def validate_proposal_path(raw_path: str) -> Path:
    proposal_path = Path(raw_path).expanduser()
    if not proposal_path.exists():
        raise ValueError(f'proposal not found: {proposal_path}')
    if not proposal_path.is_file():
        raise ValueError(f'proposal is not a file: {proposal_path}')
    if proposal_path.suffix.lower() not in VALID_SUFFIXES:
        allowed = ', '.join(sorted(VALID_SUFFIXES))
        raise ValueError(
            f'proposal must be a markdown file ({allowed}): {proposal_path}'
        )
    return proposal_path


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        proposal_path = validate_proposal_path(args.proposal_path)
    except ValueError as exc:
        print(f'error: {exc}', file=sys.stderr)
        return 1

    print(next_report_path(proposal_path))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
