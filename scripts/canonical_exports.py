"""Use declared module identities with the existing explicit-export parser."""
from collections import defaultdict

from rust_layer_exports import Exports
from rust_layer_paths import file_module


class CanonicalExports(Exports):
    def __init__(self, sources, file_modules):
        self.declared_modules = {file_module(name): module
                                 for name, module in file_modules.items()}
        if len(self.declared_modules) != len(file_modules):
            raise ValueError('Physical module identities collide')
        super().__init__(sources)

    def read(self, source, module):
        super().read(source, self.declared_modules[module])

    def expand_globs(self):
        # A declared public item shadows a glob, just as an explicit import
        # does. Otherwise `fields::{Computed}; use super::*` creates a false
        # Computed -> fields::Computed -> Computed alias cycle.
        self.explicit.update(self.exported - self.aliases.keys())
        changed = True
        while changed:
            changed = False
            # Retain the same per-round snapshot as Exports.expand_globs, but
            # index immediate children instead of rescanning every exported
            # symbol for every glob target. Newly exported aliases become
            # visible only in the next round.
            children = defaultdict(list)
            for name in self.exported.copy():
                children[name[:-1]].append(name)
            for module, targets in self.globs.items():
                for target in targets:
                    for resolved in self.resolve(target):
                        for name in children.get(resolved, ()):
                            alias = module + name[-1:]
                            if alias in self.explicit:
                                continue
                            if alias != name and name not in self.aliases[alias]:
                                self.aliases[alias].add(name)
                                if (module, target) in self.public_globs:
                                    self.exported.add(alias)
                                changed = True
