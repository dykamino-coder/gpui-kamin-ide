"""Keep lower HTML test helpers out of the ordinary Cargo feature graph.

Only kamin-html dev-dependencies may select the reviewed support feature.
Cargo's resolved normal/feature edges complement manifest checks, including
workspace inheritance and target-specific dependencies. This never builds code.
"""

import argparse
from pathlib import Path
import re
import subprocess
import tomllib


FEATURE = 'engine-test-support'
LOWER = {'kamin-style', 'kamin-layout', 'kamin-paint'}
ROOT = Path(__file__).resolve().parents[1]


def selects_support(value):
    return value.split('/')[-1] == FEATURE


def manifest_errors(documents):
    """Validate also dormant optional dependencies: they must not enable helpers."""
    errors = []
    for path, data in sorted(documents.items()):
        owner = data.get('package', {}).get('name')
        features = data.get('features', {})
        if FEATURE in features and owner not in LOWER:
            errors.append(f'{path}: support feature belongs only to lower HTML crates')
        for name, enables in features.items():
            if any(selects_support(value) for value in enables):
                errors.append(f'{path}: feature {name} must not activate {FEATURE}')
        tables = [('package', data), ('workspace', data.get('workspace', {}))]
        tables.extend((f'target.{target}', table)
                      for target, table in data.get('target', {}).items())
        for context, table in tables:
            for kind in ('dependencies', 'build-dependencies', 'dev-dependencies'):
                for alias, spec in table.get(kind, {}).items():
                    if not isinstance(spec, dict) or FEATURE not in spec.get('features', []):
                        continue
                    target = spec.get('package', alias)
                    if not (context != 'workspace' and kind == 'dev-dependencies'
                            and owner == 'kamin-html' and target in LOWER):
                        errors.append(f'{path}: {context}.{kind}.{alias} may not activate {FEATURE}')
    return errors


def read_manifests(root):
    """Read the workspace and all declared members, including target tables."""
    root = Path(root)
    workspace = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))
    paths = {root / 'Cargo.toml'}
    excluded = {path.resolve() for pattern in workspace.get('workspace', {}).get('exclude', [])
                for path in root.glob(pattern)}
    for pattern in workspace.get('workspace', {}).get('members', []):
        for directory in root.glob(pattern):
            if directory.resolve() not in excluded and (directory / 'Cargo.toml').is_file():
                paths.add(directory / 'Cargo.toml')
    return {path.relative_to(root).as_posix(): tomllib.loads(path.read_text(encoding='utf-8'))
            for path in sorted(paths)}


def graph_errors(output):
    errors = []
    for line in output.splitlines():
        # The package format catches active features on workspace roots too;
        # explicit feature edges identify the path that selected a dependency.
        package, separator, features = line.partition('|')
        if (separator and FEATURE in features.strip().split(',')) or re.search(
                r'\bfeature "' + re.escape(FEATURE) + r'"', package):
            errors.append('normal Cargo graph activates test support: ' + line.strip())
    return sorted(set(errors))


def check(root, features=()):
    errors = manifest_errors(read_manifests(root))
    command = ['cargo', 'tree', '--manifest-path', str(Path(root) / 'Cargo.toml'),
               '--workspace', '--target', 'all', '--edges', 'normal,features',
               '--prefix', 'none', '--format', '{p}|{f}', '--locked']
    if features:
        command.extend(['--features', ','.join(features)])
    result = subprocess.run(command, cwd=root, text=True, capture_output=True)
    if result.returncode:
        errors.append('Cargo feature graph failed: ' + result.stderr.strip())
    else:
        errors.extend(graph_errors(result.stdout))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--features', action='append', default=[])
    args = parser.parse_args()
    errors = check(args.root, args.features)
    for error in errors:
        print(error)
    if not errors:
        print('engine-test-support: absent from ordinary dependencies and resolved features')
    return bool(errors)


if __name__ == '__main__':
    raise SystemExit(main())
