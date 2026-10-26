"""Dependency direction proofs for opaque types used by the computed inventory."""
from functools import lru_cache
import json
from pathlib import Path
import subprocess


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
    return {dep['name'] for dep in nodes[html[0]]['deps']
            if dep['pkg'] in nodes and dep['pkg'] not in blocked}


@lru_cache(maxsize=1)
def external_crates():
    try:
        result = subprocess.run(['cargo', 'metadata', '--format-version', '1', '--offline'],
                                cwd=Path(__file__).resolve().parents[1],
                                capture_output=True, text=True, check=True)
        return unrelated_crates(json.loads(result.stdout))
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError):
        # Unavailable metadata removes exemptions; it never disables the guard.
        return set()
