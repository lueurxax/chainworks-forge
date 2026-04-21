#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Iterable

VALID_SUFFIXES = {'.md', '.markdown'}
BUILTIN_REVIEWER_IDS = {
    'ios_ui_reviewer',
    'macos_ui_reviewer',
    'apple_ux_reviewer',
    'apple_arch_reviewer',
    'rust_arch_reviewer',
    'rust_reliability_reviewer',
    'rust_performance_reviewer',
    'rust_security_reviewer',
    'go_service_arch_reviewer',
    'go_reliability_reviewer',
    'go_performance_reviewer',
    'go_security_reviewer',
    'api_contract_reviewer',
    'observability_rollout_reviewer',
    'product_reviewer',
}

IGNORE_PATTERNS = (
    'IMPLEMENTATION_AUDIT',
    '__MACOSX',
)

ROOT_REVIEW_DIRS = (
    '.review',
    '.reviews',
    'docs/reviews',
    'docs/proposal-reviews',
)


def validate_proposal_path(raw_path: str) -> Path:
    proposal_path = Path(raw_path).expanduser().resolve()
    if not proposal_path.exists():
        raise ValueError(f'proposal not found: {proposal_path}')
    if not proposal_path.is_file():
        raise ValueError(f'proposal is not a file: {proposal_path}')
    if proposal_path.suffix.lower() not in VALID_SUFFIXES:
        allowed = ', '.join(sorted(VALID_SUFFIXES))
        raise ValueError(f'proposal must be a markdown file ({allowed}): {proposal_path}')
    return proposal_path


def find_repo_root(start: Path) -> Path:
    current = start.parent if start.is_file() else start
    for candidate in [current, *current.parents]:
        if (candidate / '.git').exists():
            return candidate
    return current


def iter_markdown_files(paths: Iterable[Path]) -> Iterable[Path]:
    seen: set[Path] = set()
    for path in paths:
        if not path.exists():
            continue
        if path.is_file():
            candidates = [path]
        else:
            candidates = [p for p in path.rglob('*') if p.is_file()]
        for candidate in candidates:
            if candidate in seen:
                continue
            seen.add(candidate)
            if candidate.suffix.lower() not in VALID_SUFFIXES:
                continue
            name_upper = candidate.name.upper()
            if any(pattern in name_upper for pattern in IGNORE_PATTERNS):
                continue
            yield candidate


def candidate_paths(proposal_path: Path) -> list[Path]:
    parent = proposal_path.parent
    stem = proposal_path.stem
    repo_root = find_repo_root(proposal_path)

    paths: list[Path] = []
    paths.extend([
        parent / f'{stem}.review',
        parent / f'{stem}_review',
        parent / f'{stem}-review',
        parent / '.review' / stem,
        parent / '.reviews' / stem,
    ])

    sibling_patterns = [
        f'{stem}*PROPOSAL*REVIEW*.md',
        f'{stem}*REVIEW*.md',
        f'{stem}*EVIDENCE*PACK*.md',
        f'{stem}*RESEARCH*PACK*.md',
        f'{stem}*ROUTING*.md',
    ]
    for pattern in sibling_patterns:
        paths.extend(parent.glob(pattern))

    for review_dir in ROOT_REVIEW_DIRS:
        base = repo_root / review_dir
        if base.exists():
            paths.extend(base.glob(f'**/{stem}*.md'))
            paths.extend(base.glob(f'**/{stem}/**/*.md'))

    return paths


def classify_artifact(path: Path, text: str) -> str:
    lower_name = path.name.lower()
    lower_text = text.lower()
    if 'evidence' in lower_name and 'pack' in lower_name:
        return 'evidence-pack'
    if 'research' in lower_name and 'pack' in lower_name:
        return 'research-pack'
    if 'reviewer' in lower_name and 'selection' in lower_name:
        return 'reviewer-selection'
    if 'selected reviewers' in lower_text or 'rejected close alternatives' in lower_text:
        return 'final-review'
    if 'evidence pack' in lower_text:
        return 'evidence-pack'
    if 'research pack' in lower_text:
        return 'research-pack'
    return 'review-artifact'


def detected_reviewers(text: str) -> list[str]:
    found = []
    for reviewer_id in sorted(BUILTIN_REVIEWER_IDS):
        if re.search(rf'(?<![A-Za-z0-9_]){re.escape(reviewer_id)}(?![A-Za-z0-9_])', text):
            found.append(reviewer_id)
    return found


def summarize_artifact(path: Path, proposal_path: Path) -> dict[str, object]:
    try:
        text = path.read_text(encoding='utf-8', errors='replace')
    except OSError:
        text = ''
    stat = path.stat()
    return {
        'path': str(path),
        'relative_to_proposal_dir': str(path.relative_to(proposal_path.parent)) if path.is_relative_to(proposal_path.parent) else None,
        'type': classify_artifact(path, text),
        'detected_reviewer_ids': detected_reviewers(text),
        'mtime': stat.st_mtime,
        'size_bytes': stat.st_size,
    }


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description='Discover prior proposal-review artifacts related to a proposal markdown file.'
    )
    parser.add_argument('proposal_path', help='Path to the proposal markdown file')
    parser.add_argument('--limit', type=int, default=20, help='Maximum artifacts to return')
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        proposal_path = validate_proposal_path(args.proposal_path)
    except ValueError as exc:
        print(f'error: {exc}', file=sys.stderr)
        return 1

    artifacts = [summarize_artifact(path, proposal_path) for path in iter_markdown_files(candidate_paths(proposal_path))]
    artifacts.sort(key=lambda item: (len(item['detected_reviewer_ids']), item['mtime']), reverse=True)
    if args.limit >= 0:
        artifacts = artifacts[: args.limit]

    print(json.dumps({
        'proposal_path': str(proposal_path),
        'repo_root': str(find_repo_root(proposal_path)),
        'artifacts': artifacts,
    }, indent=2, sort_keys=True))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
