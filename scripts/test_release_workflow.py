#!/usr/bin/env python3
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
WORKFLOWS = tuple(sorted((ROOT / ".github" / "workflows").glob("*.yml")))
INSTALLER = ROOT / "scripts" / "install.ps1"

EXPECTED_ACTION_REFERENCES = (
    "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7",
    "actions/setup-node@820762786026740c76f36085b0efc47a31fe5020 # v7",
    "actions/setup-python@5fda3b95a4ea91299a34e894583c3862153e4b97 # v7",
    "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7",
    "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8",
    "dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772 # master",
    "anchore/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0",
    "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4",
)


class ReleaseWorkflowTests(unittest.TestCase):
    def test_workflow_actions_are_full_sha_pinned_with_version_comments(self) -> None:
        # This catches a workflow-action reference being changed from an immutable
        # commit SHA to a mutable tag or branch.
        for workflow_path in WORKFLOWS:
            workflow = workflow_path.read_text(encoding="utf-8")
            action_lines = [
                line.strip()
                for line in workflow.splitlines()
                if re.match(r"^\s*uses:\s+", line)
            ]
            self.assertTrue(action_lines, f"workflow has no actions: {workflow_path}")
            for action_line in action_lines:
                self.assertRegex(
                    action_line,
                    r"^uses:\s+[^\s@]+@[0-9a-f]{40}\s+#\s+[^\s#]+$",
                    f"action is not SHA-pinned with a version comment: {workflow_path}: {action_line}",
                )

        all_workflows = "\n".join(
            workflow_path.read_text(encoding="utf-8") for workflow_path in WORKFLOWS
        )
        for action_reference in EXPECTED_ACTION_REFERENCES:
            self.assertIn(action_reference, all_workflows)

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
            "prepare:",
            'git ls-remote --exit-code origin "refs/tags/$tag"',
            "manual release must be dispatched from the matching v<version> tag",
            '[[ "$GITHUB_REF" != "refs/tags/$tag" ]]',
            '"$version" == "." || "$version" == ".."',
            'gh release create "$tag" --repo "$GITHUB_REPOSITORY" --verify-tag',
            "anchore/sbom-action@",
            "actions/attest@",
            "attestations: write",
            "id-token: write",
            "sbom-path:",
            "sha256sum",
            "GITHUB_TOKEN",
            'gh release view "$tag" --repo "$GITHUB_REPOSITORY"',
            'gh release upload "$tag" --repo "$GITHUB_REPOSITORY"',
            "contents: read",
            "contents: write",
        ]
        for fragment in required_fragments:
            self.assertIn(fragment, workflow, f"missing workflow fragment: {fragment}")
        self.assertEqual(workflow.count("contents: write"), 1)

    def test_release_workflow_smokes_the_published_windows_installer(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        required_fragments = [
            "windows-installer:",
            "prepare",
            "publish",
            "runs-on: windows-latest",
            "scripts/install.ps1",
            "SKILLTAPE_RELEASE_BASE_URL",
            "SKILLTAPE_RELEASE_API_BASE_URL",
            "SKILLTAPE_RELEASE_TOKEN",
            "skilltape-console-api.exe",
            "console\\index.html",
            "init release-smoke",
        ]
        for fragment in required_fragments:
            self.assertIn(fragment, workflow, f"missing Windows installer fragment: {fragment}")

    def test_release_workflow_uses_a_supported_intel_macos_runner(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        self.assertIn("os: macos-15-intel", workflow)
        self.assertNotIn("os: macos-13", workflow)

    def test_powershell_installer_supports_authenticated_release_downloads(self) -> None:
        installer = INSTALLER.read_text(encoding="utf-8")
        self.assertIn("SKILLTAPE_RELEASE_TOKEN", installer)
        self.assertIn("SKILLTAPE_RELEASE_API_BASE_URL", installer)
        self.assertIn("releases/tags", installer)
        self.assertIn("$archiveAsset.url", installer)
        self.assertIn("$checksumsAsset.url", installer)
        self.assertIn("application/octet-stream", installer)
        self.assertIn("-replace '^\\./'", installer)
        self.assertIn("Authorization", installer)


if __name__ == "__main__":
    unittest.main()
