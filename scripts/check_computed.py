"""ADR3 shrinking used-write/mutable-escape inventory (like check_transitional).

The baseline includes informational reads, test fixtures and style construction writes. Only
non-style writes/mutable projections and escape calls have non-growing ceilings.
Matched owner transfers preserve field/access totals. Decreases need no
regeneration. --write lowers or redistributes ceilings; --base also
prevents raising checked-in ceilings. First installation compares base sources.
Record accepted owner transfers with --write before committing their source diff.
"""

import argparse
from collections import Counter
import io
import json
from pathlib import Path
import subprocess
import sys
import tarfile

# Embedded Python distributions may omit the script directory.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from computed_access import PREFIX, aggregate, guarded
from computed_f2_rebaseline import approved_base

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = 'scripts/computed_manifest'


def git(root, *args):
    return subprocess.check_output(['git', '-C', str(root), *args])


def sources(root, revision=None):
    if revision:
        with tarfile.open(fileobj=io.BytesIO(git(root, 'archive', revision, PREFIX))) as archive:
            return {p.name: archive.extractfile(p).read().decode('utf-8')
                    for p in archive if p.isfile() and p.name.endswith('.rs')}
    return {p.relative_to(root).as_posix(): p.read_text(encoding='utf-8')
            for p in (root / PREFIX).rglob('*.rs')}


def decode(documents):
    counts = Counter()
    for document in documents:
        rows = json.loads(document)
        if not isinstance(rows, list):
            raise ValueError('computed shard must be an array')
        for row in rows:
            if set(row) != {'field', 'owners'}:
                raise ValueError('invalid computed row')
            for owner, kinds in row['owners'].items():
                for kind, count in kinds.items():
                    key = row['field'], owner, kind
                    if key in counts or type(count) is not int or count <= 0:
                        raise ValueError(f'invalid/duplicate computed count: {key}')
                    if kind not in ('read', 'write', 'mutable', 'initialize', 'escape'):
                        raise ValueError(f'invalid computed access: {kind}')
                    counts[key] = count
    if not counts:
        raise ValueError('empty computed inventory')
    return counts


def documents(counts):
    rows = {}
    for (field, owner, kind), count in sorted(counts.items()):
        rows.setdefault(field, {}).setdefault(owner, {})[kind] = count
    lines = [json.dumps({'field': field, 'owners': owners}, separators=(',', ':'))
             for field, owners in rows.items()]
    return {f'access-{offset // 200:03}.json': '[\n' + ',\n'.join(lines[offset:offset + 200]) + '\n]\n'
            for offset in range(0, len(lines), 200)}


def previous(root, revision):
    paths = git(root, 'ls-tree', '-r', '--name-only', revision, '--', INVENTORY).decode().splitlines()
    paths = [p for p in paths if Path(p).name.startswith('access-') and p.endswith('.json')]
    if paths:
        return decode(git(root, 'show', f'{revision}:{p}').decode() for p in paths)
    return None


def owner_changes(current, previous):
    """Only production debt in this source diff can authorize a transfer."""
    return {key: current[key] - previous[key] for key in current.keys() | previous.keys()
            if guarded(key) and current[key] != previous[key]}


def transferred_growth(current, previous, label, changes):
    """Keep field/access budgets separate and match actual donor/recipient deltas.

    A ceiling's historical slack cannot donate debt. Test fixtures and unrestricted
    style construction cannot fund production writes. Sorting makes reports stable.
    """
    groups = {}
    for key, count in sorted(current.items()):
        if guarded(key) and count > previous[key]:
            groups.setdefault((key[0], key[2]), []).append(key)
    errors, transfers = [], []
    for (field, kind), recipients in sorted(groups.items()):
        donors = {key: min(previous[key] - current[key], -delta)
                  for key, delta in sorted(changes.items())
                  if key[0] == field and key[2] == kind and delta < 0
                  and previous[key] > current[key]}
        needed = sum(current[key] - previous[key] for key in recipients)
        allowed = all(current[key] - previous[key] <= changes.get(key, 0)
                      for key in recipients) and needed <= sum(donors.values())
        if not allowed:
            errors.extend(f'{label}: {"/".join(key)} grew {previous[key]} -> {current[key]}'
                          for key in recipients)
            continue
        for key in recipients:
            remaining = current[key] - previous[key]
            for donor in donors:
                amount = min(remaining, donors[donor])
                if amount:
                    transfers.append(f'{label}: transfer {field}/{kind}: '
                                     f'{donor[1]} -> {key[1]} ({amount})')
                    donors[donor] -= amount
                    remaining -= amount
                if not remaining:
                    break
    return errors, transfers


def check(root, base, write=False):
    current = aggregate(sources(root))
    revision = git(root, 'merge-base', base, 'HEAD').decode().strip()
    base_sources = aggregate(sources(root, revision))
    older = previous(root, revision)
    if older is None:
        older = base_sources
    # Transfers require actual source removals, not a rebaseline allowance.
    changes = owner_changes(current, base_sources)
    base_sources = approved_base(base_sources, revision)
    older = approved_base(older, revision)
    directory = root / INVENTORY
    paths = sorted(directory.glob('access-*.json'))
    ceiling = decode(p.read_text(encoding='utf-8') for p in paths) if paths else Counter()
    errors, transfers = [], []

    def compare(counts, limits, label):
        failures, moves = transferred_growth(counts, limits, label, changes)
        errors.extend(failures)
        transfers.extend(moves)

    compare(current, base_sources, 'base source counters')
    compare(current, older, 'base ceilings')
    if not write:
        compare(current, ceiling, 'checked ceilings')
        compare(ceiling, older, 'ceiling increase')
    if write and not errors:
        # Only matched source transfers can redistribute existing ceilings.
        if paths:
            compare(current, ceiling, 'checked ceilings')
        if not errors:
            outputs = documents(current)
            if set(p.name for p in paths) - outputs.keys():
                raise ValueError('shard count decreased; remove obsolete exact paths explicitly')
            for name, data in outputs.items():
                (directory / name).write_text(data, encoding='utf-8', newline='\n')
    for transfer in sorted(set(transfers)):
        print(transfer)
    return errors, current


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=ROOT)
    parser.add_argument('--base', default='origin/main')
    parser.add_argument('--write', action='store_true', help='Lower ceilings or record matched owner transfers')
    args = parser.parse_args()
    try:
        errors, current = check(args.root.resolve(), args.base, args.write)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'computed inventory: {error}', file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    print(f'computed inventory: {len(errors)} errors; {len(current)} field/owner/access aggregates')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())
