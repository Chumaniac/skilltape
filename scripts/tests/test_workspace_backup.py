import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('workspace_backup', Path(__file__).resolve().parents[1] / 'workspace_backup.py')
backup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(backup)


class WorkspaceBackupTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name).resolve()
        self.source = self.root / 'source'
        self.source.mkdir()
        (self.source / 'receipts').mkdir()
        (self.source / 'receipts/synthetic.json').write_text('{"status":"verified","inputSha256":"synthetic"}')
        (self.source / '清单.csv').write_bytes('物品,数量\n笔记本,4\n'.encode())
        self.saved = self.root / 'backup'

    def tearDown(self):
        self.directory.cleanup()

    def create(self):
        return backup.backup(self.source, self.saved, stopped=True)

    def test_stopped_snapshot_checks_and_restores_exact_bytes_into_new_workspace(self):
        self.create()
        data = backup.check(self.saved)
        self.assertEqual(data['totalBytes'], sum(p.stat().st_size for p in self.source.rglob('*') if p.is_file()))
        output = self.root / 'recovered'
        backup.restore(self.saved, output)
        self.assertFalse((output / 'INCOMPLETE').exists())
        self.assertTrue((output / 'COMPLETE').is_file())
        for path in self.source.rglob('*'):
            if path.is_file():
                self.assertEqual(path.read_bytes(), (output / 'workspace' / path.relative_to(self.source)).read_bytes())
        self.assertEqual(backup.check(self.saved), data)

    def test_existing_destination_and_unconfirmed_running_workspace_are_preserved(self):
        with self.assertRaisesRegex(ValueError, 'STOP_WORKSPACE'):
            backup.backup(self.source, self.saved)
        self.saved.mkdir()
        keep = self.saved / 'successful-original'
        keep.write_text('preserve')
        with self.assertRaisesRegex(ValueError, 'DESTINATION_EXISTS'):
            self.create()
        self.assertEqual(keep.read_text(), 'preserve')
        (self.source / 'db.sqlite-wal').write_bytes(b'active')
        with self.assertRaisesRegex(ValueError, 'RUNNING'):
            backup.backup(self.source, self.root / 'another', stopped=True)

    def test_changed_payload_is_refused_before_restore_creates_any_output(self):
        self.create()
        (self.saved / 'tree/清单.csv').write_bytes(b'changed')
        output = self.root / 'recovered'
        with self.assertRaisesRegex(ValueError, 'DIGEST_MISMATCH'):
            backup.restore(self.saved, output)
        self.assertFalse(output.exists())
        self.assertEqual((self.source / '清单.csv').read_bytes(), '物品,数量\n笔记本,4\n'.encode())

    def test_traversal_is_rejected_even_if_metadata_checksum_was_recalculated(self):
        self.create()
        data = json.loads((self.saved / 'MANIFEST.json').read_bytes())
        data['entries'][0]['path'] = '../escape'
        raw = json.dumps(data).encode()
        (self.saved / 'MANIFEST.json').write_bytes(raw)
        (self.saved / 'COMPLETE').write_text(hashlib.sha256(raw).hexdigest())
        with self.assertRaisesRegex(ValueError, 'PATH'):
            backup.check(self.saved)
        self.assertFalse((self.root / 'escape').exists())

    def test_symlink_hardlink_and_recursive_output_do_not_create_valid_backups(self):
        with self.assertRaisesRegex(ValueError, 'INSIDE_SOURCE'):
            backup.backup(self.source, self.source / 'nested', stopped=True)
        self.assertFalse((self.source / 'nested').exists())
        linked = self.source / 'linked'
        try:
            linked.symlink_to(self.source / '清单.csv')
        except OSError:
            self.skipTest('OS does not grant symlink creation to this test user')
        with self.assertRaisesRegex(ValueError, 'LINK'):
            self.create()
        linked.unlink()
        os.link(self.source / '清单.csv', linked)
        with self.assertRaisesRegex(ValueError, 'HARD_LINK'):
            self.create()
        self.assertFalse(self.saved.exists())

    def test_failure_keeps_incomplete_output_and_never_changes_source(self):
        expected = (self.source / '清单.csv').read_bytes()
        with patch.object(backup, '_copy', side_effect=OSError('synthetic copy interruption')):
            with self.assertRaises(OSError):
                self.create()
        self.assertTrue((self.saved / 'INCOMPLETE').is_file())
        self.assertFalse((self.saved / 'COMPLETE').exists())
        with self.assertRaisesRegex(ValueError, 'INCOMPLETE'):
            backup.check(self.saved)
        self.assertEqual((self.source / '清单.csv').read_bytes(), expected)

    def test_source_mutation_during_copy_is_rejected_as_a_completed_snapshot(self):
        copy = backup._copy
        def concurrent(source, output, expected, mode=0o600):
            result = copy(source, output, expected, mode)
            if Path(source).name == '清单.csv':
                Path(source).write_bytes(b'new synthetic editor content')
            return result
        with patch.object(backup, '_copy', side_effect=concurrent):
            with self.assertRaisesRegex(ValueError, 'SOURCE_CHANGED'):
                self.create()
        self.assertTrue((self.saved / 'INCOMPLETE').exists())
        self.assertFalse((self.saved / 'COMPLETE').exists())
        self.assertEqual((self.source / '清单.csv').read_bytes(), b'new synthetic editor content')

    def test_declared_resource_limits_are_checked_before_creating_output(self):
        with patch.object(backup, 'MAX_ENTRIES', 1):
            with self.assertRaisesRegex(ValueError, 'ENTRY_LIMIT'):
                self.create()
        self.assertFalse(self.saved.exists())
        with patch.object(backup, 'MAX_TOTAL_BYTES', 1):
            with self.assertRaisesRegex(ValueError, 'BYTE_LIMIT'):
                self.create()
        self.assertFalse(self.saved.exists())
        with patch.object(backup.shutil, 'disk_usage', return_value=type('Disk', (), {'free': 0})()):
            with self.assertRaisesRegex(ValueError, 'INSUFFICIENT_SPACE'):
                self.create()
        self.assertFalse(self.saved.exists())


if __name__ == '__main__':
    unittest.main()
