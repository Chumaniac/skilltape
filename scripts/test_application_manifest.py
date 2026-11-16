import hashlib
import json
import platform
from contextlib import chdir
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from application_manifest import create_manifest

ROOT = Path(__file__).resolve().parents[1]

class ManifestTests(unittest.TestCase):
    def fixture(self, root):
        app = root / 'SkillTape.app'; resources = app / 'Contents/Resources'
        (resources / 'ui').mkdir(parents=True); (resources / 'runtime/bin').mkdir(parents=True)
        (resources / 'app.json').write_text(json.dumps({'sourceCommit': 'a' * 40, 'includesWorkingTreeChanges': False}))
        target = ('aarch64' if platform.machine() == 'arm64' else 'x86_64') + '-apple-darwin'
        identity = {'schema': 'skilltape.build-info/v1', 'sourceRevision': 'a' * 40, 'dirty': False, 'channel': 'release', 'version': '0.5.0', 'target': target, 'capabilities': json.loads((ROOT / 'product-capabilities.json').read_text())['capabilities']}
        ui = {k: v for k, v in identity.items() if k != 'target'}
        (resources / 'ui/build-info.json').write_text(json.dumps(ui))
        binaries = resources / 'runtime/bin'
        for name in ['skilltape', 'skilltape-console-api']:
            (binaries / name).write_bytes(b'opaque native fixture, never execute')
        digest = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
        identity['components'] = {name: digest(binaries / name) for name in ['skilltape', 'skilltape-console-api']}
        identity['uiFiles'] = {'build-info.json': digest(resources / 'ui/build-info.json')}
        runtime_files = {'bin/' + name: digest(binaries / name) for name in ['skilltape', 'skilltape-console-api']}
        (resources / 'runtime/runtime-manifest.json').write_text(json.dumps({'minimumSystemVersion': '13.0', 'sourceCommit': 'a' * 40, 'includesWorkingTreeChanges': False, 'buildIdentity': identity, 'files': runtime_files}))
        archive = root / 'application.zip'; archive.write_bytes(b'isolated archive fixture')
        return app, resources, archive

    def test_manifest_binds_the_exact_clean_app_without_claiming_notarization(self):
        with tempfile.TemporaryDirectory() as temporary, chdir(temporary):
            app, resources, archive = self.fixture(Path(temporary))
            with mock.patch('subprocess.Popen', side_effect=AssertionError('inspection must not execute app code')):
                data = create_manifest(app, archive)
            self.assertEqual(data['version'], '0.5.0'); self.assertEqual(data['sourceCommit'], 'a' * 40)
            self.assertFalse(data['notarized']); self.assertEqual(len(data['sha256']), 64)
            self.assertIn('Contents/Resources/app.json', data['files'])
            (resources / 'ui/build-info.json').write_text(json.dumps({'sourceRevision': 'b' * 40, 'dirty': False, 'version': '0.5.0'}))
            with self.assertRaisesRegex(ValueError, 'SOURCE_MISMATCH'): create_manifest(app, archive)

    def test_application_cannot_collect_an_external_workspace_link(self):
        with tempfile.TemporaryDirectory() as temporary, chdir(temporary):
            root = Path(temporary); app, resources, archive = self.fixture(root)
            outside = root / 'original'; outside.write_bytes(b'private original remains outside')
            (resources / 'unexpected').symlink_to(outside)
            with self.assertRaisesRegex(ValueError, 'LINK_OR_SPECIAL_FILE'): create_manifest(app, archive)
            self.assertEqual(outside.read_bytes(), b'private original remains outside')

    def test_mixed_native_api_bytes_are_not_a_same_source_application(self):
        with tempfile.TemporaryDirectory() as temporary, chdir(temporary):
            app, resources, archive = self.fixture(Path(temporary))
            (resources / 'runtime/bin/skilltape-console-api').write_bytes(b'changed native bytes')
            with self.assertRaisesRegex(ValueError, 'IDENTITY_MISMATCH'): create_manifest(app, archive)

    def test_source_cannot_escape_the_explicit_invocation_context(self):
        with tempfile.TemporaryDirectory() as temporary, tempfile.TemporaryDirectory() as outside, chdir(temporary):
            app, resources, archive = self.fixture(Path(outside))
            with self.assertRaisesRegex(ValueError, 'OUTSIDE_INVOCATION_ROOT'): create_manifest(app, archive)
            link = Path(temporary) / 'redirect'; link.symlink_to(outside, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'OUTSIDE_INVOCATION_ROOT'): create_manifest(link / app.name, archive)

if __name__ == '__main__': unittest.main()
