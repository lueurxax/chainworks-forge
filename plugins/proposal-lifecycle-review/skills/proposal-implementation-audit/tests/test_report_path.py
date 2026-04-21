from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "scripts" / "report_path.py"


class ReportPathScriptTests(unittest.TestCase):
    def test_rejects_directory_path(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            result = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), tmpdir],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 1)
        self.assertIn("proposal is not a file", result.stderr)

    def test_increments_revision_from_existing_sibling_reports(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tmp_path = Path(tmpdir)
            proposal = tmp_path / "my_proposal.md"
            proposal.write_text("# Proposal\n", encoding="utf-8")
            (tmp_path / "my_proposal_IMPLEMENTATION_AUDIT_R1.md").write_text("", encoding="utf-8")
            (tmp_path / "my_proposal_IMPLEMENTATION_AUDIT_R3.md").write_text("", encoding="utf-8")

            result = subprocess.run(
                [sys.executable, str(SCRIPT_PATH), str(proposal)],
                capture_output=True,
                text=True,
                check=False,
            )

        self.assertEqual(result.returncode, 0)
        self.assertTrue(result.stdout.strip().endswith("my_proposal_IMPLEMENTATION_AUDIT_R4.md"))


if __name__ == "__main__":
    unittest.main()
