import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
README = ROOT / "README.md"
DOCS_INDEX = ROOT / "docs" / "README.md"
GUIDES_INDEX = ROOT / "docs" / "guides" / "README.md"
QUICKSTART = ROOT / "docs" / "guides" / "quickstart.md"
INSTALLATION = ROOT / "docs" / "guides" / "installation.md"
TRANSCRIPT = ROOT / "docs" / "assets" / "quickstart-terminal.txt"
VISUAL = ROOT / "docs" / "assets" / "quickstart-terminal.svg"
PUBLIC_DOCUMENTS = (
    README,
    DOCS_INDEX,
    GUIDES_INDEX,
    QUICKSTART,
    INSTALLATION,
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
INSTALLER_SOURCE_COMMIT = "beb0bba1870e20e03e5bc80a2d9234c04fc1c6f6"
UNIX_INSTALLER_SOURCE = (
    "https://raw.githubusercontent.com/Chumaniac/skilltape/"
    f"{INSTALLER_SOURCE_COMMIT}/scripts/install.sh"
)
WINDOWS_INSTALLER_SOURCE = (
    "https://raw.githubusercontent.com/Chumaniac/skilltape/"
    f"{INSTALLER_SOURCE_COMMIT}/scripts/install.ps1"
)
CI_STEP_NAME = "Validate release and installer fixtures"
CI_QUICKSTART_COMMANDS = (
    "python3 scripts/test_user_documentation.py",
    "cargo build --locked -p skilltape-cli",
    "bash scripts/test_quickstart.sh target/debug/skilltape",
)


def named_yaml_literal_run(workflow: str, step_name: str) -> str:
    """Return one named YAML step's literal ``run: |`` block without PyYAML."""
    lines = workflow.splitlines()
    step_pattern = re.compile(r"^(?P<indent>\s*)- name: " + re.escape(step_name) + r"\s*$")

    for index, line in enumerate(lines):
        match = step_pattern.match(line)
        if not match:
            continue

        step_indent = len(match.group("indent"))
        run_indent = None
        run_lines = []
        for candidate in lines[index + 1 :]:
            indent = len(candidate) - len(candidate.lstrip())
            if indent == step_indent and candidate.lstrip().startswith("- "):
                break
            if run_indent is None:
                if candidate == " " * (step_indent + 2) + "run: |":
                    run_indent = indent
                continue
            if candidate and indent <= run_indent:
                break
            run_lines.append(candidate[run_indent + 2 :])

        if run_indent is not None:
            return "\n".join(run_lines)

    raise AssertionError(f"missing literal run block for CI step: {step_name}")


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

    def test_readme_primary_navigation_includes_optional_configuration(self):
        text = README.read_text(encoding="utf-8")
        destinations = {
            destination.split("#", 1)[0].strip().strip("<>")
            for destination in LINK_RE.findall(text)
        }
        self.assertIn("docs/guides/configuration.md", destinations)
        self.assertIn("[Configuration](docs/guides/configuration.md)", text)

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

    def test_native_release_update_examples_download_pinned_installers(self):
        text = INSTALLATION.read_text(encoding="utf-8")
        for source in (UNIX_INSTALLER_SOURCE, WINDOWS_INSTALLER_SOURCE):
            self.assertIn(source, text)
        self.assertNotIn("./scripts/install.sh", text)
        self.assertNotIn(r".\scripts\install.ps1", text)
        unix_download = (
            'curl --fail --location --silent --show-error --output "$installer_path"'
        )
        self.assertEqual(2, text.count('installer_path="$(mktemp)"'))
        self.assertEqual(2, text.count(unix_download))
        self.assertEqual(2, text.count('bash "$installer_path"'))
        self.assertIn(
            'bash "$installer_path" 0.1.0 "$HOME/.local/bin" "aarch64-apple-darwin"',
            text,
        )
        self.assertIn(
            f'Invoke-WebRequest -Uri "{WINDOWS_INSTALLER_SOURCE}" '
            "-OutFile $installerPath",
            text,
        )
        self.assertIn("& $installerPath", text)
        self.assertNotRegex(
            text, r"\|\s*(?:\S*/)?(?:bash|sh|pwsh|powershell)\b"
        )
        for value in (
            '"https://github.com/Chumaniac/skilltape/releases/download"',
            '"0.1.0"',
            '"$env:LOCALAPPDATA\\SkillTape\\bin"',
            '"x86_64-pc-windows-msvc"',
        ):
            self.assertIn(value, text)

    def test_ci_runs_user_documentation_and_quickstart_gates_in_named_step(self):
        workflow = (ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        run = named_yaml_literal_run(workflow, CI_STEP_NAME)
        release_workflow_test = "python3 scripts/test_release_workflow.py"
        self.assertIn(release_workflow_test, run)
        previous_position = run.index(release_workflow_test)
        for command in CI_QUICKSTART_COMMANDS:
            self.assertEqual(1, workflow.count(command))
            self.assertIn(command, run)
            command_position = run.index(command)
            self.assertGreater(command_position, previous_position)
            previous_position = command_position

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
