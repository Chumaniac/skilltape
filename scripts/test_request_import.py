"""Exercise the exported-input boundary without running arbitrary tasks."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

binary = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix='skilltape-import-') as directory:
    root = Path(directory).resolve()
    source = root / 'request.json'
    request = {'expected': [{'item': 'synthetic-paper', 'quantity': 5}],
               'received': [{'item': 'synthetic-paper', 'quantity': 4}]}
    source.write_text(json.dumps(request))
    def run(output, path=source):
        return subprocess.run([str(binary), 'starter', 'new', 'reconcile', '--request', str(path),
                               '--output', str(root / output)], capture_output=True)
    assert run('valid').returncode == 0
    assert json.loads((root / 'valid/inputs/request.json').read_text()) == request
    assert not (root / 'valid/delivery').exists()  # Import never executes or invents a Receipt.
    saved = (root / 'valid/inputs/request.json').read_bytes()
    assert run('valid').returncode != 0
    assert (root / 'valid/inputs/request.json').read_bytes() == saved
    for name, raw in [('unknown', json.dumps({**request, 'trusted': True})),
                      ('negative', json.dumps({'expected': [{'item': 'x', 'quantity': -1}], 'received': []})),
                      ('oversize', ' ' * (1024 * 1024 + 1)), ('malformed', '{')]:
        source.write_text(raw)
        assert run(name).returncode != 0
        assert not (root / name).exists()
    source.write_text(json.dumps(request))
    linked = root / 'linked.json'
    linked.symlink_to(source)
    assert run('linked-output', linked).returncode != 0
    assert not (root / 'linked-output').exists()
print('Exported request import, no-overwrite, schema, capacity and link boundaries passed.')
