"""Local type bindings for explicit impl checks, separate from public exports."""
from collections import defaultdict
from rust_layer_exports import Exports


class ImplBindings(Exports):
    def __init__(self, exports, declarations):
        # Never mutate the export model reused by the migration path planner.
        self.aliases = defaultdict(set, {k: set(v) for k, v in exports.aliases.items()})
        self.globs = {k: set(v) for k, v in exports.globs.items()}
        self.public_globs = set(exports.public_globs)
        self.exported, self.modules = set(exports.exported), set(exports.modules)
        self.explicit = set(exports.explicit)
        self.declared = set(declarations)
        self.ambiguous = {p for p in self.declared if p in self.aliases
                          and p in self.explicit and self.aliases[p] - {p}}
        for path in self.declared - self.explicit:
            # A private declared type shadows a glob in its own module too.
            self.aliases.pop(path, None)
        self.explicit.update(self.declared)
        self.expand_globs()

    def expand_globs(self):
        changed = True
        while changed:
            changed = False
            public = set(self.exported)
            local = self.declared | set(self.aliases)
            public_children, local_children = defaultdict(list), defaultdict(list)
            for name in public:
                public_children[name[:-1]].append(name)
            for name in public | local:
                local_children[name[:-1]].append(name)
            for module, targets in self.globs.items():
                for target in targets:
                    for resolved in self.resolve(target):
                        # Private parent bindings are visible to descendants.
                        # Do not publish them or grant access to sibling scopes.
                        children = (local_children if module[:len(resolved)] == resolved
                                    else public_children)
                        for name in children.get(resolved, ()):
                            alias = module + name[-1:]
                            if alias in self.explicit or alias == name:
                                continue
                            if name not in self.aliases[alias]:
                                self.aliases[alias].add(name)
                                if (module, target) in self.public_globs:
                                    self.exported.add(alias)
                                changed = True

    def resolve(self, path, seen=frozenset()):
        result = super().resolve(path, seen)
        if any(path[:size] in self.ambiguous for size in range(1, len(path) + 1)):
            # All cfg branches may contain both a declaration and an explicit
            # import. Keep an unknown alternative instead of selecting either.
            result.add(('__impl_binding_requires_cfg_review__',) + path)
        return result
