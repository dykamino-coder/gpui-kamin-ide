"""Resolve explicit imports/reexports to defining modules, without rustc.

All cfg branches are considered. Macro expansion, macro-generated names and
include! files are NOT resolved; explicit paths in macro bodies are still scanned
by rust_layer_paths. This is a lexical guard, not a substitute for Rust analysis.
"""

from collections import defaultdict

from rust_layer_paths import IDENT, absolute, file_module, tokens, use_tree


def public(words, index):
    """Visibility before an item, including pub(in ...) and fn modifiers."""
    index -= 1
    while index >= 0 and words[index][0] in ('async', 'unsafe', 'const', 'extern'):
        index -= 1
    if index >= 0 and words[index][0] == ')':
        depth = 1
        index -= 1
        while index >= 0 and depth:
            depth += (words[index][0] == ')') - (words[index][0] == '(')
            index -= 1
    return index >= 0 and words[index][0] == 'pub'


class Exports:
    def __init__(self, sources):
        self.aliases = defaultdict(set)
        self.globs = defaultdict(set)
        self.public_globs = set()
        self.exported = set()
        self.modules = set()
        for filename, source in sources.items():
            self.read(source, file_module(filename))
        self.explicit = set(self.aliases)
        self.expand_globs()

    def expand_globs(self):
        changed = True
        while changed:
            changed = False
            names = self.exported.copy()
            for module, targets in self.globs.items():
                for target in targets:
                    for resolved in self.resolve(target):
                        for name in names:
                            if len(name) == len(resolved) + 1 and name[:-1] == resolved:
                                alias = module + name[-1:]
                                if alias in self.explicit:
                                    continue  # An explicit import shadows glob imports.
                                if alias != name and name not in self.aliases[alias]:
                                    self.aliases[alias].add(name)
                                    if (module, target) in self.public_globs:
                                        self.exported.add(alias)
                                    changed = True

    def read(self, source, module):
        words = list(tokens(source))
        scopes, module_scope, index = [], True, 0
        while index < len(words):
            word = words[index][0]
            visible = public(words, index)
            if module_scope and word == 'use':
                leaves, index = use_tree(words, index + 1, aliases=True)
                for path, _, alias in leaves:
                    target = absolute(path, module, ())
                    if alias == '*':
                        self.globs[module].add(target[:-1])
                        if visible:
                            self.public_globs.add((module, target[:-1]))
                    elif alias and alias != '_':
                        self.aliases[module + (alias,)].add(target)
                        if visible:
                            self.exported.add(module + (alias,))
                continue
            if module_scope and word in ('mod', 'fn', 'struct', 'enum', 'trait',
                                         'type', 'const', 'static', 'union'):
                if index + 1 < len(words) and IDENT.fullmatch(words[index + 1][0]):
                    if word == 'mod':
                        self.modules.add(module + (words[index + 1][0],))
                    if visible:
                        self.exported.add(module + (words[index + 1][0],))
            if word == '{':
                scopes.append((module, module_scope))
                module_scope = index >= 2 and words[index - 2][0] == 'mod'
                if module_scope:
                    module += (words[index - 1][0],)
            elif word == '}' and scopes:
                module, module_scope = scopes.pop()
            index += 1

    def resolve(self, path, seen=frozenset()):
        """Follow aliases (including module prefixes), glob chains and cycles."""
        for size in range(len(path), 0, -1):
            prefix, suffix = path[:size], path[size:]
            # Rust has separate module/type and value namespaces: a module may
            # reexport its same-named function (`mod f; pub use f::f;`).
            if suffix and prefix in self.modules:
                return {path}
            if prefix in self.aliases:
                if prefix in seen:
                    return set()
                targets = self.aliases[prefix] - {prefix}
                if not targets:
                    return {path}
                return set().union(*(
                    self.resolve(target + suffix, seen | {prefix})
                    for target in targets))
        return {path}

    def paths(self, path, layers=()):
        def defining(path, seen=frozenset()):
            if path in seen:
                return set()
            results = set()
            for resolved in self.resolve(path):
                if not resolved or resolved[0] in layers:
                    results.add(resolved)
                    continue
                # Importing a neutral namespace (including *) must not hide its
                # exported phase symbols behind a subsequent local module alias.
                module = resolved[:-1] if resolved[-1] == '*' else resolved
                children = [p for p in self.exported if p[:-1] == module]
                for child in children:
                    results.update(defining(child, seen | {path}))
            return results
        return sorted({'::'.join(p) for p in defining(tuple(path.split('::')))})
