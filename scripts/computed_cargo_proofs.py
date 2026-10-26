"""Dependency direction proofs for opaque types used by the computed inventory."""
from functools import lru_cache
from collections import defaultdict
from pathlib import Path
import tomllib


def unrelated_crates(metadata):
    """Return direct dependency aliases whose graph cannot reach kamin-html."""
    html = [p['id'] for p in metadata['packages'] if p['name'] == 'kamin-html']
    if len(html) != 1 or not metadata.get('resolve'): return set()
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    if html[0] not in nodes: return set()
    # A missing graph node cannot establish the absence of a dependency.
    blocked = {html[0]}
    blocked.update(node['id'] for node in nodes.values()
                   if any(dep['pkg'] not in nodes for dep in node['deps']))
    while True:
        more = {node['id'] for node in nodes.values()
                if any(dep['pkg'] in blocked for dep in node['deps'])}
        if more <= blocked: break
        blocked.update(more)
    aliases = defaultdict(list)
    for dep in nodes[html[0]]['deps']:
        aliases[dep['name']].append(dep['pkg'])
    return {name for name, packages in aliases.items()
            if all(package in nodes and package not in blocked for package in packages)}


def dependency_specs(manifest, workspace):
    """Include every target, dependency kind and optional feature conservatively."""
    tables = [manifest, *manifest.get('target', {}).values()]
    for table in tables:
        for kind in ('dependencies', 'dev-dependencies', 'build-dependencies'):
            for alias, specification in table.get(kind, {}).items():
                if isinstance(specification, str): specification = {'version': specification}
                if specification.get('workspace'):
                    specification = workspace.get(alias, {})
                    if isinstance(specification, str): specification = {'version': specification}
                    if not specification:
                        yield alias.replace('-', '_'), None
                        continue
                yield alias.replace('-', '_'), specification.get('package', alias)


def locked_metadata(root):
    """Read the checked-in graph, without Cargo, a registry cache or network access.

    Cargo.lock contains resolved package identities and a superset of the active
    feature/target edges. Manifests supply direct crate aliases (including
    workspace inheritance). Missing or ambiguous edges cannot establish a proof.
    """
    def read(path):
        with path.open('rb') as source: return tomllib.load(source)

    packages = read(root / 'Cargo.lock')['package']
    workspace = read(root / 'Cargo.toml').get('workspace', {}).get('dependencies', {})
    manifest = read(root / 'crates/html/Cargo.toml')
    by_name = defaultdict(list)
    records = {}
    for package in packages:
        identity = (package['name'], package['version'], package.get('source', ''))
        if identity in records: raise ValueError('duplicate locked package')
        records[identity] = package
        by_name[package['name']].append(identity)

    def resolve(specification):
        parts = specification.split(' ', 2)
        candidates = by_name[parts[0]]
        if len(parts) > 1: candidates = [p for p in candidates if p[1] == parts[1]]
        if len(parts) > 2: candidates = [p for p in candidates if p[2] == parts[2].removeprefix('(').removesuffix(')')]
        elif len(parts) == 2 and len(candidates) > 1:
            # With equal name/version, Cargo writes the source for the registry
            # edge; the unqualified edge identifies the local path package.
            local = [p for p in candidates if not p[2]]
            if len(local) == 1: candidates = local
        return candidates[0] if len(candidates) == 1 else None

    html = [p for p in records if p[0] == manifest['package']['name'] and not p[2]]
    if len(html) != 1 or html[0][0] != 'kamin-html': raise ValueError('missing HTML package')
    nodes = {identity: {'id': identity, 'deps': [
        {'name': dep.split(' ', 1)[0], 'pkg': resolve(dep)}
        for dep in package.get('dependencies', [])]}
        for identity, package in records.items()}
    direct = nodes[html[0]]['deps']
    aliases = []
    for alias, name in dependency_specs(manifest, workspace):
        matching = [dep['pkg'] for dep in direct if dep['name'] == name]
        aliases.extend({'name': alias, 'pkg': package} for package in matching or [None])
    nodes[html[0]]['deps'] = aliases
    return {'packages': [{'name': p[0], 'id': p} for p in records],
            'resolve': {'nodes': list(nodes.values())}}


@lru_cache(maxsize=1)
def external_crates():
    try:
        return unrelated_crates(locked_metadata(Path(__file__).resolve().parents[1]))
    except (OSError, ValueError, KeyError, TypeError):
        # An incomplete checked-in graph never disables the guard.
        return set()
