"""Enforce ARCHITECTURE.md's HTML phase dependencies, including all cfg branches.

Run from any directory. Exceptions name the exact source file and Rust path,
an occurrence budget, an owner and a concrete removal task. Stale exceptions
fail. CI's --base additionally prevents increasing the exception inventory.
Root/intermediate reexports and aliases resolve to their defining modules.
Macro expansion/generated imports and include! contents are not resolved;
literal explicit paths in macro bodies are still scanned (all cfg branches).
"""

import argparse
from collections import Counter
import json
from pathlib import Path
import subprocess
import sys

# Support embedded Python distributions that omit the script directory.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from rust_layer_paths import dependencies
from rust_layer_exports import Exports
from html_source_inventory import PREFIX, read
from html_guard_paths import normalize


REPO = Path(__file__).resolve().parent.parent
ALLOWLIST = 'scripts/layer_import_allowlist.json'
STAGE = {'dom': 0, 'box_facts': 0, 'font_facts': 0, 'style': 1, 'layout': 2, 'text': 2, 'paint': 3, 'svg': 3, 'render': 4}


def forbidden(source, target):
    return STAGE[target] > STAGE[source] and (source, target) != ('dom', 'style')


def inventory(root):
    found = []
    sources = {name.removeprefix(PREFIX): normalize(item.text)
               for name, item in read(root).items()}
    exports = Exports(sources)
    for relative, contents in sources.items():
        source = relative.split('/')[0].removesuffix('.rs')
        if source not in STAGE or source == 'render':
            continue
        for target, line in dependencies(contents, relative, STAGE):
            diagnostics = set()
            for resolved in exports.paths(target, STAGE):
                phase = resolved.split('::')[0]
                if phase in STAGE and forbidden(source, phase):
                    # Preserve exact debt keys when spelling already names its phase.
                    diagnostic = target if target.split('::')[0] == phase else resolved
                    diagnostics.add(diagnostic)
            found.extend((relative, path, line) for path in sorted(diagnostics))
    return found


def entries(data):
    result = {}
    for item in data:
        key = (item['file'], item['path'])
        if key in result:
            raise ValueError(f'duplicate exception: {key}')
        if not item.get('owner', '').strip() or not item.get('task', '').strip():
            raise ValueError(f'exception needs owner and removal task: {key}')
        if type(item.get('count')) is not int or item['count'] < 1:
            raise ValueError(f'exception needs a positive occurrence count: {key}')
        if '*' in item['file'] or '*' in item['path']:
            raise ValueError(f'wildcard exceptions are forbidden: {key}')
        result[key] = item['count']
    return result


def growth(root, base, current):
    revision = subprocess.check_output(
        ['git', '-C', str(root), 'merge-base', base, 'HEAD'], text=True,
    ).strip()
    present = subprocess.run(
        ['git', '-C', str(root), 'cat-file', '-e', f'{revision}:{ALLOWLIST}'],
        capture_output=True,
    )
    if present.returncode:
        # Bootstrap: the first commit installing this check records existing debt.
        return []
    previous = entries(json.loads(subprocess.check_output(
        ['git', '-C', str(root), 'show', f'{revision}:{ALLOWLIST}'], text=True,
    )))
    return [f'exception inventory grew: {file} -> {path}'
            for (file, path), count in current.items()
            if count > previous.get((file, path), 0)]


def check(root, base=None):
    allowed = entries(json.loads((root / ALLOWLIST).read_text(encoding='utf-8')))
    found = inventory(root)
    counts = Counter((file, path) for file, path, _ in found)
    errors = []
    for (file, path), count in counts.items():
        budget = allowed.get((file, path), 0)
        if count > budget:
            lines = ','.join(str(line) for f, p, line in found if (f, p) == (file, path))
            errors.append(f'{file}:{lines}: forbidden {file.split("/")[0]} -> {path} '
                          f'({count} references, allowed {budget})')
    for key, budget in allowed.items():
        if counts[key] < budget:
            errors.append(f'stale exception: {key[0]} -> {key[1]} '
                          f'(found {counts[key]}, recorded {budget}); shrink allowlist')
    if base:
        errors.extend(growth(root, base, allowed))
    return errors, len(found), len(allowed)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO)
    parser.add_argument('--base', help='Git base revision; exception inventory may only shrink')
    args = parser.parse_args()
    try:
        errors, references, exceptions = check(args.root.resolve(), args.base)
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        print(f'layer imports: {error}', file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    print(f'layer imports: {len(errors)} errors; {references} inherited references '
          f'in {exceptions} exact exceptions')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())

# KaminIDE crate-split guard adapter schema 1
