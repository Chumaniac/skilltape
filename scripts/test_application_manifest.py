import json
import platform
from pathlib import Path
import tempfile
import unittest
from application_manifest import create_manifest


class ManifestTests(unittest.TestCase):
    def fixture(self, root):
        app = root / 'SkillTape.app'; resources = app / 'Contents/Resources'
        (resources / 'ui').mkdir(parents=True); (resources / 'runtime').mkdir()
        (resources / 'app.json').write_text(json.dumps({'sourceCommit': 'a' * 40, 'includesWorkingTreeChanges': False}))
        (resources / 'ui/build-info.json').write_text(json.dumps({'sourceRevision': 'a' * 40, 'dirty': False, 'version': '0.5.0'}))
        (resources / 'runtime/runtime-manifest.json').write_text(json.dumps({'minimumSystemVersion': '13.0', 'sourceCommit': 'a' * 40, 'includesWorkingTreeChanges': False}))
        target = ('aarch64' if platform.machine() == 'arm64' else 'x86_64') + '-apple-darwin'
        identity = {'schema': 'skilltape.build-info/v1', 'sourceRevision': 'a' * 40, 'dirty': False, 'channel': 'release', 'version': '0.5.0', 'target': target, 'capabilities': json.loads((Path(__file__).parents[1] / 'product-capabilities.json').read_text())['capabilities']}
        (resources / 'ui/build-info.json').write_text(json.dumps(identity))
        binaries = resources / 'runtime/bin'; binaries.mkdir()
        for name in ['skilltape', 'skilltape-console-api']:
            executable = binaries / name
            executable.write_text('#!/bin/sh\nprintf \'%s\\n\' \'' + json.dumps(identity) + '\'\n')
            executable.chmod(0o700)
        archive = root / 'application.zip'; archive.write_bytes(b'isolated archive fixture')
        return app, resources, archive

    def test_manifest_binds_the_exact_clean_app_without_claiming_notarization(self):
        with tempfile.TemporaryDirectory() as temporary:
            app, resources, archive = self.fixture(Path(temporary))
            data = create_manifest(app, archive)
            self.assertEqual(data['version'], '0.5.0'); self.assertEqual(data['sourceCommit'], 'a' * 40)
            self.assertFalse(data['notarized']); self.assertEqual(len(data['sha256']), 64)
            self.assertIn('Contents/Resources/app.json', data['files'])
            (resources / 'ui/build-info.json').write_text(json.dumps({'sourceRevision': 'b' * 40, 'dirty': False, 'version': '0.5.0'}))
            with self.assertRaisesRegex(ValueError, 'SOURCE_MISMATCH'): create_manifest(app, archive)

    def test_application_cannot_collect_an_external_workspace_link(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); app, resources, archive = self.fixture(root)
            outside = root / 'original'; outside.write_bytes(b'private original remains outside')
            (resources / 'unexpected').symlink_to(outside)
            with self.assertRaisesRegex(ValueError, 'LINK_OR_SPECIAL_FILE'): create_manifest(app, archive)
            self.assertEqual(outside.read_bytes(), b'private original remains outside')

    def test_mixed_native_api_is_not_a_same_source_application(self):
        with tempfile.TemporaryDirectory() as temporary:
            app, resources, archive = self.fixture(Path(temporary))
            identity = json.loads((resources / 'ui/build-info.json').read_text())
            identity['sourceRevision'] = 'b' * 40
            executable = resources / 'runtime/bin/skilltape-console-api'
            executable.write_text('#!/bin/sh\nprintf \'%s\\n\' \'' + json.dumps(identity) + '\'\n')
            with self.assertRaisesRegex(ValueError, 'mismatch'): create_manifest(app, archive)


if __name__ == '__main__': unittest.main()
