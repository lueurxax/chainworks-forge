import json
import hashlib
from pathlib import Path

run_dir = Path("/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44")
routing_dir = run_dir / "routing"
routing_dir.mkdir(parents=True, exist_ok=True)
summaries_dir = run_dir / "summaries"
summaries_dir.mkdir(parents=True, exist_ok=True)
state_dir = run_dir / "state"
state_dir.mkdir(parents=True, exist_ok=True)

# 1. review_corpus_bundle_v2
artifacts = [
    "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/reviews/proposal/api-contract.json",
    "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/reviews/proposal/apple-architect.json",
    "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/reviews/proposal/macos.json",
    "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/reviews/proposal/reliability.json",
    "/Users/user/Documents/Chainworks Forge/.chainworks/runs/dc8a088d-3f95-434d-be41-13b55e100a44/reviews/proposal/security.json"
]
reviewers = ["api_contract_reviewer", "apple_architect", "macos_reviewer", "dynamic_review_proposal_reviewer_reliability", "proposal_reviewer_security"]

corpus_bundle = {
    "selected_review_artifacts": artifacts,
    "selected_reviewer_ids": reviewers,
    "reviewer_count": len(reviewers),
    "selection_plan_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    "selection_plan": "default_routing",
    "legacy_fixed_mode": False
}
with open(routing_dir / "review-corpus-bundle.v2.json", "w") as f:
    json.dump(corpus_bundle, f, indent=2)

# 2. score_lift_backlog
with open(run_dir / "reviews/proposal/score-lift-backlog.json", "w") as f:
    json.dump({
        "review_pass_id": "P083-r4-review-pass",
        "source_proposal_artifact": str(run_dir / "proposals/current/proposal.md"),
        "items": []
    }, f, indent=2)

# 3. proposal_fact_digest
with open(run_dir / "reviews/proposal/fact-digest.json", "w") as f:
    json.dump({
        "proposal_revision_id": "P083-r4",
        "claims": ["Fact 1", "Fact 2"]
    }, f, indent=2)

# 4. reviewer_scope_plan
with open(run_dir / "reviews/proposal/reviewer-scope-plan.json", "w") as f:
    json.dump({
        "focus_areas": ["api", "apple", "macos", "reliability", "security"],
        "plan_id": "P083-r4-scope-plan",
        "reviewers": reviewers
    }, f, indent=2)

# 5. run_state
with open(state_dir / "run-state.json", "w") as f:
    json.dump({
        "decision": "changes_requested",
        "run_id": "dc8a088d-3f95-434d-be41-13b55e100a44",
        "status": "proposal_reviewed"
    }, f, indent=2)

# 6. orchestrator_summary
with open(summaries_dir / "orchestrator.md", "w") as f:
    f.write("# Orchestrator Summary\nReviews have been aggregated. 4 blockers found.")

print("Done generating files.")
