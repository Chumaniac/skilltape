#!/usr/bin/env python3
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PACKAGER = ROOT / "scripts" / "package_release.py"


class ReleasePackageTests(unittest.TestCase):
    def make_fixture(self, root: Path, target: str) -> tuple[Path, Path, Path]:
        binary_dir = root / "target" / target / "release"
        ui_dist = root / "ui-dist"
        output_dir = root / "dist"
        binary_dir.mkdir(parents=True)
        (ui_dist / "assets").mkdir(parents=True)
        output_dir.mkdir()
        suffix = ".exe" if "windows" in target else ""
        (binary_dir / f"skilltape{suffix}").write_bytes(b"skilltape binary")
        (binary_dir / f"skilltape-console-api{suffix}").write_bytes(b"api binary")
        (ui_dist / "index.html").write_text("<main>Console</main>", encoding="utf-8")
        (ui_dist / "assets" / "app.js").write_text("console.log('ok')", encoding="utf-8")
        return binary_dir, ui_dist, output_dir

    def run_packager(
        self, version: str, target: str, binary_dir: Path, ui_dist: Path, output_dir: Path
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(PACKAGER),
                "--version",
                version,
                "--target",
                target,
                "--binary-dir",
                str(binary_dir),
                "--ui-dist",
                str(ui_dist),
                "--output-dir",
                str(output_dir),
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_unix_archive_contains_the_complete_console_layout(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary_dir, ui_dist, output_dir = self.make_fixture(root, "x86_64-apple-darwin")

            result = self.run_packager(
                "0.1.0", "x86_64-apple-darwin", binary_dir, ui_dist, output_dir
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            archive = output_dir / "skilltape-v0.1.0-x86_64-apple-darwin.tar.gz"
            self.assertTrue(archive.is_file())
            with tarfile.open(archive, "r:gz") as package:
                names = set(package.getnames())
            self.assertIn(
                "skilltape-v0.1.0-x86_64-apple-darwin/skilltape",
                names,
            )
            self.assertIn(
                "skilltape-v0.1.0-x86_64-apple-darwin/skilltape-console-api",
                names,
            )
            self.assertIn(
                "skilltape-v0.1.0-x86_64-apple-darwin/console/index.html",
                names,
            )
            self.assertIn(
                "skilltape-v0.1.0-x86_64-apple-darwin/console/assets/app.js",
                names,
            )

    def test_windows_archive_uses_executable_names_and_zip(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary_dir, ui_dist, output_dir = self.make_fixture(root, "x86_64-pc-windows-msvc")

            result = self.run_packager(
                "0.1.0", "x86_64-pc-windows-msvc", binary_dir, ui_dist, output_dir
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            archive = output_dir / "skilltape-v0.1.0-x86_64-pc-windows-msvc.zip"
            self.assertTrue(archive.is_file())
            with zipfile.ZipFile(archive) as package:
                names = set(package.namelist())
            self.assertIn(
                "skilltape-v0.1.0-x86_64-pc-windows-msvc/skilltape.exe",
                names,
            )
            self.assertIn(
                "skilltape-v0.1.0-x86_64-pc-windows-msvc/skilltape-console-api.exe",
                names,
            )
            self.assertIn(
                "skilltape-v0.1.0-x86_64-pc-windows-msvc/console/index.html",
                names,
            )

    def test_missing_api_binary_fails_without_creating_an_archive(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary_dir, ui_dist, output_dir = self.make_fixture(root, "x86_64-apple-darwin")
            (binary_dir / "skilltape-console-api").unlink()

            result = self.run_packager(
                "0.1.0", "x86_64-apple-darwin", binary_dir, ui_dist, output_dir
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(list(output_dir.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
