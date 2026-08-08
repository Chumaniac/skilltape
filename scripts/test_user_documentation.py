import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
README = ROOT / "README.md"
DOCS_INDEX = ROOT / "docs" / "README.md"
GUIDES_INDEX = ROOT / "docs" / "guides" / "README.md"
QUICKSTART = ROOT / "docs" / "guides" / "quickstart.md"
TRANSCRIPT = ROOT / "docs" / "assets" / "quickstart-terminal.txt"
VISUAL = ROOT / "docs" / "assets" / "quickstart-terminal.svg"
PUBLIC_DOCUMENTS = (
    README,
    DOCS_INDEX,
    GUIDES_INDEX,
    QUICKSTART,
    ROOT / "docs" / "guides" / "installation.md",
    ROOT / "docs" / "guides" / "configuration.md",
)

PRIMARY_PROMISE = (
    "Turn a real local workflow into a reviewable Agent Skill you can replay "
    "and verify before you share it."
)
RETIRED_README_HEADINGS = (
    "## CI and Skill repository integration",
    "## Security, compatibility, and benchmarks",
    "## Design goals",
)

LINK_RE = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")


class UserDocumentationContractTests(unittest.TestCase):
    def test_readme_has_a_value_statement_beta_and_user_entry_links(self):
        text = README.read_text(encoding="utf-8")
        self.assertIn(PRIMARY_PROMISE, text)
        self.assertIn("**Beta**", text)
        self.assertIn("docs/assets/quickstart-terminal.svg", text)
        self.assertIn("docs/assets/quickstart-terminal.txt", text)
        self.assertIn("docs/guides/quickstart.md", text)
        self.assertIn(
            "https://github.com/Chumaniac/skilltape/releases/tag/v0.1.0", text
        )
        destinations = {
            destination.split("#", 1)[0].strip().strip("<>")
            for destination in LINK_RE.findall(text)
        }
        self.assertIn("docs/assets/quickstart-terminal.svg", destinations)
        self.assertIn("docs/assets/quickstart-terminal.txt", destinations)

    def test_visual_demo_svg_is_accessible(self):
        self.assertTrue(VISUAL.is_file())
        visual = VISUAL.read_text(encoding="utf-8")
        for fragment in ("<title>", "<desc>"):
            self.assertIn(fragment, visual)

    def test_visual_demo_transcript_has_commands_results_and_redaction(self):
        self.assertTrue(TRANSCRIPT.is_file())
        transcript = TRANSCRIPT.read_text(encoding="utf-8")
        for fragment in (
            "skilltape capture demo",
            "skilltape compile",
            "skilltape lint",
            "skilltape verify",
            '"ok":true',
            "Compiled skill at",
            '"errors":[]',
            '"status":"succeeded"',
            "<workspace>",
        ):
            self.assertIn(fragment, transcript)
        self.assertNotIn("/tmp/", transcript)

    def test_readme_does_not_promote_retired_implementation_sections(self):
        text = README.read_text(encoding="utf-8")
        for heading in RETIRED_README_HEADINGS:
            self.assertNotIn(heading, text)

    def test_quickstart_has_a_unix_first_result_and_safe_boundary(self):
        self.assertTrue(QUICKSTART.is_file())
        text = QUICKSTART.read_text(encoding="utf-8")
        for fragment in (
            "## macOS and Linux",
            "bwrap",
            "sandbox-exec",
            "beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6",
            "installation.md",
            "configuration.md",
            "whoami.exe",
            "Replay/Verify fail closed",
        ):
            self.assertIn(fragment, text)

    def test_user_indexes_route_to_tasks_not_internal_plans(self):
        required = (
            "guides/quickstart.md",
            "guides/installation.md",
            "guides/configuration.md",
            "../examples/minimal-skill/README.md",
            "../SECURITY.md",
            "../CONTRIBUTING.md",
        )
        combined = DOCS_INDEX.read_text(encoding="utf-8") + GUIDES_INDEX.read_text(encoding="utf-8")
        for fragment in required:
            self.assertIn(fragment, combined)
        for retired in ("superpowers/", "release-readiness.md", "CodeQL path-safety audit"):
            self.assertNotIn(retired, combined)

    def test_public_relative_markdown_links_resolve(self):
        for source in PUBLIC_DOCUMENTS:
            self.assertTrue(source.is_file())
            for destination in LINK_RE.findall(source.read_text(encoding="utf-8")):
                target = destination.split("#", 1)[0].strip().strip("<>")
                if not target or "://" in target or target.startswith("mailto:"):
                    continue
                self.assertTrue(
                    (source.parent / target).resolve().exists(),
                    f"{source.relative_to(ROOT)} links to missing {target}",
                )


if __name__ == "__main__":
    unittest.main()
