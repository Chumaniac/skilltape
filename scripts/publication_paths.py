"""Constrain operator-selected publication paths to the invocation directory."""
import os
from pathlib import Path


def publication_path(value):
    basedir = os.path.realpath(os.getcwd())
    filepath = os.path.realpath(os.path.abspath(os.path.expanduser(value)))
    prefix = basedir.rstrip(os.sep) + os.sep
    if filepath == basedir:
        return Path(basedir)
    if not filepath.startswith(prefix):
        raise ValueError('PATH_OUTSIDE_INVOCATION_ROOT')
    return Path(filepath)
