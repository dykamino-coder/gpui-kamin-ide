"""Conservative explicit impl ownership checks; Rust compilation remains required."""
from collections import Counter
import hashlib
from canonical_exports import CanonicalExports
from impl_bindings import ImplBindings
from module_sources import modules
from rewrite_paths import spans
from rust_layer_paths import absolute, IDENT


def module_items(words, initial):
    module, scopes, visible, item = initial, [], True, None
    parens = brackets = 0
    keywords = {'struct', 'enum', 'union', 'trait', 'type', 'impl', 'mod',
                'fn', 'const', 'static', 'use', 'extern', 'macro', 'macro_rules'}
    for i, (word, _, _, _) in enumerate(words):
        if visible and not (parens or brackets) and word in keywords and item is None:
            yield i, module
            item = word
        if word == '{':
            body = not (parens or brackets)
            scopes.append((module, visible, None if body else item, parens, brackets))
            visible = visible and body and item == 'mod' and i >= 2 and words[i - 2][0] == 'mod'
            if visible:
                module += (words[i - 1][0],)
            item, parens, brackets = None, 0, 0
        elif word == '}' and scopes:
            module, visible, item, parens, brackets = scopes.pop()
        elif visible:
            parens += (word == '(') - (word == ')')
            brackets += (word == '[') - (word == ']')
            if word == ';' and not (parens or brackets):
                item = None


def closes_angle(words, index):
    # The tokenizer keeps '-' and '>' separately. A function return arrow
    # inside a generic bound must not close the impl parameter list.
    return words[index][0] == '>' and (not index or words[index - 1][0] != '-')


def header(words, start):
    angle = paren = square = 0
    for end in range(start + 1, len(words)):
        word = words[end][0]
        if word in ('{', ';') and not (angle or paren or square):
            return words[start + 1:end]
        angle += (word == '<') - (closes_angle(words, end) and angle > 0)
        paren += (word == '(') - (word == ')')
        square += (word == '[') - (word == ']')
    raise ValueError('Unclosed impl head')


def heads(words):
    """Split only the top-level `for`, after optional impl parameters."""
    if words and words[0][0] == '<':
        depth = 0
        for i, item in enumerate(words):
            depth += (item[0] == '<') - closes_angle(words, i)
            if not depth:
                words = words[i + 1:]
                break
    depth = 0
    for i, (word, _, _, _) in enumerate(words):
        if not depth and word == 'for':
            return words[:i], words[i + 1:]
        closing = word in (')', ']') or closes_angle(words, i)
        depth += (word in ('<', '(', '[')) - (closing and depth > 0)
    return None, words


def nominal(words):
    if not words:
        return None
    i = int(words[0][0] == '!')
    path = []
    while i < len(words) and IDENT.fullmatch(words[i][0]):
        path.append(words[i][0])
        i += 1
        if i + 1 >= len(words) or words[i][0] != '::':
            break
        i += 1
    return tuple(path) if path else None


def audit(sources, owner, file_modules=None, exports=None):
    if file_modules is None:
        _, file_modules = modules(sources)
    tokens = {name: list(spans(source)) for name, source in sources.items()}
    declarations = {}
    for name, words in tokens.items():
        for i, module in module_items(words, file_modules[name]):
            kind = words[i][0]
            if kind in ('struct', 'enum', 'union', 'trait', 'type') and i + 1 < len(words):
                symbol = words[i + 1][0]
                if IDENT.fullmatch(symbol):
                    declarations[module + (symbol,)] = (owner(name), kind, name)

    if exports is None:
        exports = CanonicalExports(sources, file_modules)
    bindings = ImplBindings(exports, declarations)

    def resolve(words, module):
        path = nominal(words)
        if path is None:
            return None
        results = bindings.resolve(absolute(path, module, ()))
        found = [p for p in results if p in declarations]
        if len(results) != 1 or len(found) != 1:
            return None
        package, kind, filename = declarations[found[0]]
        return dict(path='::'.join(found[0]), package=package, kind=kind, file=filename)

    rows = []
    for name, words in tokens.items():
        for i, module in module_items(words, file_modules[name]):
            if words[i][0] != 'impl':
                continue
            impl = header(words, i)
            trait_head, self_head = heads(impl)
            target, trait = resolve(self_head, module), resolve(trait_head, module)
            current = owner(name)
            if target and target['kind'] != 'type' and target['package'] != current:
                status = ('inherent-foreign-owner' if trait_head is None else
                          'trait-and-self-foreign-owner' if trait and trait['package'] != current
                          and not any(w[0] == '<' for w in trait_head) else
                          'local-trait' if trait and trait['package'] == current else 'foreign-self-review-trait')
            elif target and target['kind'] != 'type':
                status = 'local-self'
            else:
                status = 'unresolved-self-review'
            end = impl[-1][3] if impl else words[i][3]
            rows.append(dict(file=name, line=words[i][1], package=current, status=status,
                             header=sources[name][words[i][2]:end], target=target, trait=trait,
                             source_sha256=hashlib.sha256(sources[name].encode()).hexdigest()))
    return dict(counts=dict(Counter(r['status'] for r in rows)), impls=rows,
                limitations='All cfg branches, explicit module items only. Alias self types, '
                'generic/fundamental wrapper rules and macro-generated impls are not proven. '
                'Unknown self/trait heads require review. Rust compilation is mandatory.')
