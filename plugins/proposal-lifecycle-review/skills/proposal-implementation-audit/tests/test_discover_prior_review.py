from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "discover_prior_review.py"


class DiscoverPriorReviewScriptTests(unittest.TestCase):
    def test_finds_sidecar_review_artifact_and_reviewer_ids(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            proposal = root / "my_proposal.md"
            proposal.write_text("# Proposal\n", encoding="utf-8")
            sidecar = root / "my_proposal.review"
            sidecar.mkdir()
            (sidecar / "final-review.md").write_text(
                "## Selected reviewers\n- rust_arch_reviewer\n- api_contract_reviewer\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(proposal)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(len(payload["artifacts"]), 1)
        self.assertEqual(payload["artifacts"][0]["type"], "final-review")
        self.assertIn("rust_arch_reviewer", payload["artifacts"][0]["detected_reviewer_ids"])
        self.assertIn("api_contract_reviewer", payload["artifacts"][0]["detected_reviewer_ids"])

    def test_ignores_prior_implementation_audit_reports(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            proposal = root / "my_proposal.md"
            proposal.write_text("# Proposal\n", encoding="utf-8")
            (root / "my_proposal_IMPLEMENTATION_AUDIT_R1.md").write_text(
                "Selected reviewers: go_service_arch_reviewer\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(proposal)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 0, result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["artifacts"], [])

    def test_rejects_non_markdown(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            proposal = Path(tmpdir) / "proposal.txt"
            proposal.write_text("proposal", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(proposal)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 1)
        self.assertIn("proposal must be a markdown file", result.stderr)


if __name__ == "__main__":
    unittest.main()
