"""Preserve an upper compatibility module while recovering lower ownership.

A facade may wrap a lower module to retain constructors owned by the facade.
Only a direct root alias to a declared upper file with an explicit lower glob
can stand in for the original lower declaration. Unknown forms stay rejected.
"""
import re

from rewrite_paths import lexemes
from module_sources import DECL, code_mask


def top_uses(source):
    words = [item for item in lexemes(source) if item[0] is not None]
    depth = 0
    for index, (word, _, _, _) in enumerate(words):
        if word == 'use' and depth == 0:
            stop = index + 1
            while stop < len(words) and words[stop][0] != ';':
                stop += 1
            if stop < len(words):
                yield words[index:stop + 1], index > 0 and words[index - 1][0] == 'pub'
        depth += (word == '{') - (word == '}')


def recover(source, sources, owners):
    aliases, changes = Aliases(), []
    for words, _ in top_uses(source):
        tokens = [item[0] for item in words]
        if len(tokens) != 7 or tokens[:3] != ['use', 'crate', '::']:
            continue
        _, _, _, upper, sep, root, end = tokens
        target = owners.get(root)
        if (sep != '::' or end != ';' or target in (None, 'kamin-html')
                or owners.get(upper) != 'kamin-html'):
            continue
        candidates = [name for name in (f'{upper}/{root}.rs', f'{upper}/{root}/mod.rs')
                      if name in sources]
        if len(candidates) != 1:
            raise ValueError(f'Facade compatibility alias needs one module: {root}')
        expected = ['use', target.replace('-', '_'), '::', root, '::', '*', ';']
        if not any(public and [item[0] for item in use] == expected
                   for use, public in top_uses(sources[candidates[0]])):
            raise ValueError(f'Facade compatibility module must forward its lower root: {root}')
        start, stop = words[0][2], words[-1][3]
        if re.search(r'/[/*]', source[start:stop]):
            raise ValueError('Comment inside facade compatibility alias requires preservation')
        if root in aliases:
            raise ValueError(f'Duplicate facade compatibility alias: {root}')
        aliases[root] = source[start:stop]
        changes.append((start, stop, f'mod {root};'))
    mask = code_mask(source)
    depth = 0
    root_matches = []
    for match in DECL.finditer(mask):
        # DECL inside nested bodies is excluded by a brace-depth scan.
        prefix = mask[:match.start()]
        if prefix.count('{') == prefix.count('}') and match['end'] == ';':
            root_matches.append(match)
    for match in root_matches:
        root = match['name']
        target = owners.get(root)
        if target in (None, 'kamin-html'):
            continue
        raw = source[match.start():match.end()]
        attr = re.search(r'#\[path = "([^"\\]+)"\]', raw)
        if attr is None:
            continue
        filename = attr[1]
        candidates = [(upper, filename) for upper, owner in owners.items()
                      if owner == 'kamin-html' and filename in
                      (f'{upper}/{root}.rs', f'{upper}/{root}/mod.rs')]
        if len(candidates) != 1:
            raise ValueError(f'Unsupported facade module path: {filename}')
        upper, filename = candidates[0]
        expected = ['use', target.replace('-', '_'), '::', root, '::', '*', ';']
        if not any(public and [item[0] for item in use] == expected
                   for use, public in top_uses(sources[filename])):
            raise ValueError(f'Facade compatibility module must forward its lower root: {root}')
        if root in aliases:
            raise ValueError(f'Duplicate facade compatibility alias: {root}')
        parent = f'{upper}/mod.rs'
        if parent not in sources:
            raise ValueError(f'Facade module requires its upper parent: {parent}')
        marker = f'\n// Virtual ownership declaration; never emitted.\nmod {root};\n'
        sources[parent] += marker
        aliases.overlays[parent] = marker
        aliases.declarations.add(root)
        aliases[root] = raw
        changes.append((match.start(), match.end(), f'{match["vis"] or ""}mod {root};'))
    for start, stop, replacement in sorted(changes, reverse=True):
        source = source[:start] + replacement + source[stop:]
    return source, aliases


class Aliases(dict):
    """Virtual upper declarations are removed before any generated file is emitted."""
    def __init__(self):
        super().__init__()
        self.overlays = {}
        self.declarations = set()

    def restore(self, filename, source):
        marker = self.overlays.get(filename)
        if marker:
            if not source.endswith(marker):
                raise ValueError(f'Virtual facade declaration changed: {filename}')
            return source[:-len(marker)]
        return source
