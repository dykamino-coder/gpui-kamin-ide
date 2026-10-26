"""Deterministic migration planning; unresolved ownership rejects the whole plan.

The input checkout must already satisfy the semantic ownership contracts.
The generated diff still requires compiler, visibility and behavioral checks.
"""
from collections import defaultdict
from dataclasses import dataclass
from pathlib import PurePosixPath
import re

from rewrite_paths import rewrite, lexemes
from canonical_exports import CanonicalExports
from module_sources import modules, code_mask, DECL
from source_relocation import source_locations, relocate_includes, PACKAGE_DIRS
from migration_test_support import widen


ROOT_OWNERS = {
    **dict.fromkeys(['defaults', 'cache', 'color', 'css_syntax', 'dom', 'style',
                    'resources', 'tolerance', 'box_facts'], 'kamin-style'),
    **dict.fromkeys(['layout', 'text', 'interactive', 'hooks'], 'kamin-layout'),
    **dict.fromkeys(['paint', 'svg'], 'kamin-paint'),
    **dict.fromkeys(['animation', 'document', 'render', 'depth_tests', 'coverage'], 'kamin-html'),
}
RANK = {'kamin-style': 0, 'kamin-layout': 1, 'kamin-paint': 2, 'kamin-html': 3}


@dataclass(frozen=True)
class Change:
    old: str
    new: str
    before: str
    after: str


@dataclass
class Plan:
    changes: list
    edges: dict
    blockers: list
    includes: list


def owner(filename):
    if filename == 'lib.rs':
        return 'kamin-html'
    root = filename.split('/')[0].removesuffix('.rs')
    if root not in ROOT_OWNERS:
        raise ValueError(f'No reviewed owner for root {root}: {filename}')
    return ROOT_OWNERS[root]


def expose_boundary(source):
    """Promote the existing crate-wide contract in a newly internal package.

    Private, pub(super) and pub(in crate::ancestor) implementation details
    remain restricted. Rewriting must keep their ancestor inside the package.
    Comments, attributes, literals and algorithm bodies are untouched.
    """
    words = [(word, start, end) for word, _, start, end in lexemes(source)
             if word is not None]
    changes = []
    for i, (word, start, _) in enumerate(words):
        if word != 'pub' or [x[0] for x in words[i + 1:i + 2]] != ['(']:
            continue
        if [x[0] for x in words[i + 1:i + 4]] == ['(', 'crate', ')']:
            if re.search(r'/[/*]', source[start:words[i + 3][2]]):
                raise ValueError('Comment inside visibility needs an explicit preservation rule')
            changes.append((start, words[i + 3][2], 'pub'))
        elif [x[0] for x in words[i + 1:i + 3]] == ['(', 'in']:
            if words[i + 3][0] not in ('crate', 'self', 'super'):
                raise ValueError('pub(in ...) ancestor escaped its package')
    for start, end, text in reversed(changes):
        source = source[:start] + text + source[end:]
    return source


def virtual_root(source, roots):
    """Recover the declaration graph when checking a tree already migrated."""
    words = [item for item in lexemes(source) if item[0] is not None]
    changes, depth = [], 0
    for i, (word, _, start, _) in enumerate(words):
        if word == '{':
            depth += 1
        elif word == '}':
            depth -= 1
        elif word == 'use' and depth == 0 and i + 4 < len(words):
            namespace, separator, name, end = [item[0] for item in words[i + 1:i + 5]]
            if (name in roots and separator == '::' and end == ';'
                    and ROOT_OWNERS.get(name, '').replace('-', '_') == namespace):
                right = words[i + 4][3]
                if re.search(r'/[/*]', source[start:right]):
                    raise ValueError('Comment inside generated facade requires preservation')
                changes.append((start, right, f'mod {name};'))
    for start, end, text in reversed(changes):
        source = source[:start] + text + source[end:]
    return source


def facade(source, declarations):
    changes = []
    mask = code_mask(source)
    for path, module in declarations.items():
        if len(path) != 1 or module.inline or module.declaration_file != 'lib.rs':
            continue
        name = path[0]
        target = ROOT_OWNERS[name]
        if target == 'kamin-html':
            continue
        match = DECL.match(mask, module.declaration_start)
        start, end = match.start('vis'), match.end()
        # A missing visibility group's span is -1; use the mod keyword instead.
        if start == -1:
            start = mask.index('mod', match.end('attrs'), match.end())
        visible = match['vis'] or ''
        changes.append((start, end, f'{visible}use {target.replace("-", "_")}::{name};'))
    for start, end, text in sorted(changes, reverse=True):
        source = source[:start] + text + source[end:]
    return source


def crate_attributes(source):
    """Retain root compilation contracts in each extracted package."""
    words = [item for item in lexemes(source) if item[0] is not None]
    result, index, depth = [], 0, 0
    while index < len(words):
        token = words[index][0]
        if depth == 0 and [item[0] for item in words[index:index + 3]] == ['#', '!', '[']:
            begin, stop, brackets = words[index][2], index + 3, 1
            while stop < len(words) and brackets:
                brackets += (words[stop][0] == '[') - (words[stop][0] == ']')
                stop += 1
            if brackets:
                raise ValueError('Unclosed root crate attribute')
            result.append(source[begin:words[stop - 1][3]])
            index = stop
            continue
        depth += (token == '{') - (token == '}')
        index += 1
    return result


def lower_root(package, declarations, source):
    lines = ['//! Внутренняя фаза HTML/CSS: явные контракты конвейера.',
             '', *crate_attributes(source), '']
    mask = code_mask(source)
    for path, module in sorted(declarations.items()):
        if len(path) != 1 or module.inline or ROOT_OWNERS.get(path[0]) != package:
            continue
        if module.declaration_file != 'lib.rs':
            raise ValueError(f'Unexpected root declaration: {path}')
        match = DECL.match(mask, module.declaration_start)
        attrs = source[match.start('attrs'):match.end('attrs')].strip()
        if attrs:
            lines.append(attrs)
        lines.append(f'pub mod {path[0]};')
    return '\n'.join(lines) + '\n'


def plan(sources, existing_files, physical=None, crate_roots=None):
    """Pure function: preflight all sources/includes/edges before any write.

    sources use original logical names. physical maps those names to their
    current physical locations, allowing --check on an already moved tree.
    existing_files contains all source/asset files, not only Rust modules.
    """
    sources = dict(sources)
    roots = {name.split('/')[0].removesuffix('.rs') for name in sources if name != 'lib.rs'}
    unknown = roots - ROOT_OWNERS.keys()
    if unknown:
        raise ValueError(f'Unassigned roots: {sorted(unknown)}')
    originals = dict(sources)
    sources = {name: source.replace('\r\n', '\n') for name, source in sources.items()}
    original_root = originals['lib.rs']
    sources['lib.rs'] = virtual_root(sources['lib.rs'], roots)
    declarations, file_modules = modules(sources)
    if set(file_modules) != set(sources):
        raise ValueError(f'Undeclared implementation files: {sorted(set(sources) - file_modules.keys())}')
    exports = CanonicalExports(sources, file_modules)
    sources = widen(sources, declarations, file_modules, exports, ROOT_OWNERS)
    # Widening changes attribute spans, so declaration positions must come from
    # the rewritten text rather than the original preflight offsets.
    declarations, file_modules = modules(sources)
    exports = CanonicalExports(sources, file_modules)

    def resolve(path):
        if path and path[0] in ROOT_OWNERS:
            return path
        targets = exports.resolve(tuple(path))
        if len(targets) != 1:
            raise ValueError(f'Ambiguous facade alias: {path}: {targets}')
        return next(iter(targets))

    destinations = source_locations(sources, ROOT_OWNERS)
    physical = physical or {name: 'crates/html/src/' + name for name in sources}
    locations = {physical[name]: destinations['crates/html/src/' + name] for name in sources}
    changes, edges, includes = [], defaultdict(list), []
    package_tokens = {name.replace('-', '_'): name for name in RANK}
    for filename, source in sorted(sources.items()):
        package = owner(filename)
        if filename == 'lib.rs':
            after = facade(source, declarations)
        else:
            after = rewrite(source, filename, package, ROOT_OWNERS, resolve,
                            module_path=file_modules[filename])
            if package != 'kamin-html':
                after = expose_boundary(after)
        after, targets = relocate_includes(after, physical[filename], locations, existing_files)
        includes.extend((filename, old, new) for old, new in targets)
        tokens = [token for token in lexemes(after) if token[0] is not None]
        for i, (word, line, _, _) in enumerate(tokens):
            if word in package_tokens and i + 1 < len(tokens) and tokens[i + 1][0] == '::':
                target = package_tokens[word]
                if package != target:
                    edges[(package, target)].append((filename, line))
        before = originals[filename]
        new = locations[physical[filename]]
        if physical[filename] != new or before != after:
            changes.append(Change(physical[filename], new, before, after))
    for package in sorted(RANK):
        if package == 'kamin-html':
            continue
        new = f'crates/{PACKAGE_DIRS[package]}/src/lib.rs'
        after = lower_root(package, declarations, sources['lib.rs'])
        before = (crate_roots or {}).get(package, '')
        if before != after:
            changes.append(Change(new, new, before, after))
    blockers = [dict(source=source, target=target, sites=sites)
                for (source, target), sites in sorted(edges.items())
                if RANK[target] > RANK[source]]
    # A downward path graph alone does not permit foreign inherent impls.
    # Audit explicit nominal heads before writes; aliases/macros/generic orphan
    # cases remain compiler/manual work, never a claimed coherence proof.
    from impl_ownership import audit
    ownership = audit(sources, owner, file_modules, exports)
    for item in ownership['impls']:
        if item['status'] in ('inherent-foreign-owner', 'trait-and-self-foreign-owner'):
            blockers.append(dict(kind=item['status'], source=item['package'],
                                 target=item['target']['package'],
                                 sites=[(item['file'], item['line'])], header=item['header']))
    return Plan(changes, dict(edges), blockers, includes)
