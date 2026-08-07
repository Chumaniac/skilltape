#!/usr/bin/env python3
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"


class ReleaseWorkflowTests(unittest.TestCase):
    def test_release_workflow_has_locked_builds_all_targets_and_a_single_write_job(self) -> None:
        self.assertTrue(WORKFLOW.is_file(), "release workflow is missing")
        workflow = WORKFLOW.read_text(encoding="utf-8")
        required_fragments = [
            "tags:",
            "v*",
            "workflow_dispatch:",
            "version:",
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
            "cargo build --locked --release",
            "npm ci --prefix apps/skilltape-console",
            "actions/upload-artifact@v4",
            "sha256sum",
            "GITHUB_TOKEN",
            "contents: read",
            "contents: write",
        ]
        for fragment in required_fragments:
            self.assertIn(fragment, workflow, f"missing workflow fragment: {fragment}")
        self.assertEqual(workflow.count("contents: write"), 1)
        self.assertNotIn("actions/upload-artifact@v4", workflow.split("publish:", 1)[-1])


if __name__ == "__main__":
    unittest.main()
