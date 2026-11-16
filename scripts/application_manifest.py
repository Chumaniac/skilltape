"""Describe an owned portable application without including user data."""
import argparse
import hashlib
import json
from pathlib import Path
import re
from package_release import validate_identities
from publication_paths import publication_path


def read_metadata(path, maximum):
    if not path.is_file() or path.is_symlink() or path.stat().st_size > maximum:
        raise ValueError('APPLICATION_METADATA_INVALID')
    return json.loads(path.read_text())


def create_manifest(application: Path, archive: Path):
    application = publication_path(application)
    archive = publication_path(archive)
    if application.is_symlink() or not application.is_dir() or archive.is_symlink() or not archive.is_file():
        raise ValueError('APPLICATION_OR_ARCHIVE_INVALID')
    if archive.stat().st_size > 1024 * 1024 * 1024:
        raise ValueError('APPLICATION_CAPACITY')
    config = read_metadata(application / 'Contents/Resources/app.json', 16384)
    identity = read_metadata(application / 'Contents/Resources/ui/build-info.json', 16384)
    revision = config.get('sourceCommit')
    if not isinstance(revision, str) or not re.fullmatch('[a-f0-9]{40}', revision) or config.get('includesWorkingTreeChanges') is not False or identity.get('sourceRevision') != revision or identity.get('dirty') is not False:
        raise ValueError('APPLICATION_SOURCE_MISMATCH')
    files = {}; total = 0
    for item in application.rglob('*'):
        if item.is_symlink() or not (item.is_file() or item.is_dir()):
            raise ValueError('APPLICATION_LINK_OR_SPECIAL_FILE')
        if item.is_file():
            total += item.stat().st_size
            relative = str(item.relative_to(application))
            if len(files) >= 30000 or total > 512 * 1024 * 1024 or len(relative.encode('utf-8')) > 1024:
                raise ValueError('APPLICATION_CAPACITY')
            with item.open('rb') as stream:
                files[relative] = hashlib.file_digest(stream, 'sha256').hexdigest()
    runtime = read_metadata(application / 'Contents/Resources/runtime/runtime-manifest.json', 8 * 1024 * 1024)
    components = runtime.get('buildIdentity', {})
    target = components.get('target')
    if target not in {'aarch64-apple-darwin', 'x86_64-apple-darwin'}:
        raise ValueError('APPLICATION_SOURCE_MISMATCH')
    # Inspect bytes only. Never execute binaries from a user-selected application.
    validate_identities(identity['version'], target, components, components, identity)
    if components['sourceRevision'] != revision or runtime.get('sourceCommit') != revision or runtime.get('includesWorkingTreeChanges') is not False:
        raise ValueError('APPLICATION_SOURCE_MISMATCH')
    prefix = 'Contents/Resources/runtime/'
    runtime_files = {name[len(prefix):]: digest for name, digest in files.items() if name.startswith(prefix) and name != prefix + 'runtime-manifest.json'}
    ui_prefix = 'Contents/Resources/ui/'
    ui_files = {name[len(ui_prefix):]: digest for name, digest in files.items() if name.startswith(ui_prefix)}
    binary_hashes = {name: runtime_files.get('bin/' + name) for name in ['skilltape', 'skilltape-console-api']}
    if runtime_files != runtime.get('files') or binary_hashes != components.get('components') or ui_files != components.get('uiFiles'):
        raise ValueError('APPLICATION_FILE_IDENTITY_MISMATCH')
    with archive.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    return {'schema': 'skilltape.application-release/v1', 'version': identity['version'], 'sourceCommit': revision,
            'minimumSystemVersion': runtime['minimumSystemVersion'], 'signing': 'ad-hoc', 'notarized': False,
            'archive': archive.name, 'sha256': digest, 'files': files, 'bytes': total,
            'validation': 'file hashes; verify published provenance separately'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--application', type=Path, required=True)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    data = create_manifest(args.application, args.archive)
    with publication_path(args.output).open('x') as stream:
        stream.write(json.dumps(data, sort_keys=True, indent=2) + '\n')


if __name__ == '__main__':
    main()
