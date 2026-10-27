"""Private draft: keep HTML checks keyed by logical phase after crate extraction.

This reads sources only. It does not rewrite paths, loosen a ratchet or infer
which implementation belongs to a crate. Package/module rules are reviewed input.
"""

from dataclasses import dataclass
import io
from pathlib import Path, PurePosixPath
import subprocess
import tarfile


PREFIX = 'crates/html/src/'
PACKAGES = {
    'kamin-html': 'crates/html/src/',
    'kamin-style': 'crates/html-style/src/',
    'kamin-layout': 'crates/html-layout/src/',
    'kamin-paint': 'crates/html-paint/src/',
    'kamin-font-facts': 'crates/font-facts/src/',
}


@dataclass(frozen=True)
class Source:
    physical: str
    logical: str
    package: str
    text: str


def logical_name(package, relative):
    """Implementation namespaces stay stable; every generated crate root counts."""
    path = PurePosixPath(relative)
    if path.is_absolute() or '..' in path.parts:
        raise ValueError(f'unsafe HTML source path: {relative}')
    if relative == 'lib.rs' and package != 'kamin-html':
        relative = '__crate_roots__/' + package.replace('-', '_') + '.rs'
    return PREFIX + relative


def inventory(files):
    """Consume physical path -> text; reject two owners of one logical source."""
    result = {}
    for physical, text in sorted(files.items()):
        if not physical.endswith('.rs'):
            continue
        matches = [(package, prefix) for package, prefix in PACKAGES.items()
                   if physical.startswith(prefix)]
        if not matches:
            continue
        if len(matches) != 1:
            raise ValueError(f'ambiguous HTML source package: {physical}')
        package, prefix = matches[0]
        logical = logical_name(package, physical[len(prefix):])
        if logical in result:
            raise ValueError(f'duplicate logical HTML source: {logical}: '
                             f'{result[logical].physical}, {physical}')
        result[logical] = Source(physical, logical, package, text)
    return result


def read(root, revision=None):
    root = Path(root)
    files = {}
    if revision is not None:
        # ls-tree accepts absent paths at an older, unsplit revision.
        listing = subprocess.check_output(
            ['git', '-C', str(root), 'ls-tree', '-r', '--name-only', revision,
             '--', *PACKAGES.values()], text=True).splitlines()
        # Archive directory roots, not thousands of individual file names:
        # Windows caps a subprocess command line at 32767 UTF-16 characters.
        prefixes = [prefix for prefix in PACKAGES.values()
                    if any(path.startswith(prefix) for path in listing)]
        if prefixes:
            data = subprocess.check_output(
                ['git', '-C', str(root), 'archive', revision, '--', *prefixes])
            with tarfile.open(fileobj=io.BytesIO(data)) as archive:
                for item in archive:
                    if item.isfile() and item.name.endswith('.rs'):
                        files[item.name] = archive.extractfile(item).read().decode('utf-8')
    else:
        for prefix in PACKAGES.values():
            for path in (root / prefix).rglob('*.rs'):
                if not path.is_file():
                    raise ValueError(f'Source is not a regular file: {path.relative_to(root).as_posix()}')
                files[path.relative_to(root).as_posix()] = path.read_bytes().decode('utf-8')
    return inventory(files)


def texts(root, revision=None):
    return {logical: item.text for logical, item in read(root, revision).items()}
