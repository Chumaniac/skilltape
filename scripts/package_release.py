#!/usr/bin/env python3
"""Assemble a self-contained SkillTape CLI and Console release archive."""

import argparse
import shutil
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path


def safe_component(value: str, label: str) -> str:
    if (
        not value
        or value in {".", ".."}
        or "/" in value
        or "\\" in value
        or Path(value).name != value
    ):
        raise ValueError(f"{label} must be a single safe path component")
    return value


def require_directory(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_dir():
        raise ValueError(f"{label} is not a regular directory: {path}")


def require_file(path: Path, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} is not a regular file: {path}")


def copy_ui(ui_dist: Path, destination: Path) -> None:
    require_directory(ui_dist, "UI dist")
    require_file(ui_dist / "index.html", "UI index")
    assets = ui_dist / "assets"
    require_directory(assets, "UI assets")
    asset_files = []
    for source in sorted(ui_dist.rglob("*")):
        relative = source.relative_to(ui_dist)
        target = destination / relative
        if source.is_symlink():
            raise ValueError(f"UI contains a symlink: {source}")
        if source.is_dir():
            target.mkdir(parents=True, exist_ok=True)
        elif source.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
            if source.is_relative_to(assets):
                asset_files.append(source)
        else:
            raise ValueError(f"UI contains a non-regular entry: {source}")
    if not asset_files:
        raise ValueError("UI assets directory must contain at least one regular file")


def stage_release(
    version: str, target: str, binary_dir: Path, ui_dist: Path, staging_root: Path
) -> Path:
    archive_root = staging_root / f"skilltape-v{version}-{target}"
    console_root = archive_root / "console"
    archive_root.mkdir(parents=True)
    require_directory(binary_dir, "release binary directory")
    suffix = ".exe" if "windows" in target else ""
    require_file(binary_dir / f"skilltape{suffix}", "skilltape binary")
    require_file(binary_dir / f"skilltape-console-api{suffix}", "Console API binary")
    shutil.copy2(binary_dir / f"skilltape{suffix}", archive_root / f"skilltape{suffix}")
    shutil.copy2(
        binary_dir / f"skilltape-console-api{suffix}",
        archive_root / f"skilltape-console-api{suffix}",
    )
    copy_ui(ui_dist, console_root)
    return archive_root


def iter_entries(root: Path):
    yield root
    yield from sorted(root.rglob("*"))


def add_tar_entry(package: tarfile.TarFile, path: Path, archive_root: Path) -> None:
    relative = path.relative_to(archive_root.parent)
    info = package.gettarinfo(str(path), arcname=str(relative))
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    if path.is_file():
        with path.open("rb") as source:
            package.addfile(info, source)
    else:
        package.addfile(info)


def write_archive(archive_root: Path, output: Path, windows: bool) -> None:
    partial = output.with_name(output.name + ".partial")
    if output.exists() or output.is_symlink():
        raise ValueError(f"release archive already exists: {output}")
    try:
        if windows:
            with zipfile.ZipFile(partial, "w", compression=zipfile.ZIP_DEFLATED) as package:
                for path in iter_entries(archive_root):
                    relative = path.relative_to(archive_root.parent).as_posix()
                    if path.is_dir():
                        relative += "/"
                        package.writestr(zipfile.ZipInfo(relative, (1980, 1, 1, 0, 0, 0)), b"")
                    else:
                        info = zipfile.ZipInfo(relative, (1980, 1, 1, 0, 0, 0))
                        info.compress_type = zipfile.ZIP_DEFLATED
                        package.writestr(info, path.read_bytes())
        else:
            with tarfile.open(partial, "w:gz") as package:
                for path in iter_entries(archive_root):
                    add_tar_entry(package, path, archive_root)
        partial.replace(output)
    except Exception:
        partial.unlink(missing_ok=True)
        raise


def build_archive(
    version: str, target: str, binary_dir: Path, ui_dist: Path, output_dir: Path
) -> Path:
    version = safe_component(version.removeprefix("v"), "version")
    target = safe_component(target, "target")
    if output_dir.exists() and output_dir.is_symlink():
        raise ValueError(f"output directory is a symlink: {output_dir}")
    output_dir.mkdir(parents=True, exist_ok=True)
    suffix = ".zip" if "windows" in target else ".tar.gz"
    output = output_dir / f"skilltape-v{version}-{target}{suffix}"
    with tempfile.TemporaryDirectory(prefix="skilltape-release-") as temporary:
        staging_root = Path(temporary)
        archive_root = stage_release(version, target, binary_dir, ui_dist, staging_root)
        write_archive(archive_root, output, "windows" in target)
    return output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--binary-dir", type=Path, required=True)
    parser.add_argument("--ui-dist", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        archive = build_archive(
            args.version, args.target, args.binary_dir, args.ui_dist, args.output_dir
        )
    except (OSError, ValueError) as error:
        print(f"package release failed: {error}", file=sys.stderr)
        return 1
    print(archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
