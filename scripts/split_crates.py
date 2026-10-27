"""Plan/apply the HTML phase extraction after semantic dependencies are removed.

No source snapshots are embedded: the current checkout is the sole input.
--check never writes. Default application requires a clean tree and a fully
downward source graph; all source bytes and destination collisions are checked
again before the first write. A second invocation produces no changes.
"""
import argparse
import json
from pathlib import Path
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from html_source_inventory import read, PREFIX, PACKAGES
from migration_plan import plan, Change, RANK
from migration_manifests import generate, original_contract
from migration_write import apply, preflight
from migration_guards import plan as guard_plan


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args], text=True).strip()


def assemble(root):
    root = Path(root)
    inventory = [item for item in read(root).values() if item.package in RANK]
    sources, physical, crate_roots = {}, {}, {}
    package_sources = {name: [] for name in RANK}
    for item in inventory:
        if item.logical.startswith(PREFIX + '__crate_roots__/'):
            crate_roots[item.package] = item.text
        else:
            name = item.logical.removeprefix(PREFIX)
            sources[name] = item.text
            physical[name] = item.physical
    existing = set(git(root, 'ls-files', '--cached', '--others', '--exclude-standard').splitlines())
    result = plan(sources, existing, physical, crate_roots)
    if result.blockers:
        return result
    after = dict(sources)
    by_physical = {path: name for name, path in physical.items()}
    for change in result.changes:
        if change.old in by_physical:
            after[by_physical[change.old]] = change.after
    from migration_plan import owner
    for name, text in after.items():
        package_sources[owner(name)].append(text)
    for name, text in crate_roots.items():
        package_sources[name].append(text)
    # Example/integration-test code stays in the upper package; its external
    # imports participate in dependency selection just like unit-test imports.
    for directory in ('examples', 'tests'):
        for path in (root / 'crates/html' / directory).rglob('*.rs'):
            package_sources['kamin-html'].append(path.read_bytes().decode('utf-8'))
    manifests = {}
    for package in RANK:
        path = PACKAGES[package].removesuffix('/src/') + '/Cargo.toml'
        if (root / path).is_file():
            manifests[path] = (root / path).read_bytes().decode('utf-8')
    original = original_contract(manifests)
    for name, text in generate(original, package_sources, result.edges).items():
        before = manifests.get(name, '')
        if before != text:
            result.changes.append(Change(name, name, before, text))
    result.changes.extend(guard_plan(root))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args(argv)
    root = args.root.resolve()
    initial_status = git(root, 'status', '--porcelain', '--untracked-files=all')
    try:
        result = assemble(root)
        if not result.blockers:
            preflight(root, result.changes)
    except (ValueError, SyntaxError, OSError) as error:
        parser.error(f'Preflight failed; no changes applied: {error}')
    report = dict(changes=len(result.changes),
                  edges=[dict(source=a, target=b, references=len(sites))
                         for (a, b), sites in sorted(result.edges.items())],
                  blockers=result.blockers, checked_includes=len(result.includes))
    print(json.dumps(report, ensure_ascii=False, indent=2))
    if result.blockers:
        return 2
    if args.check:
        return int(bool(result.changes))
    if not result.changes:
        return 0
    if initial_status:
        parser.error('Application requires a clean Git tree; inspect/commit the current diff')
    if git(root, 'status', '--porcelain', '--untracked-files=all') != initial_status:
        parser.error('Git tree changed while planning; no changes applied')
    apply(root, result.changes)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
