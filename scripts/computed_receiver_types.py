"""Conservative receiver type facts for mutable style escapes.

Unknown receivers remain guarded. Only explicit Rust types prove that a call
belongs to unrelated state; names such as cache/state never grant exemptions.
"""
from collections import defaultdict
from rust_layer_paths import file_module

STYLE = {'Computed', 'InheritedStyle', 'SharedStyle', 'BoxStyle'}
PRIMITIVES = set('bool char str String usize isize u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64'.split())
WRAPPERS = set('Rc Arc Box Pin Cell RefCell Ref RefMut Option Result Vec VecDeque HashMap HashSet BTreeMap BTreeSet SmallVec Cow Mutex RwLock'.split())


def closing(words, start):
    pairs = {'(': ')', '[': ']', '{': '}', '<': '>'}
    stack = []
    for i in range(start, len(words)):
        word = words[i]
        if word in pairs:
            stack.append(pairs[word])
        elif stack and word == stack[-1]:
            stack.pop()
            if not stack:
                return i
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
    def __init__(self, streams):
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
        self.qualified = set()
        self.fields, self.bindings, self.impls = defaultdict(list), {}, {}
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
                # A generic parameter may shadow a nominal type in another file.
                generic = i + (1 if word == 'impl' else 2)
                if word in ('impl', 'fn', 'struct', 'enum', 'union', 'trait', 'type') and words[generic:generic + 1] == ['<']:
                    end = closing(words, generic)
                    for j in range(generic + 1, end):
                        if words[j].isidentifier() and words[j - 1] in ('<', ',', 'const'):
                            self.generics.add(words[j])
                if word in ('struct', 'enum', 'union'):
                    name = words[i + 1]
                    self.nominal.add(name)
                    if i in module_level:
                        self.local_nominal[path].add(name)
                        self.qualified.add(('crate', *file_module(path.removeprefix('crates/html/src/')), name))
                    at = until(words, i + 2, {'{', '(', ';'})
                    # A nominal name alone does not prove its fields/variants unrelated.
                    if at < len(words) and words[at] in ('{', '(') and closing(words, at) > at + 1:
                        self.payload_nominal.add(name)
                    if at < len(words) and words[at] == '{':
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
                if word == 'use':
                    end = until(words, i + 1, {';'})
                    part = words[i + 1:end]
                    if part[:1] not in (['std'], ['core'], ['alloc']):
                        self.opaque.update(names(part))
                        if '*' in part: self.foreign_globs.add(path)
                if word == 'impl':
                    at = until(words, i + 1, {'{', ';'})
                    if at >= len(words) or words[at] != '{': continue
                    head = words[i + 1:at]
                    if head[:1] == ['<']: head = head[closing(head, 0) + 1:]
                    if 'for' in head:
                        trait, head = head[:head.index('for')], head[head.index('for') + 1:]
                        trait_impls.append((trait, head))
                    if 'where' in head: head = head[:head.index('where')]
                    body_end = closing(words, at)
                    self.impls[path].append((at, body_end, tuple(head)))
                    if any(words[j:j + 2] in (['fn', 'borrow_mut'], ['fn', 'get_mut'], ['fn', 'make_mut'])
                           for j in range(at + 1, body_end)):
                        self.opaque.update(names(head))
        for trait, head in trait_impls:
            if any(name in self.indirections or name in self.opaque for name in names(trait)):
                self.opaque.update(names(head))

    @staticmethod
    def _bindings(words):
        result = defaultdict(list)
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
            if word == '|':
                end = i + 1
                while end < len(words) and words[end] != '|': end += 1
                if end < len(words):
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

    def kind(self, value, path, local=True, seen=()):
        ids = names(value)
        if not ids: return None
        if any(name in STYLE for name in ids): return True
        if path in self.foreign_globs: return None
        if any(name in self.generics or name in self.opaque for name in ids): return None
        # Only an exact defining module path resolves a qualified nominal.
        proven = set()
        value = self._qualified(value, proven)
        if value is None: return None
        ids = names(value)
        results = []
        for name in ids:
            if name in self.aliases:
                if name in seen: return None
                if not local or name not in self.local_aliases[path]:
                    results.append(None)
                else:
                    results.extend(self.kind(alias, path, local, seen + (name,)) for alias in self.local_alias_values[path, name])
            elif name in self.nominal:
                known = (local and name in self.local_nominal[path]) or name in proven
                results.append(False if known and name not in self.payload_nominal else None)
            elif name in PRIMITIVES or name in WRAPPERS:
                results.append(False)
            else:
                results.append(None)
        if True in results: return True
        return False if results and all(x is False for x in results) else None

    def _project(self, value, field, path, seen=()):
        value = self._qualified(value)
        if value is None: return [()]
        while value[:1] in (['&'], ['mut'], ['*']): value = value[1:]
        if value[:1] == ["'"] and len(value) >= 2: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        ids = names(value)
        if not ids: return [()]
        root = ids[0]
        if root in self.opaque or root in self.generics: return [()]
        if root in self.aliases:
            if root in seen: return [()]
            if root not in self.local_aliases[path]: return [()]
            return [p for alias in self.local_alias_values[path, root] for p in self._project(alias, field, path, seen + (root,))]
        if root in ('Rc', 'Arc', 'Box', 'Pin') and '<' in value:
            start = value.index('<')
            return self._project(value[start + 1:closing(value, start)], field, path, seen)
        # Unresolved generic substitutions cannot prove a non-style receiver.
        if '<' in value:
            start = value.index('<')
            args = value[start + 1:closing(value, start)]
            if names(args): return [()]
            value = value[:start]
        if root not in self.local_nominal[path] or self.field_files[root, field] != {path}: return [()]
        return self.fields.get((root, field), [()])

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
        if not values: return [()]
        tail = expression[1:]
        while tail:
            if len(tail) < 2 or tail[0] != '.' or not (tail[1].isidentifier() or tail[1].isdigit()): return [()]
            values = [part for value in values for part in self._project(value, tail[1], path)]
            tail = tail[2:]
        return values

    def canonical_receiver(self, value, path, api, associated, seen=()):
        # Only inherent standard APIs prove the return type. Nominal receivers
        # may obtain same-named methods from local, imported or blanket traits.
        value = list(value)
        while value and value[0] in ('&', 'mut', '*'): value = value[1:]
        if value[:1] == ["'"] and len(value) > 1: value = value[2:]
        while value[:1] == ['mut']: value = value[1:]
        if not value: return False
        root = value[0]
        if root in self.local_aliases[path]:
            aliases = self.local_alias_values[path, root]
            return root not in seen and bool(aliases) and all(
                self.canonical_receiver(alias, path, api, associated, seen + (root,))
                for alias in aliases)
        if root in self.nominal: return False
        allowed = {'make_mut': {'Rc', 'Arc'}, 'get_mut': {'Rc', 'Arc', 'RefCell'},
                   'borrow_mut': {'RefCell'}}[api]
        if not associated: allowed &= {'RefCell'}
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
            start = index - 2
            while start >= 2 and words[start - 1] == '.' and (words[start - 2].isidentifier() or words[start - 2].isdigit()): start -= 2
            expression = words[start:index - 1]
        values = self.receiver(path, expression, index)
        return any(self.kind(value, path) is not False or not self.canonical_receiver(value, path, words[index], associated) for value in values)
