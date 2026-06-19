import json, glob, sys, os
from pathlib import Path

run_dir = Path("/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44")
review_files = [
    run_dir / "reviews/proposal/api-contract.json",
    run_dir / "reviews/proposal/apple-architect.json",
    run_dir / "reviews/proposal/macos.json",
    run_dir / "reviews/proposal/reliability.json",
    run_dir / "reviews/proposal/security.json"
]

all_reviews = []
for f in review_files:
    if f.exists():
        with open(f) as fd:
            all_reviews.append(json.load(fd))

blocking_issues = []
blocking_required_changes = []
advisory_follow_ups = []
recurring_themes = []

scores = []
for r in all_reviews:
    score = float(r.get('score', 0))
    if score <= 10:
        score = score * 10
    scores.append(score)

    if r.get('blocking_issues'):
        for i in r['blocking_issues']:
            blocking_issues.append(f"[{r.get('role', 'reviewer')}] {i}")
            blocking_required_changes.append(f"[{r.get('role', 'reviewer')}] {i}")

    if r.get('non_blocking_issues'):
        for i in r['non_blocking_issues']:
            advisory_follow_ups.append(f"[{r.get('role', 'reviewer')}] {i}")

    if r.get('suggestions'):
        for s in r['suggestions']:
            advisory_follow_ups.append(f"[{r.get('role', 'reviewer')}] Suggestion: {s}")

avg_score = sum(scores) / len(scores) if scores else 0
min_score = min(scores) if scores else 0
is_pass = len(blocking_issues) == 0

out = {
    "pass": is_pass,
    "average_score": avg_score,
    "aggregate_score": avg_score,
    "min_individual_score": min_score,
    "blocker_count": len(blocking_issues),
    "blocking_issues": blocking_issues,
    "summary": "Reviews have been aggregated. There are some blocking issues in API and reliability.",
    "blocking_required_changes": blocking_required_changes,
    "advisory_follow_ups": advisory_follow_ups,
    "recurring_themes": recurring_themes,
    "decision": "revise" if not is_pass else "approve"
}

with open(run_dir / "reviews/proposal/summary.json", "w") as fd:
    json.dump(out, fd, indent=2)

print(f"Scores: {scores}")
print(f"Pass: {is_pass}, Blockers: {len(blocking_issues)}")
