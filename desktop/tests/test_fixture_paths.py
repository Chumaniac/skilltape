"""Refuse fixture output paths outside the caller's private disposable directory."""
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import unittest
import uuid

FIXTURE = Path(__file__).with_name("backend_fixture.py")


class FixturePathChecks(unittest.TestCase):
    def refuse(self, output):
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        result = subprocess.run([sys.executable, str(FIXTURE), str(port), str(output), "normal"],
                                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=3)
        self.assertNotEqual(result.returncode, 0)

    def test_arbitrary_directory_is_not_writable(self):
        with tempfile.TemporaryDirectory(prefix="fixture-path-refusal-") as directory:
            output = Path(directory) / (str(uuid.uuid4()) + ".json")
            self.refuse(output)
            self.assertFalse(output.exists())

    def test_existing_file_is_preserved(self):
        root = Path(tempfile.gettempdir()).resolve() / ("backend-check-" + str(uuid.uuid4()))
        root.mkdir(mode=0o700)
        output = root / (str(uuid.uuid4()) + ".json")
        try:
            output.write_text("retained synthetic fixture")
            self.refuse(output)
            self.assertEqual(output.read_text(), "retained synthetic fixture")
        finally:
            output.unlink(missing_ok=True)
            root.rmdir()

    def test_symlink_file_is_preserved(self):
        root = Path(tempfile.gettempdir()).resolve() / ("backend-check-" + str(uuid.uuid4()))
        root.mkdir(mode=0o700)
        link = root / (str(uuid.uuid4()) + ".json")
        try:
            with tempfile.TemporaryDirectory(prefix="fixture-file-target-") as directory:
                target = Path(directory) / "retained.txt"
                target.write_text("retained synthetic fixture")
                link.symlink_to(target)
                self.refuse(link)
                self.assertTrue(link.is_symlink())
                self.assertEqual(target.read_text(), "retained synthetic fixture")
        finally:
            link.unlink(missing_ok=True)
            root.rmdir()

    def test_symlink_directory_is_refused(self):
        with tempfile.TemporaryDirectory(prefix="fixture-link-target-") as target:
            root = Path(tempfile.gettempdir()).resolve() / ("backend-check-" + str(uuid.uuid4()))
            root.symlink_to(target, target_is_directory=True)
            try:
                self.refuse(root / (str(uuid.uuid4()) + ".json"))
                self.assertEqual(list(Path(target).iterdir()), [])
            finally:
                root.unlink()

    def test_shared_directory_is_refused(self):
        root = Path(tempfile.gettempdir()).resolve() / ("backend-check-" + str(uuid.uuid4()))
        root.mkdir(mode=0o700)
        try:
            os.chmod(root, 0o777)
            output = root / (str(uuid.uuid4()) + ".json")
            self.refuse(output)
            self.assertFalse(output.exists())
        finally:
            os.chmod(root, 0o700)
            root.rmdir()


if __name__ == "__main__":
    unittest.main()
