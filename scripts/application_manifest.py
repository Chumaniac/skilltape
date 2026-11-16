"""Describe an owned portable application without including user data."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import platform
from package_release import verify_components


def create_manifest(application: Path, archive: Path):
    if application.is_symlink() or not application.is_dir() or archive.is_symlink() or not archive.is_file():
        raise ValueError('APPLICATION_OR_ARCHIVE_INVALID')
    config = json.loads((application / 'Contents/Resources/app.json').read_text())
    identity = json.loads((application / 'Contents/Resources/ui/build-info.json').read_text())
    revision = config.get('sourceCommit')
    if not isinstance(revision, str) or not re.fullmatch('[a-f0-9]{40}', revision) or config.get('includesWorkingTreeChanges') is not False or identity.get('sourceRevision') != revision or identity.get('dirty') is not False:
        raise ValueError('APPLICATION_SOURCE_MISMATCH')
    files = {}; total = 0
    for item in application.rglob('*'):
        if item.is_symlink() or not (item.is_file() or item.is_dir()):
            raise ValueError('APPLICATION_LINK_OR_SPECIAL_FILE')
        if item.is_file():
            total += item.stat().st_size
            if len(files) >= 30000 or total > 512 * 1024 * 1024:
                raise ValueError('APPLICATION_CAPACITY')
            with item.open('rb') as stream:
                files[str(item.relative_to(application))] = hashlib.file_digest(stream, 'sha256').hexdigest()
    runtime = json.loads((application / 'Contents/Resources/runtime/runtime-manifest.json').read_text())
    target = ('aarch64' if platform.machine() == 'arm64' else 'x86_64') + '-apple-darwin'
    components = verify_components(identity['version'], target, application / 'Contents/Resources/runtime/bin', application / 'Contents/Resources/ui')
    if components['sourceRevision'] != revision or runtime.get('sourceCommit') != revision or runtime.get('includesWorkingTreeChanges') is not False:
        raise ValueError('APPLICATION_SOURCE_MISMATCH')
    with archive.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'schema': 'skilltape.application-release/v1', 'version': identity['version'], 'sourceCommit': revision,
            'minimumSystemVersion': runtime['minimumSystemVersion'], 'signing': 'ad-hoc', 'notarized': False,
            'archive': archive.name, 'sha256': digest, 'files': files, 'bytes': total}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--application', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data = create_manifest(args.application, args.archive)
    with args.output.open('x') as stream:
        stream.write(json.dumps(data, sort_keys=True, indent=2) + '\n')


if __name__ == '__main__':
    main()
