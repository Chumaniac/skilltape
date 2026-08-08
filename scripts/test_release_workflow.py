#!/usr/bin/env python3
import copy
import re
import tempfile
import unittest
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "release.yml"
WORKFLOW_DIRECTORY = ROOT / ".github" / "workflows"
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


def workflow_paths(directory: Path = WORKFLOW_DIRECTORY) -> tuple[Path, ...]:
    return tuple(sorted((*directory.glob("*.yml"), *directory.glob("*.yaml"))))


def load_workflow(path: Path) -> object:
    try:
        return yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as error:
        raise AssertionError(f"invalid workflow YAML: {path}") from error


def parsed_action_references(node: object) -> tuple[str, ...]:
    references: list[str] = []

    def visit(value: object) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                if key == "uses":
                    if not isinstance(child, str):
                        raise AssertionError("workflow uses value must be a string")
                    references.append(child)
                visit(child)
        elif isinstance(value, list):
            for child in value:
                visit(child)

    visit(node)
    return tuple(references)


def assert_actions_are_immutable(workflow_files: tuple[Path, ...]) -> None:
    for workflow_path in workflow_files:
        for action_reference in parsed_action_references(load_workflow(workflow_path)):
            if not re.fullmatch(r"[^@\s]+@[0-9a-f]{40}", action_reference):
                raise AssertionError(
                    f"action is not SHA-pinned: {workflow_path}: {action_reference}"
                )


def release_build_mapping(workflow: object) -> dict[str, object]:
    if not isinstance(workflow, dict):
        raise AssertionError("release workflow root must be a mapping")
    jobs = workflow.get("jobs")
    if not isinstance(jobs, dict):
        raise AssertionError("release workflow is missing jobs mapping")
    build = jobs.get("build")
    if not isinstance(build, dict):
        raise AssertionError("release workflow is missing build job")
    return build


def named_step_mapping(build: dict[str, object], name: str) -> dict[str, object]:
    steps = build.get("steps")
    if not isinstance(steps, list):
        raise AssertionError("release workflow build job is missing steps")
    for step in steps:
        if isinstance(step, dict) and step.get("name") == name:
            return step
    raise AssertionError(f"release workflow is missing step: {name}")


def assert_step_value(step: dict[str, object], key: str, expected: str) -> None:
    value: object = step
    for component in key.split("."):
        if not isinstance(value, dict) or component not in value:
            raise AssertionError(f"step is missing {key}")
        value = value[component]
    if value != expected:
        raise AssertionError(f"step {key} must be {expected!r}")


def assert_missing_step_value(step: dict[str, object], key: str) -> None:
    value: object = step
    for component in key.split("."):
        if not isinstance(value, dict) or component not in value:
            return
        value = value[component]
    raise AssertionError(f"step must not contain {key}")


def assert_release_attestations(build: dict[str, object]) -> None:
    provenance = named_step_mapping(build, "Attest release archive provenance")
    assert_step_value(
        provenance, "uses", "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6"
    )
    assert_step_value(
        provenance, "with.subject-path", "${{ steps.package.outputs.archive }}"
    )
    assert_missing_step_value(provenance, "with.sbom-path")

    sbom = named_step_mapping(build, "Attest release archive SBOM")
    assert_step_value(
        sbom, "uses", "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6"
    )
    assert_step_value(sbom, "with.subject-path", "${{ steps.package.outputs.archive }}")
    assert_step_value(sbom, "with.sbom-path", "${{ steps.package.outputs.sbom }}")


def job_block(workflow: str, job_name: str) -> str:
    start = re.search(rf"^  {re.escape(job_name)}:\n", workflow, re.MULTILINE)
    if start is None:
        raise AssertionError(f"release workflow is missing job: {job_name}")
    remainder = workflow[start.end() :]
    end = re.search(r"^  [A-Za-z0-9_-]+:\n", remainder, re.MULTILINE)
    return remainder[: end.start()] if end is not None else remainder


def job_needs(job: str) -> set[str]:
    match = re.search(r"^    needs: (?P<value>.+)$", job, re.MULTILINE)
    if match is None:
        return set()
    required_jobs = match.group("value").strip()
    if isinstance(required_jobs, str):
        if required_jobs.startswith("[") and required_jobs.endswith("]"):
            return {item.strip() for item in required_jobs[1:-1].split(",")}
        return {required_jobs}
    raise AssertionError("workflow needs value is not a string")


def job_steps(job: str) -> tuple[tuple[str, str], ...]:
    return tuple(
        (match.group("name"), match.group("body"))
        for match in re.finditer(
            r"^      - name: (?P<name>[^\n]+)\n(?P<body>.*?)(?=^      - |\Z)",
            job,
            re.MULTILINE | re.DOTALL,
        )
    )


def named_step(steps: tuple[tuple[str, str], ...], name: str) -> str:
    for step_name, step_body in steps:
        if step_name == name:
            return step_body
    raise AssertionError(f"release workflow is missing step: {name}")


def step_value(step: str, key: str) -> str:
    match = re.search(rf"^        {re.escape(key)}: (?P<value>.+)$", step, re.MULTILINE)
    if match is None:
        raise AssertionError(f"step is missing {key}")
    return match.group("value")


def with_value(step: str, key: str) -> str:
    match = re.search(rf"^          {re.escape(key)}: (?P<value>.+)$", step, re.MULTILINE)
    if match is None:
        raise AssertionError(f"step is missing with.{key}")
    return match.group("value")


def run_script(step: str) -> str:
    match = re.search(
        r"^        run: \|\n(?P<script>(?:^          .*\n?)*)", step, re.MULTILINE
    )
    if match is None:
        raise AssertionError("step is missing a shell script")
    return match.group("script")


class ReleaseWorkflowTests(unittest.TestCase):
    def test_workflow_actions_are_full_sha_pinned_with_version_comments(self) -> None:
        # This catches a workflow-action reference being changed from an immutable
        # commit SHA to a mutable tag or branch.
        workflow_files = workflow_paths()
        self.assertTrue(workflow_files, "workflow directory is empty")
        assert_actions_are_immutable(workflow_files)

        all_workflows = "\n".join(
            workflow_path.read_text(encoding="utf-8") for workflow_path in workflow_files
        )
        for action_reference in EXPECTED_ACTION_REFERENCES:
            self.assertIn(action_reference, all_workflows)

    def test_action_pinning_ignores_scalar_text_and_rejects_mutable_references(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            (directory / "empty.yml").write_text("name: Empty\n", encoding="utf-8")
            (directory / "valid.yaml").write_text(
                """name: Valid
jobs:
  test:
    steps:
      - name: Shell text is not an action
        run: |
          echo "uses: actions/checkout@v7"
          # uses: actions/checkout@v7
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - { uses: \"actions/setup-node@820762786026740c76f36085b0efc47a31fe5020\" } # v7
# uses: actions/checkout@v7
note: "uses: actions/checkout@v7"
continued_note: "first line
  uses: actions/checkout@v7"
continued_single_note: 'first line
  uses: actions/checkout@v7'
quoted_note: 'fake "uses": actions/checkout@v7'
""",
                encoding="utf-8",
            )
            discovered = workflow_paths(directory)
            self.assertEqual({path.name for path in discovered}, {"empty.yml", "valid.yaml"})
            assert_actions_are_immutable(discovered)

            mutable = directory / "mutable.yaml"
            mutable.write_text(
                "- uses: actions/checkout@v7 # v7\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(AssertionError, "action is not SHA-pinned"):
                assert_actions_are_immutable(workflow_paths(directory))

    def test_action_pinning_rejects_mutable_quoted_mapping_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            cases = {
                "normal.yaml": "- 'uses': actions/checkout@v7 # v7\n",
                "flow.yaml": 'steps: [{ "uses": actions/checkout@v7, name: checkout }]\n',
            }
            for filename, workflow in cases.items():
                with self.subTest(filename=filename):
                    path = directory / filename
                    path.write_text(workflow, encoding="utf-8")
                    with self.assertRaisesRegex(
                        AssertionError, "action is not SHA-pinned"
                    ):
                        assert_actions_are_immutable((path,))

    def test_action_pinning_parses_quoted_nested_and_flow_uses_mappings(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            valid = directory / "valid.yml"
            valid.write_text(
                """note: '\"uses\": actions/checkout@v7'
run: |
  echo 'uses: actions/checkout@v7'
steps:
  - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
""",
                encoding="utf-8",
            )
            assert_actions_are_immutable((valid,))

            cases = {
                "normal-quoted.yml": """jobs:
  test:
    steps:
      - 'uses': actions/checkout@v7
""",
                "flow-quoted.yaml": 'steps: [{ "uses": actions/checkout@v7 }]\n',
                "nested-quoted.yaml": """jobs:
  test:
    steps:
      - nested:
          - uses: actions/checkout@v7
""",
                "explicit-quoted-key.yaml": """? 'uses'
: actions/checkout@v7
""",
            }
            for filename, workflow in cases.items():
                with self.subTest(filename=filename):
                    path = directory / filename
                    path.write_text(workflow, encoding="utf-8")
                    with self.assertRaisesRegex(
                        AssertionError, "action is not SHA-pinned"
                    ):
                        assert_actions_are_immutable((path,))

    def test_action_pinning_rejects_invalid_workflow_yaml(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "invalid.yaml"
            path.write_text("jobs: [\n", encoding="utf-8")
            with self.assertRaisesRegex(
                AssertionError, rf"invalid workflow YAML: {re.escape(str(path))}"
            ):
                assert_actions_are_immutable((path,))

    def test_action_pinning_rejects_mutable_flow_sequence_mapping(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            (directory / "flow.yaml").write_text(
                "steps: [{ uses: actions/checkout@v7, name: checkout }]\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(AssertionError, "action is not SHA-pinned"):
                assert_actions_are_immutable(workflow_paths(directory))

    def test_action_pinning_rejects_mutable_flow_mapping_after_plain_hash_scalar(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "flow.yml"
            path.write_text(
                "steps: [{ name: foo#bar, uses: actions/checkout@v7 }]\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(AssertionError, "action is not SHA-pinned"):
                assert_actions_are_immutable((path,))

    def test_action_pinning_ignores_explicit_indentation_block_scalar(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "scalar.yaml"
            path.write_text(
                "steps:\n  - run: |2\n      uses: actions/checkout@v7\n",
                encoding="utf-8",
            )
            assert_actions_are_immutable((path,))

    def test_release_workflow_enforces_immutable_release_relationships(self) -> None:
        self.assertTrue(WORKFLOW.is_file(), "release workflow is missing")
        workflow = WORKFLOW.read_text(encoding="utf-8")
        assert_release_attestations(release_build_mapping(load_workflow(WORKFLOW)))
        for fragment in (
            "tags:\n      - 'v*'",
            "workflow_dispatch:\n    inputs:\n      version:",
            "x86_64-unknown-linux-gnu",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
            "x86_64-pc-windows-msvc",
        ):
            self.assertIn(fragment, workflow)
        prepare = job_block(workflow, "prepare")
        build = job_block(workflow, "build")
        publish = job_block(workflow, "publish")
        windows_installer = job_block(workflow, "windows-installer")
        self.assertEqual(job_needs(build), {"prepare"})
        self.assertEqual(job_needs(publish), {"prepare", "build"})
        self.assertEqual(job_needs(windows_installer), {"prepare", "publish"})

        prepare_validation = run_script(
            named_step(job_steps(prepare), "Resolve and validate release tag")
        )
        self.assertIn('if [[ "$tag" == "v0.1.0" ]]', prepare_validation)
        self.assertIn('git ls-remote --exit-code origin "refs/tags/$tag"', prepare_validation)
        self.assertIn("manual release must be dispatched from the matching v<version> tag", prepare_validation)
        self.assertIn('[[ "$GITHUB_REF" != "refs/tags/$tag" ]]', prepare_validation)
        self.assertIn('"$version" == "." || "$version" == ".."', prepare_validation)
        self.assertIn("id-token: write", build)
        self.assertIn("attestations: write", build)

        publish_steps = job_steps(publish)
        self.assertNotIn(
            "Revalidate release tag before publishing",
            {step_name for step_name, _ in publish_steps},
        )
        for step_name, step_body in publish_steps:
            if step_name != "Publish GitHub release":
                self.assertNotIn("git ls-remote", step_body)
        publish_script = run_script(named_step(publish_steps, "Publish GitHub release"))
        for fragment in (
            'tag="${{ needs.prepare.outputs.tag }}"',
            'git ls-remote --exit-code origin "refs/tags/$tag"',
            'peeled_ref="refs/tags/$tag^{}"',
            'git ls-remote origin "$direct_ref" "$peeled_ref"',
            'git ls-remote origin "$direct_ref"',
            '"$tag_commit" != "$GITHUB_SHA"',
        ):
            self.assertIn(fragment, publish_script)
        self.assertEqual(publish_script.count("verify_release_tag"), 3)
        self.assertRegex(
            publish_script,
            r'if gh release view "\$tag" --repo "\$GITHUB_REPOSITORY".*?then\n'
            r'\s+verify_release_tag\n'
            r'\s+gh release upload "\$tag" --repo "\$GITHUB_REPOSITORY"',
            "existing-release publication must revalidate immediately before upload",
        )
        self.assertRegex(
            publish_script,
            r'else\n'
            r'\s+verify_release_tag\n'
            r'\s+gh release create "\$tag" --repo "\$GITHUB_REPOSITORY" --verify-tag',
            "new-release publication must revalidate immediately before creation",
        )
        self.assertIn('gh release view "$tag" --repo "$GITHUB_REPOSITORY"', publish_script)

        checksums = run_script(named_step(publish_steps, "Generate checksums"))
        self.assertIn('sha256sum "${archives[@]}" | sort > checksums.txt', checksums)

        build_steps = job_steps(build)
        self.assertIn(
            'cargo build --locked --release --target "${{ matrix.target }}"', build
        )
        self.assertIn("npm ci --prefix apps/skilltape-console", build)
        package = named_step(build_steps, "Package release archive")
        self.assertEqual(step_value(package, "id"), "package")
        package_script = run_script(package)
        self.assertIn('echo "archive=$archive" >> "$GITHUB_OUTPUT"', package_script)
        self.assertIn('echo "sbom=$archive.spdx.json" >> "$GITHUB_OUTPUT"', package_script)
        sbom = next(
            step_body
            for _, step_body in build_steps
            if re.search(r"^        uses: anchore/sbom-action@", step_body, re.MULTILINE)
        )
        self.assertEqual(with_value(sbom, "file"), "${{ steps.package.outputs.archive }}")
        self.assertEqual(with_value(sbom, "output-file"), "${{ steps.package.outputs.sbom }}")
        self.assertEqual(with_value(sbom, "format"), "spdx-json")
        self.assertEqual(with_value(sbom, "upload-artifact"), "false")
        self.assertEqual(with_value(sbom, "upload-release-assets"), "false")
        release_upload = named_step(build_steps, "Upload release artifact")
        self.assertEqual(with_value(release_upload, "path"), "release/*")

    def test_release_attestations_reject_contract_mutations(self) -> None:
        workflow = load_workflow(WORKFLOW)
        build = release_build_mapping(workflow)
        assert_release_attestations(build)

        missing_provenance_uses = copy.deepcopy(build)
        named_step_mapping(
            missing_provenance_uses, "Attest release archive provenance"
        ).pop("uses")
        with self.assertRaisesRegex(AssertionError, "step is missing uses"):
            assert_release_attestations(missing_provenance_uses)

        retagged_provenance = copy.deepcopy(build)
        named_step_mapping(retagged_provenance, "Attest release archive provenance")[
            "uses"
        ] = "evil/actions/attest@0123456789abcdef0123456789abcdef01234567"
        with self.assertRaisesRegex(AssertionError, "step uses must be"):
            assert_release_attestations(retagged_provenance)

        provenance_with_sbom = copy.deepcopy(build)
        provenance = named_step_mapping(
            provenance_with_sbom, "Attest release archive provenance"
        )
        provenance_with = provenance.get("with")
        self.assertIsInstance(provenance_with, dict)
        provenance_with["sbom-path"] = "${{ steps.package.outputs.sbom }}"
        with self.assertRaisesRegex(
            AssertionError, "step must not contain with.sbom-path"
        ):
            assert_release_attestations(provenance_with_sbom)

        missing_sbom_step = copy.deepcopy(build)
        steps = missing_sbom_step.get("steps")
        self.assertIsInstance(steps, list)
        missing_sbom_step["steps"] = [
            step
            for step in steps
            if not isinstance(step, dict)
            or step.get("name") != "Attest release archive SBOM"
        ]
        with self.assertRaisesRegex(
            AssertionError, "release workflow is missing step: Attest release archive SBOM"
        ):
            assert_release_attestations(missing_sbom_step)

        changed_sbom_path = copy.deepcopy(build)
        sbom_with = named_step_mapping(
            changed_sbom_path, "Attest release archive SBOM"
        ).get("with")
        self.assertIsInstance(sbom_with, dict)
        sbom_with["sbom-path"] = "release/archive.spdx.json"
        with self.assertRaisesRegex(AssertionError, "step with.sbom-path must be"):
            assert_release_attestations(changed_sbom_path)

    def test_release_attestations_reject_quoted_provenance_sbom_path(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "quoted-provenance-sbom.yml"
            path.write_text(
                """jobs:
  build:
    steps:
      - name: Attest release archive provenance
        uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6
        with:
          subject-path: ${{ steps.package.outputs.archive }}
          "sbom-path": ${{ steps.package.outputs.sbom }}
      - name: Attest release archive SBOM
        uses: actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6
        with:
          subject-path: ${{ steps.package.outputs.archive }}
          sbom-path: ${{ steps.package.outputs.sbom }}
""",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(
                AssertionError, "step must not contain with.sbom-path"
            ):
                assert_release_attestations(release_build_mapping(load_workflow(path)))

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
