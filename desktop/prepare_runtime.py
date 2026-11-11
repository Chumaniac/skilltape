"""Assemble an owned, relocatable runtime. Never copy a user's workspace."""
import argparse
import hashlib
import json
import os
import re
import platform
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def declared_minimum(text):
    # vtool also prints linker/tool versions. They are not OS deployment targets.
    values = re.findall(r'^\s*minos\s+(\d+(?:\.\d+){1,2})\s*$', text, re.M)
    values += re.findall(r'cmd LC_VERSION_MIN_MACOSX\s+cmdsize \d+\s+version (\d+(?:\.\d+){1,2})', text)
    if not values:
        raise ValueError('MACHO_DEPLOYMENT_TARGET_MISSING')
    return max(values, key=lambda value: tuple(map(int, value.split('.'))))


def minimum_macos(directory):
    versions = {'native-window': '13.0'}
    for path in directory.rglob('*'):
        if not path.is_file() or path.is_symlink():
            continue
        with path.open('rb') as stream:
            magic = stream.read(4)
        if magic not in {b'\xcf\xfa\xed\xfe', b'\xfe\xed\xfa\xcf', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca'}:
            continue
        output = subprocess.check_output(['xcrun', 'vtool', '-show-build', str(path)], text=True, timeout=15)
        versions[str(path.relative_to(directory))] = declared_minimum(output)
    return max(versions.values(), key=lambda value: tuple(map(int, value.split('.')))), versions


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    profile = json.loads((ROOT / 'desktop/portable.json').read_text())
    product = profile['product']
    output = args.output.expanduser().absolute()
    if output.exists():
        raise SystemExit('OUTPUT_EXISTS; select a new empty directory')
    output.mkdir(parents=True)
    app = output / 'app'
    app.mkdir()

    def copy(relative, destination=None):
        source = ROOT / relative
        target = output / destination if destination else app / relative
        if not source.exists() or source.is_symlink():
            raise SystemExit('BUILD_ASSET_MISSING_OR_LINK: ' + relative)
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(source, target, ignore=shutil.ignore_patterns('__pycache__', '*.pyc', 'node_modules', '.DS_Store'))
        else:
            shutil.copy2(source, target)

    if product != 'skilltape':
        raise SystemExit('PRODUCT_UNSUPPORTED')
    for binary in ['skilltape', 'skilltape-console-api']:
        copy('target/release/' + binary, 'bin/' + binary)

    if (ROOT / 'desktop/runtime-licenses').is_dir():
        shutil.copytree(ROOT / 'desktop/runtime-licenses', output / 'runtime-licenses')
    for relative in ['LICENSE', 'THIRD_PARTY_NOTICES.md']:
        if (ROOT / relative).is_file():
            shutil.copy2(ROOT / relative, output / relative)
    # Reject external links after assembly, including Python runtime links.
    for p in output.rglob('*'):
        if p.is_symlink() and not p.resolve().is_relative_to(output.resolve()):
            raise SystemExit('EXTERNAL_RUNTIME_LINK: ' + str(p.relative_to(output)))
    manifest = {str(p.relative_to(output)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(output.rglob('*')) if p.is_file() and not p.is_symlink()}
    minimum, deployments = minimum_macos(output)
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip())
    (output / 'runtime-manifest.json').write_text(json.dumps({'schema': 'runtime-files/v1', 'product': product, 'sourceCommit': revision, 'includesWorkingTreeChanges': dirty, 'minimumSystemVersion': minimum, 'binaryDeploymentTargets': deployments, 'files': manifest}, indent=2) + '\n')
    print(json.dumps({'product': product, 'files': len(manifest), 'runtime': str(output)}))


if __name__ == '__main__':
    main()
