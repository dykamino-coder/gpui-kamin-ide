"""Conservative receiver type facts for mutable style escapes.

Unknown receivers remain guarded. Only explicit Rust types prove that a call
belongs to unrelated state; names such as cache/state never grant exemptions.
"""
from collections import defaultdict
from rust_layer_paths import file_module, use_tree
from computed_cargo_proofs import external_crates

STYLE = {'Computed', 'InheritedStyle', 'SharedStyle', 'BoxStyle'}
PRIMITIVES = set('bool char str String usize isize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64'.split())
WRAPPERS = set('Rc Arc Box Pin Cell RefCell Ref RefMut Option Result Vec VecDeque HashMap HashSet BTreeMap BTreeSet SmallVec Cow Mutex RwLock'.split())
STANDARD_PATHS = {('std', module, name): name for module, group in (
    ('rc', 'Rc'), ('sync', 'Arc Mutex RwLock'), ('cell', 'RefCell Ref RefMut Cell'),
    ('boxed', 'Box'), ('pin', 'Pin'), ('vec', 'Vec'), ('option', 'Option'),
    ('result', 'Result'), ('string', 'String'), ('collections', 'VecDeque HashMap HashSet BTreeMap BTreeSet'),
) for name in group.split()}


class DeclaredType(tuple):
    """Keep the declaration's module when projecting across source files."""
    def __new__(cls, value, path):
        result = super().__new__(cls, value)
        result.path = getattr(value, 'path', path)
        return result


class TypeTokens(list):
    """Cache repeated delimiter scans within a source file."""
    def __init__(self, words):
        super().__init__(words)
        self.ends = {}


def closing(words, start):
    cache = getattr(words, 'ends', None)
    if cache is not None and start in cache: return cache[start]
    pairs = {'(': ')', '[': ']', '{': '}', '<': '>'}
    stack = []
    for i in range(start, len(words)):
        word = words[i]
        if word in pairs:
            stack.append(pairs[word])
        elif stack and word == stack[-1]:
            stack.pop()
            if not stack:
                if cache is not None: cache[start] = i
                return i
    if cache is not None: cache[start] = len(words)
    return len(words)


def until(words, start, stops):
    i = start
    while i < len(words) and words[i] not in stops:
        i = closing(words, i) + 1 if words[i] in ('(', '[', '{', '<') else i + 1
    return i


def names(words):
    return [word for i, word in enumerate(words) if word.isidentifier()
            and word not in ('mut', 'const', 'dyn', 'impl')
            and (i + 1 == len(words) or words[i + 1] != '::')
            and (not i or words[i - 1] != "'")]


class ReceiverTypes:
    def __init__(self, streams, external=None):
        streams = {path: TypeTokens(words) for path, words in streams.items()}
        self.streams = streams
        self.external = external_crates() if external is None else external
        self.import_paths, self.glob_paths = defaultdict(list), defaultdict(list)
        self.locations = defaultdict(set)
        self.module_files = defaultdict(set)
        for path in streams:
            self.module_files[('crate', *file_module(path.removeprefix('crates/html/src/')))].add(path)
        self.modules = defaultdict(set)
        self.structs = set()
        self.enums = set()
        self.trait_methods = set()
        self.nominal, self.aliases = set(), defaultdict(list)
        self.payload_nominal = set()
        self.local_nominal, self.local_aliases = defaultdict(set), defaultdict(set)
        self.local_alias_values = defaultdict(list)
        self.inline_modules = {}
        self.foreign_globs = set()
        self.field_files = defaultdict(set)
        self.indirections = {'Deref', 'DerefMut', 'Borrow', 'BorrowMut'}
        trait_impls = []
        self.generics = set()
        self.opaque = set()
        self.indirect_nominals = set()
        self.deref_nominals = set()
        self.qualified = set()
        self.fields, self.bindings, self.impls = defaultdict(list), {}, {}
        self.declared_fields = defaultdict(list)
        self.enum_payloads = defaultdict(list)
        self.type_parameters = {}
        self.deref_targets = defaultdict(list)
        self.default_types = set()
        self.clone_types = set()
        for path, words in streams.items():
            inline = [(i + 2, closing(words, i + 2)) for i, word in enumerate(words[:-2])
                      if word == 'mod' and words[i + 1].isidentifier() and words[i + 2] == '{']
            self.inline_modules[path] = inline
            depth, module_level = 0, set()
            for i, word in enumerate(words):
                if depth == 0: module_level.add(i)
                depth += (word == "{") - (word == "}")
            self.bindings[path] = self._bindings(words)
            self.impls[path] = []
            for i, word in enumerate(words[:-1]):
                if i in module_level and word == 'mod': self.modules[path].add(words[i + 1])
                if word == 'trait':
                    at = until(words, i + 1, {'{', ';'})
                    if words[at:at + 1] == ['{']:
                        self.trait_methods.update(words[j + 1] for j in range(at, closing(words, at) - 1)
                                                  if words[j] == 'fn')
                # A generic parameter may shadow a nominal type in another file.
                generic = i + (1 if word == 'impl' else 2)
                if word in ('impl', 'fn', 'struct', 'enum', 'union', 'trait', 'type') and words[generic:generic + 1] == ['<']:
                    end = closing(words, generic)
                    start = generic + 1
                    while start < end:
                        stop = until(words, start, {',', '>'})
                        parameter = words[start:stop]
                        if parameter and parameter[0].isidentifier():
                            self.generics.add(parameter[1] if parameter[0] == 'const' and len(parameter) > 1 else parameter[0])
                        start = stop + 1
                if word in ('struct', 'enum', 'union'):
                    name = words[i + 1]
                    if i in module_level:
                        prefix = i - 1
                        while prefix >= 0 and words[prefix] not in (';', '}', '{'): prefix -= 1
                        attributes = words[prefix + 1:i]
                        if 'derive' in attributes and 'Default' in attributes: self.default_types.add((path, name))
                        if 'derive' in attributes and 'Clone' in attributes: self.clone_types.add((path, name))
                    self.nominal.add(name)
                    if i in module_level:
                        self.local_nominal[path].add(name)
                        self.qualified.add(('crate', *file_module(path.removeprefix('crates/html/src/')), name))
                        self.locations[('crate', *file_module(path.removeprefix('crates/html/src/')), name)].add(path)
                        if word == 'struct': self.structs.add((path, name))
                    at = until(words, i + 2, {'{', '(', ';'})
                    # A nominal name alone does not prove its fields/variants unrelated.
                    if at < len(words) and words[at] in ('{', '(') and closing(words, at) > at + 1:
                        self.payload_nominal.add(name)
                    if at < len(words) and words[at] == '{' and word == 'enum':
                        end, j = closing(words, at), at + 1
                        if '!' not in words[at:end]: self.enums.add((path, name))
                        while j < end:
                            stop = until(words, j, {',', '}'})
                            part = words[j:stop]
                            payload = next((k for k, token in enumerate(part) if token in ('(', '{')), None)
                            if payload is not None:
                                last, first = closing(part, payload), payload + 1
                                while first < last:
                                    finish = until(part, first, {',', ')', '}'})
                                    value = part[first:finish]
                                    if part[payload] == '{':
                                        value = value[value.index(':') + 1:] if ':' in value else []
                                    self.enum_payloads[path, name].append(tuple(value))
                                    first = finish + 1
                            j = stop + 1
                    elif at < len(words) and words[at] == '{':
                        end = closing(words, at)
                        j = at + 1
                        while j < end:
                            stop = until(words, j, {',', '}'})
                            part = words[j:stop]
                            if ':' in part:
                                colon = part.index(':')
                                if colon and part[colon - 1].isidentifier():
                                    self.fields[name, part[colon - 1]].append(tuple(part[colon + 1:]))
                                    self.field_files[name, part[colon - 1]].add(path)
                                    self.declared_fields[path, name, part[colon - 1]].append(tuple(part[colon + 1:]))
                            j = stop + 1
                    elif at < len(words) and words[at] == '(':
                        end, j, index = closing(words, at), at + 1, 0
                        while j < end:
                            stop = until(words, j, {',', ')'})
                            part = words[j:stop]
                            if part[:1] == ['pub']:
                                part = part[1:]
                                if part[:1] == ['(']: part = part[closing(part, 0) + 1:]
                            self.fields[name, str(index)].append(tuple(part))
                            self.field_files[name, str(index)].add(path)
                            self.declared_fields[path, name, str(index)].append(tuple(part))
                            j, index = stop + 1, index + 1
                if word == 'type' and words[i + 1].isidentifier():
                    end = until(words, i + 2, {';', '{'})
                    part = words[i + 2:end]
                    if '=' in part:
                        self.aliases[words[i + 1]].append(tuple(part[part.index('=') + 1:]))
                        if i in module_level:
                            self.local_aliases[path].add(words[i + 1])
                            self.local_alias_values[path, words[i + 1]].append(tuple(part[part.index('=') + 1:]))
                if word in STYLE and words[i + 1:i + 2] == ['as'] and i + 2 < len(words):
                    self.aliases[words[i + 2]].append((word,))
                    if i in module_level:
                        self.local_aliases[path].add(words[i + 2])
                        self.local_alias_values[path, words[i + 2]].append((word,))
                if word in self.indirections and words[i + 1:i + 2] == ['as'] and i + 2 < len(words):
                    self.indirections.add(words[i + 2])
                # A standard import can rename another type to Rc/RefCell.
                if word == 'as' and words[i + 1] in WRAPPERS:
                    self.opaque.add(words[i + 1])
                if word == 'use' and words[i + 1] != '<':
                    end = until(words, i + 1, {';'})
                    part = words[i + 1:end]
                    if i in module_level:
                        leaves, _ = use_tree([(w, 0) for w in words], i + 1, aliases=True)
                        for imported, _, alias in leaves:
                            imported = self._absolute(path, imported)
                            if imported and imported[-1] == '*': self.glob_paths[path].append(imported[:-1])
                            elif imported: self.import_paths[path, alias].append(imported)
                    if part[:1] not in (['std'], ['core'], ['alloc']):
                        self.opaque.update(names(part))
                        if '*' in part: self.foreign_globs.add(path)
                if word == 'impl' and i in module_level:
                    at = until(words, i + 1, {'{', ';'})
                    if at >= len(words) or words[at] != '{': continue
                    head = words[i + 1:at]
                    if head[:1] == ['<']: head = head[closing(head, 0) + 1:]
                    if 'for' in head:
                        trait, head = head[:head.index('for')], head[head.index('for') + 1:]
                        trait_impls.append((path, trait, head, at, closing(words, at)))
                    if 'where' in head: head = head[:head.index('where')]
                    body_end = closing(words, at)
                    self.impls[path].append((at, body_end, tuple(head)))
                    if any(words[j:j + 2] in (['fn', 'borrow_mut'], ['fn', 'get_mut'], ['fn', 'make_mut'])
                           for j in range(at + 1, body_end)):
                        self.opaque.update(names(head))
                        self.indirect_nominals.update(names(head))
                if word in ('struct', 'enum', 'type') and i in module_level:
                    parameters = []
                    if words[i + 2:i + 3] == ['<']:
                        start, end = i + 3, closing(words, i + 2)
                        while start < end:
                            stop = until(words, start, {',', '>'})
                            part = words[start:stop]
                            parameters.append(part[0] if part and part[0].isidentifier() and part[0] != 'const' else None)
                            start = stop + 1
                    self.type_parameters[path, words[i + 1]] = parameters
        for path, trait, head, at, end in trait_impls:
            if any(name in self.indirections for name in names(trait)):
                self.deref_nominals.update(names(head))
                if names(trait)[-1:] in (['Deref'], ['DerefMut']):
                    owner = self._resolve(path, tuple(head[::2]))
                    if owner is not None and owner[0] == 'local':
                        for j in range(at + 1, end - 2):
                            if streams[path][j:j + 3] == ['type', 'Target', '=']:
                                stop = until(streams[path], j + 3, {';'})
                                self.deref_targets[owner[1], owner[2]].append(DeclaredType(streams[path][j + 3:stop], path))
            if any(name in self.indirections or name in self.opaque for name in names(trait)):
                self.opaque.update(names(head))
                self.indirect_nominals.update(names(head))
        self.resolution_cache = {}
        self.method_names = defaultdict(set)
        for file, impls in self.impls.items():
            for start, end, owner in impls:
                resolution = self._resolve(file, tuple(owner[::2]))
                if resolution is not None and resolution[0] == 'local':
                    self.method_names[resolution[1], resolution[2]].update(
                        streams[file][j + 1] for j in range(start, end - 1) if streams[file][j] == 'fn')
        self.kind_cache = {}
        self.proofs = defaultdict(list)
        for path, words in streams.items():
            self._parameter_proofs(path, words)
            self._closure_proofs(path, words)
            self._local_proofs(path, words)
            self._entry_proofs(path, words)

    def _parameter_proofs(self, path, words):
        for i, word in enumerate(words):
            if word != 'fn': continue
            at = until(words, i + 1, {'(', ';', '{'})
            if words[at:at + 1] != ['(']: continue
            end, start = closing(words, at), at + 1
            body = until(words, end + 1, {'{', ';'})
            if words[body:body + 1] != ['{']: continue
            while start < end:
                stop = until(words, start, {',', ')'})
                parameter = words[start:stop]
                if parameter[:1] == ['mut']: parameter = parameter[1:]
                if len(parameter) > 2 and parameter[0].isidentifier() and parameter[1] == ':':
                    self.proofs[path, parameter[0]].append((body + 1, closing(words, body), tuple(parameter[2:])))
                start = stop + 1

    def _local_proofs(self, path, words):
        for i, word in enumerate(words):
            if word != 'let': continue
            at = i + 1 + (words[i + 1:i + 2] == ['mut'])
            if at + 2 >= len(words) or not words[at].isidentifier(): continue
            end = until(words, at + 1, {';'})
            part = words[at + 1:end]
            if not part or '=' not in part: continue
            equal = part.index('=')
            if part[0] == ':': values = [tuple(part[1:equal])]
            elif equal == 0:
                expression = part[1:]
                clone = expression[-4:] == ['.', 'clone', '(', ')']
                if clone: expression = expression[:-4]
                values = self.receiver(path, expression, i)
                if expression[-4:] == ['::', 'default', '(', ')'] and 'default' not in self.trait_methods:
                    target = expression[:-4]
                    resolution = self._resolve(path, tuple(target[::2]))
                    if resolution is not None and resolution[0] == 'local' and (resolution[1], resolution[2]) in self.default_types:
                        if 'default' not in self.method_names[resolution[1], resolution[2]]:
                            values = [tuple(target)]
                if clone and ('clone' in self.trait_methods or not all(self._clone_preserves(v, path) for v in values)): continue
            else: continue
            if not values or any(not value for value in values): continue
            braces = [j for j in range(i) if words[j] == '{' and closing(words, j) > end]
            if not braces: continue
            for value in values:
                self.proofs[path, words[at]].append((end + 1, closing(words, braces[-1]), DeclaredType(value, path)))

    def _clone_preserves(self, value, path):
        path = getattr(value, 'path', path)
        standard = self._standard(value, path)
        if standard is not None and standard[:1] in (['Rc'], ['Arc'], ['Vec']): return True
        value = list(value)
        while value[:1] in (['&'], ['mut']): value = value[1:]
        resolution = self._resolve(path, tuple(value[::2]))
        if resolution is None or resolution[0] != 'local' or (resolution[1], resolution[2]) not in self.clone_types: return False
        # An inherent clone may take precedence over the derived Clone method.
        return 'clone' not in self.method_names[resolution[1], resolution[2]]

    def _closure_proofs(self, path, words):
        """Only an unambiguous inherent method supplies a callback signature."""
        methods = defaultdict(list)
        for i, word in enumerate(words[:-2]):
            if word != 'fn': continue
            at = until(words, i + 2, {'(', ';', '{'})
            if at == len(words) or words[at] != '(': continue
            owners = [owner for start, end, owner in self.impls[path] if start < i < end]
            if len(owners) != 1: continue
            parameters, start, end = [], at + 1, closing(words, at)
            while start < end:
                stop = until(words, start, {',', ')'})
                parameters.append(words[start:stop])
                start = stop + 1
            callbacks = {}
            # The receiver is not a call argument. Associated calls stay unknown.
            if not parameters or 'self' not in parameters[0]: continue
            for argument, parameter in enumerate(parameters[1:]):
                colon = until(parameter, 0, {':'})
                signature = parameter[colon + 1:]
                if signature[:2] != ['impl', 'FnOnce'] or signature[2:3] != ['(']: continue
                end, start, types = closing(signature, 2), 3, []
                while start < end:
                    stop = until(signature, start, {',', ')'})
                    types.append(tuple(signature[start:stop]))
                    start = stop + 1
                callbacks[argument] = types
            methods[owners[0], words[i + 1]].append(callbacks)
        for i, word in enumerate(words[:-2]):
            if word != 'self' or words[i + 1] != '.': continue
            name = words[i + 2]
            if words[i + 3:i + 4] != ['(']: continue
            owners = [owner for start, end, owner in self.impls[path] if start < i < end]
            if len(owners) != 1: continue
            signatures = methods[owners[0], name]
            if len(signatures) != 1: continue
            end, start, argument = closing(words, i + 3), i + 4, 0
            while start < end:
                stop = until(words, start, {',', ')'})
                types = signatures[0].get(argument)
                first = start + (words[start:start + 1] == ['move'])
                if types is not None and words[first:first + 1] == ['|']:
                    bar = first + 1
                    while bar < stop and words[bar] != '|': bar += 1
                    parameters = words[first + 1:bar]
                    # Typed and destructured parameters already fail closed.
                    simple = parameters[::2]
                    if (bar < stop and parameters[1::2] == [','] * (len(simple) - 1)
                            and len(simple) == len(types) and all(p.isidentifier() for p in simple)):
                        for parameter, value in zip(simple, types):
                            self.proofs[path, parameter].append((bar + 1, stop, value))
                start, argument = stop + 1, argument + 1

    def _entry_proofs(self, path, words):
        """Infer a local only from a standard map's checked entry operation."""
        for i, word in enumerate(words):
            if word != 'let': continue
            at = i + 1 + (words[i + 1:i + 2] == ['mut'])
            if at + 2 >= len(words) or not words[at].isidentifier() or words[at + 1] != '=': continue
            end = until(words, at + 2, {';'})
            expression = words[at + 2:end]
            if 'first_entry' not in expression and 'last_entry' not in expression: continue
            values = self.receiver(path, expression, i)
            if not values or any(value[:1] != ['@map-entry'] for value in values): continue
            braces = [j for j in range(i) if words[j] == '{' and closing(words, j) > end]
            if not braces: continue
            scope_end = closing(words, braces[-1])
            for value in values:
                self.proofs[path, words[at]].append((end + 1, scope_end, DeclaredType(value, path)))

    @staticmethod
    def _bindings(words):
        result = defaultdict(list)
        closure_end = -1
        for i, word in enumerate(words):
            # Parameter, typed closure parameter or explicitly annotated local.
            if word == ':' and i and words[i - 1].isidentifier():
                end = until(words, i + 1, {',', ')', '=', ';', '{', '|'})
                result[words[i - 1]].append(tuple(words[i + 1:end]))
            if word == 'fn':
                at = until(words, i + 1, {'(', ';', '{'})
                if at < len(words) and words[at] == '(':
                    end, start = closing(words, at), at + 1
                    while start < end:
                        stop = until(words, start, {',', ')'})
                        parameter = words[start:stop]
                        colon = until(parameter, 0, {':'})
                        pattern = parameter[:colon]
                        simple = [part for part in pattern if part not in ('mut', 'ref')]
                        if len(simple) != 1 or not simple[0].isidentifier():
                            for name in names(pattern): result[name].append(())
                        start = stop + 1
            if word == 'let':
                start = i + 1
                while start < len(words) and words[start] in ('mut', 'ref'): start += 1
                end = until(words, start, {'=', ';'})
                pattern = words[start:end]
                if len(pattern) >= 2 and pattern[1] == ':': continue
                # Untyped bindings, including destructuring, shadow prior facts.
                for name in names(pattern): result[name].append(())
            if word == 'for':
                end = until(words, i + 1, {'in', '{'})
                if end < len(words) and words[end] == 'in':
                    for name in names(words[i + 1:end]): result[name].append(())
            if word == '|' and i > closure_end:
                end = i + 1
                while end < len(words) and words[end] != '|': end += 1
                if end < len(words):
                    closure_end = end
                    for name in names(words[i + 1:end]): result[name].append(())
            if words[i:i + 2] == ['=', '>']:
                start = i - 1
                while start >= 0 and words[start] not in ('{', '}', ',', '=>', ';'): start -= 1
                for name in names(words[start + 1:i]): result[name].append(())
        return result

    def _qualified(self, value, proven=None):
        value, resolved, i = list(value), [], 0
        while i < len(value):
            if i + 1 < len(value) and value[i + 1] == '::':
                end = i + 2
                while end + 1 < len(value) and value[end + 1] == '::': end += 2
                path = tuple(value[i:end + 1:2])
                if path not in self.qualified: return None
                if proven is not None: proven.add(path[-1])
                resolved.append(path[-1]); i = end + 1
            else:
                resolved.append(value[i]); i += 1
        return resolved

    def _absolute(self, path, imported):
        imported = tuple(imported)
        module = file_module(path.removeprefix('crates/html/src/'))
        if imported[:1] == ('self',): return ('crate', *module, *imported[1:])
        if imported[:1] == ('super',):
            parents = 0
            while imported[parents:parents + 1] == ('super',): parents += 1
            if parents > len(module): return ()
            return ('crate', *module[:len(module) - parents], *imported[parents:])
        if imported and imported[0] in self.modules[path]: return ('crate', *module, *imported)
        return imported

    def _resolve(self, path, parts, seen=(), exports=False):
        cache = getattr(self, 'resolution_cache', None)
        if cache is not None and not seen:
            key = (path, tuple(parts), exports)
            if key not in cache: cache[key] = self._resolve_inner(path, parts, exports=exports)
            return cache[key]
        return self._resolve_inner(path, parts, seen, exports)

    def _resolve_inner(self, path, parts, seen=(), exports=False):
        parts = self._absolute(path, parts)
        if not parts: return None
        if len(parts) == 1:
            name = parts[0]
            if name in PRIMITIVES - {'String'}: return None if exports else ('external',)
            if name in self.local_nominal[path]:
                if self.import_paths[path, name] or name in self.local_aliases[path]: return None
                return ('local', path, name)
            if name in self.local_aliases[path]: return ('alias', path, name)
            imports = self.import_paths[path, name]
            if imports:
                if len(imports) != 1 or (path, name) in seen: return None
                return self._resolve(path, imports[0], seen + ((path, name),))
            matches = []
            for prefix in self.glob_paths[path]:
                if prefix[:1] != ('crate',): return None
                key = (path, parts)
                if key in seen: continue
                resolved = self._resolve(path, prefix + (name,), seen + (key,))
                if resolved is not None: matches.append(resolved)
            if len(matches) == 1: return matches[0]
            if matches or name in self.opaque or name in self.nominal: return None
            if not exports and (name in PRIMITIVES or name in WRAPPERS): return ('external',)
            return None
        root = parts[0]
        if root == 'crate':
            owners = self.locations[parts]
            if len(owners) == 1: return ('local', next(iter(owners)), parts[-1])
            files = self.module_files[parts[:-1]]
            key = (path, parts)
            if not owners and len(files) == 1 and key not in seen:
                return self._resolve(next(iter(files)), (parts[-1],), seen + (key,), exports=True)
            return None
        # The standard library cannot depend on a downstream workspace crate.
        if root in self.external or root in ('std', 'core', 'alloc'):
            if root in self.local_nominal[path] or root in self.modules[path] or self.import_paths[path, root]: return None
            return ('external',)
        return None

    def kind(self, value, path, local=True, seen=(), substitutions=None):
        path = getattr(value, 'path', path)
        if not seen and substitutions is None:
            key = (path, tuple(value))
            if key not in self.kind_cache: self.kind_cache[key] = self._kind(value, path)
            return self.kind_cache[key]
        return self._kind(value, path, local, seen, substitutions)

    def _kind(self, value, path, local=True, seen=(), substitutions=None):
        path = getattr(value, 'path', path)
        substitutions = {} if substitutions is None else substitutions
        value = list(value)
        if any(name in STYLE for name in names(value)): return True
        while value[:1] in (['&'], ['mut'], ['*']): value = value[1:]
        if value[:1] == ["'"] and len(value) > 1: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value: return None
        if value[0] in ('(', '['):
            end = closing(value, 0)
            if end != len(value) - 1: return None
            start, results = 1, []
            while start < end:
                stop = until(value, start, {',', ';', ')', ']'})
                results.append(self.kind(value[start:stop], path, seen=seen, substitutions=substitutions))
                if value[stop:stop + 1] == [';']: break
                start = stop + 1
            return True if True in results else None if None in results else False
        if value[0] == '@map-entry': value = ['BTreeMap', *value[1:]]
        parts, at = [value[0]], 1
        while value[at:at + 1] == ['::'] and at + 1 < len(value):
            parts.append(value[at + 1]); at += 2
        if len(parts) == 1 and (path, parts[0]) in substitutions:
            key = (path, '@generic', parts[0])
            if key in seen: return None
            argument, source = substitutions[path, parts[0]]
            return self.kind(argument, source, seen=seen + (key,), substitutions=substitutions)
        resolution = self._resolve(path, parts)
        if resolution is None or any(p in self.generics for p in parts): return None
        if resolution[0] in ('local', 'alias') and resolution[-1] in STYLE: return True
        results, arguments = [], []
        if value[at:at + 1] == ['<']:
            end, start = closing(value, at), at + 1
            if end != len(value) - 1: return None
            while start < end:
                stop = until(value, start, {',', '>'})
                argument = value[start:stop]
                arguments.append((argument, path))
                if argument[:1] != ["'"]: results.append(self.kind(argument, path, seen=seen, substitutions=substitutions))
                start = stop + 1
        elif at != len(value): return None
        if resolution[0] in ('alias', 'local'):
            _, owner, name = resolution
            parameters = self.type_parameters.get((owner, name), [])
            if len(arguments) != len(parameters): return None
            substitutions = dict(substitutions)
            substitutions.update({(owner, parameter): argument for parameter, argument in zip(parameters, arguments) if parameter is not None})
        if resolution[0] == 'alias':
            _, owner, name = resolution
            key = (owner, name)
            if key in seen: return None
            aliases = self.local_alias_values[owner, name]
            if not aliases: return None
            results.extend(self.kind(alias, owner, seen=seen + (key,), substitutions=substitutions) for alias in aliases)
        elif resolution[0] == 'local':
            _, owner, name = resolution
            key = (owner, name)
            if name in self.deref_nominals and key not in seen:
                targets = self.deref_targets[owner, name]
                if not targets: return None
                results.extend(self.kind(t, owner, seen=seen + (key,), substitutions=substitutions) for t in targets)
            if key not in seen:
                fields = [(field, values) for (file, nominal, field), values in self.declared_fields.items() if file == owner and nominal == name]
                enum = self.enum_payloads[owner, name]
                if (owner, name) in self.enums:
                    results.extend(self.kind(t, owner, seen=seen + (key,), substitutions=substitutions) for t in enum)
                elif name in self.payload_nominal and (owner, name) not in self.structs: return None
                if name in self.payload_nominal and not fields and (owner, name) not in self.enums: return None
                for field, types in fields:
                    results.extend(self.kind(t, owner, seen=seen + (key,), substitutions=substitutions) for t in types)
        if True in results: return True
        return None if None in results else False

    def _project(self, value, field, path, seen=()):
        path = getattr(value, 'path', path)
        value = list(value)
        while value[:1] in (['&'], ['mut'], ['*']): value = value[1:]
        if value[:1] == ["'"] and len(value) >= 2: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value: return [()]
        root = value[0]
        if root in self.generics: return [()]
        if root in self.local_aliases[path]:
            if (path, root) in seen: return [()]
            return [p for alias in self.local_alias_values[path, root] for p in self._project(alias, field, path, seen + ((path, root),))]
        if root in ('Rc', 'Arc', 'Box', 'Pin') and self._standard(value, path) is not None and '<' in value:
            start = value.index('<')
            return self._project(value[start + 1:closing(value, start)], field, path, seen)
        # Unresolved generic substitutions cannot prove a non-style receiver.
        if '<' in value:
            start = value.index('<')
            args = value[start + 1:closing(value, start)]
            if names(args): return [()]
            value = value[:start]
        parts = tuple(value[::2])
        resolution = self._resolve(path, parts)
        if resolution is None or resolution[0] != 'local': return [()]
        _, owner, root = resolution
        return [DeclaredType(v, owner) for v in self.declared_fields.get((owner, root, field), [()])]

    def receiver(self, path, expression, index):
        if any(a < index < b for a, b in self.inline_modules[path]): return [()]
        expression = list(expression)
        while expression and expression[0] in ('&', 'mut', '*'): expression.pop(0)
        while expression[:1] == ['('] and closing(expression, 0) == len(expression) - 1:
            expression = expression[1:-1]
            while expression and expression[0] in ('&', 'mut', '*'): expression.pop(0)
        if not expression or not expression[0].isidentifier(): return [()]
        if expression[0] == 'self':
            values = [owner for start, end, owner in self.impls[path] if start < index < end]
        else:
            values = self.bindings[path].get(expression[0], [()])
            proofs = [(start, value) for start, end, value in self.proofs[path, expression[0]]
                      if start <= index < end]
            if proofs:
                start = max(start for start, _ in proofs)
                # A nested binding/pattern must never inherit an outer proof.
                if expression[0] in self._bindings(self.streams[path][start:index]):
                    return [()]
                values = [value for position, value in proofs if position == start]
        if not values: return [()]
        tail = expression[1:]
        while tail:
            if len(tail) < 2 or tail[0] != '.' or not (tail[1].isidentifier() or tail[1].isdigit()): return [()]
            if tail[2:3] == ['(']:
                end = closing(tail, 2)
                # first_entry/last_entry return Option<OccupiedEntry>; only ?
                # proves that the local contains the occupied variant itself.
                if tail[1] not in ('first_entry', 'last_entry') or end != 3 or tail[end + 1:] != ['?']:
                    return [()]
                results = []
                for value in values:
                    source = getattr(value, 'path', path)
                    value = self._standard(value, path)
                    if value is None or value[:2] != ['BTreeMap', '<']: return [()]
                    results.append(DeclaredType(['@map-entry', *value[1:]], source))
                return results
            values = [part for value in values for part in self._project(value, tail[1], path)]
            tail = tail[2:]
        return values

    def _standard(self, value, path, seen=()):
        path = getattr(value, 'path', path)
        value = list(value)
        while value and value[0] in ('&', 'mut', '*'): value.pop(0)
        if value[:1] == ["'"] and len(value) > 1: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value: return None
        root = value[0]
        end = value.index('<') if '<' in value else len(value)
        parts = tuple(value[:end:2])
        if parts in STANDARD_PATHS and self._resolve(path, parts) == ('external',):
            return [STANDARD_PATHS[parts], *value[end:]]
        if root in self.local_aliases[path]:
            aliases = self.local_alias_values[path, root]
            if root in seen or len(aliases) != 1: return None
            return self._standard(aliases[0], path, seen + (root,))
        imports = self.import_paths[path, root]
        if imports:
            if len(imports) != 1 or imports[0] not in STANDARD_PATHS: return None
            return [STANDARD_PATHS[imports[0]], *value[1:]]
        if root == '@map-entry': return value
        if root not in WRAPPERS or self._resolve(path, (root,)) != ('external',): return None
        return value

    def canonical_receiver(self, value, path, api, associated, seen=()):
        # Only inherent standard APIs prove the return type. Nominal receivers
        # may obtain same-named methods from local, imported or blanket traits.
        path = getattr(value, 'path', path)
        value = self._standard(value, path)
        if value is None: return False
        while value and value[0] in ('&', 'mut', '*'): value = value[1:]
        if value[:1] == ["'"] and len(value) > 1: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value: return False
        root = value[0]
        if not associated and root in ('Rc', 'Arc', 'Box', 'Pin') and value[1:2] == ['<']:
            if api in self.trait_methods: return False
            return self.canonical_receiver(value[2:closing(value, 1)], path, api, False, seen)
        # Vec's get_mut comes through slice autoderef. A local trait method can
        # win method lookup before autoderef, unlike an inherent map method.
        if root == 'Vec' and api in self.trait_methods: return False
        if root in self.local_aliases[path]:
            aliases = self.local_alias_values[path, root]
            return root not in seen and bool(aliases) and all(
                self.canonical_receiver(alias, path, api, associated, seen + (root,))
                for alias in aliases)
        if root in self.nominal: return False
        allowed = {'make_mut': {'Rc', 'Arc'}, 'get_mut': {'Rc', 'Arc', 'RefCell', 'Vec', 'VecDeque', 'BTreeMap', 'HashMap', '@map-entry'},
                   'borrow_mut': {'RefCell'}}[api]
        if not associated: allowed -= {'Rc', 'Arc'}
        return root in allowed and value[1:2] == ['<']

    def may_escape(self, path, words, index):
        associated = words[index - 1] == '::'
        if associated:
            # Unknown UFCS providers may return style from an unrelated input.
            allowed = {'make_mut': {'Rc', 'Arc'}, 'get_mut': {'Rc', 'Arc', 'RefCell'},
                       'borrow_mut': {'RefCell'}}[words[index]]
            if index < 2 or words[index - 2] not in allowed: return True
            provider = index - 2
            while provider >= 2 and words[provider - 1] == '::': provider -= 2
            standard = {'Rc': ['std', '::', 'rc', '::', 'Rc'],
                        'Arc': ['std', '::', 'sync', '::', 'Arc'],
                        'RefCell': ['std', '::', 'cell', '::', 'RefCell']}
            if provider != index - 2 and words[provider:index - 1] != standard[words[index - 2]]: return True
            start = index + 2
            end = until(words, start, {',', ')'})
            expression = words[start:end]
        else:
            expression = self.field_receiver(words, index)
        values = self.receiver(path, expression, index)
        return any(self.kind(value, path) is not False or not self.canonical_receiver(value, path, words[index], associated) for value in values)

    @staticmethod
    def field_receiver(words, index):
        start = index - 2
        while start >= 2 and words[start - 1] == '.' and (words[start - 2].isidentifier() or words[start - 2].isdigit()):
            start -= 2
        # Do not mistake a suffix of an unresolved call/index/path for a local.
        if start < 0 or (start and words[start - 1] in ('.', '::')): return []
        return words[start:index - 1]

    def _unrelated_field(self, value, field, path, seen=()):
        path = getattr(value, 'path', path)
        value = list(value)
        while value[:1] in (['&'], ['mut'], ['*']): value = value[1:]
        if value[:1] == ["'"] and len(value) > 1: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value or any(name in STYLE for name in names(value)): return False
        if value[0] in ('Rc', 'Arc', 'Box', 'Pin') and self._standard(value, path) is not None and '<' in value:
            start = value.index('<')
            return self._unrelated_field(value[start + 1:closing(value, start)], field, path, seen)
        at = value.index('<') if '<' in value else len(value)
        resolution = self._resolve(path, tuple(value[:at:2]))
        if resolution is None: return False
        if resolution[0] == 'external': return self.kind(value, path) is False
        _, owner, root = resolution
        if root in STYLE or root in self.generics: return False
        key = (owner, root)
        if key in seen: return False
        if resolution[0] == 'alias':
            aliases = self.local_alias_values[owner, root]
            return bool(aliases) and all(self._unrelated_field(alias, field, owner, seen + (key,)) for alias in aliases)
        # A declared field belongs to this nominal even when other fields contain
        # Computed. Missing fields could instead be found through unknown Deref.
        return (owner, root) in self.structs and bool(self.declared_fields[owner, root, field])

    def may_write(self, path, words, index):
        values = self.receiver(path, self.field_receiver(words, index), index)
        return any(not self._unrelated_field(value, words[index], path) for value in values)
