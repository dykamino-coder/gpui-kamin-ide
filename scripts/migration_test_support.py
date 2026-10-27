"""Widen only lower test contracts reached from the upper package.

The semantic preparation retains cfg(test) in a single crate. Extraction alone
introduces the reviewed feature for an upper test harness to call those helpers.
Module identities and explicit reexport chains come from the current source,
including #[path] modules; no test filename or generated symbol prefix is used.
"""
import re

from html_guard_paths import normalize
from module_sources import code_mask, DECL, matching_brace
from rewrite_paths import lexemes, rewrite
from rust_layer_paths import use_tree

SUPPORT = '#[cfg(any(test, feature = "engine-test-support"))]'
EXACT_TEST = re.compile(r'#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]')


def alias_chain(exports, path, seen=frozenset()):
    """Retain all relay identities, not just the final defining function."""
    result = {path}
    for size in range(len(path), 0, -1):
        prefix, suffix = path[:size], path[size:]
        if suffix and prefix in exports.modules:
            return result
        if prefix not in exports.aliases:
            continue
        if prefix in seen:
            raise ValueError(f'Cyclic test-contract alias: {path}')
        targets = exports.aliases[prefix] - {prefix}
        if len(targets) > 1:
            raise ValueError(f'Ambiguous test-contract alias: {path}')
        result.add(prefix)
        for target in targets:
            result.update(alias_chain(exports, target + suffix, seen | {prefix}))
        return result
    return result


def scope_at(filename, offset, declarations, file_modules, masks):
    candidates = [file_modules[filename]]
    for path, module in declarations.items():
        if not module.inline or module.filename != filename:
            continue
        match = DECL.match(masks[filename], module.declaration_start)
        stop = matching_brace(masks[filename], match.end() - 1)
        if match.end() <= offset < stop:
            candidates.append(path)
    return max(candidates, key=len)


def item_identities(source, match, module):
    """Find the item governed by an exact test cfg; reject unknown syntax."""
    words = [word for word in lexemes(source[match.end():]) if word[0] is not None]
    plain = [(word, line) for word, line, _, _ in words]
    index = 0
    # Other attributes can intervene between cfg and the item.
    while index < len(words) and words[index][0] == '#':
        if index + 1 >= len(words) or words[index + 1][0] != '[':
            raise ValueError('Unsupported test-contract attribute')
        index, depth = index + 2, 1
        while index < len(words) and depth:
            depth += (words[index][0] == '[') - (words[index][0] == ']')
            index += 1
    for index in range(index, len(words)):
        word = words[index][0]
        if word == 'use':
            leaves, after = use_tree(plain, index + 1, aliases=True)
            if not leaves or after >= len(words) or words[after][0] != ';':
                raise ValueError('Unsupported test-contract reexport')
            if any(alias in ('*', '_', None) for _, _, alias in leaves):
                raise ValueError('Test-contract glob requires an explicit export')
            return {module + (alias,) for _, _, alias in leaves}
        if word in ('mod', 'fn', 'struct', 'enum', 'trait', 'type', 'const', 'static', 'union'):
            if index + 1 >= len(words):
                raise ValueError('Incomplete test-contract item')
            return {module + (words[index + 1][0],)}
        if word == 'impl':
            # Associated fixtures are governed by the impl's cfg, not by the
            # production type's identity. Reject generic/trait impls until
            # their ownership can be represented without broadening a gate.
            if index + 2 >= len(words) or words[index + 2][0] != '{':
                raise ValueError('Unsupported test-contract impl header')
            owner = words[index + 1][0]
            if not re.fullmatch(r'(?:r#)?[A-Za-z_]\w*', owner):
                raise ValueError('Unsupported test-contract impl owner')
            depth, identities = 1, set()
            for offset in range(index + 3, len(words)):
                token = words[offset][0]
                if token == 'fn' and depth == 1:
                    identities.add(module + (owner, words[offset + 1][0]))
                depth += (token == '{') - (token == '}')
                if not depth:
                    return identities
            raise ValueError('Unclosed test-contract impl')
        if word in ('{', ';'):
            break
    raise ValueError('Unsupported item after exact test cfg')


def widen(sources, declarations, file_modules, exports, owners, upper='kamin-html'):
    """Return source copies; only exact cfg(test) on reached items can change.

    Ordinary upper uses of a lower test-only item still fail the subsequent
    normal Cargo check because this feature is absent from its normal graph.
    The operation is deterministic and a no-op on its already widened output.
    """
    required = set()

    def observe(path):
        canonical = exports.resolve(tuple(path))
        if len(canonical) != 1:
            raise ValueError(f'Ambiguous upper test-contract path: {path}')
        target = next(iter(canonical))
        if target and target[0] in owners and owners[target[0]] != upper:
            required.update(alias_chain(exports, tuple(path)))
        return tuple(path) if path and path[0] in owners else target

    for filename, source in sorted(sources.items()):
        root = filename.split('/')[0].removesuffix('.rs')
        if filename == 'lib.rs' or owners.get(root) == upper:
            rewrite(normalize(source), filename, upper, owners, observe,
                    module_path=file_modules[filename])
    masks = {name: code_mask(source) for name, source in sources.items()}
    result = dict(sources)
    for filename, source in sorted(sources.items()):
        root = filename.split('/')[0].removesuffix('.rs')
        if filename == 'lib.rs' or owners.get(root) == upper:
            continue
        edits = []
        for match in EXACT_TEST.finditer(masks[filename]):
            module = scope_at(filename, match.start(), declarations, file_modules, masks)
            try:
                identities = item_identities(source, match, module)
            except ValueError as exc:
                line = source.count('\n', 0, match.start()) + 1
                raise ValueError(f'{filename}:{line}: {exc}') from exc
            if any(path[:len(identity)] == identity
                   for path in required for identity in identities):
                edits.append((match.start(), match.end()))
        for start, end in reversed(edits):
            source = source[:start] + SUPPORT + source[end:]
        result[filename] = source
    return result
