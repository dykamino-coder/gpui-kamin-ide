"""Extract explicit Rust module paths, including nested use trees and relative paths.

This is a lexical dependency check, not name resolution: an imported alias is
accounted for at its declaration. All cfg branches and test modules are checked.
"""

import re


TOKEN = re.compile(
    r'//[^\n]*|/\*|(?:br|cr|r)(?P<hashes>\#*)"|'
    r'(?:b|c)?"(?:\\.|[^"\\])*"|'
    r"b?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|.)|[^'\\\n])'|"
    r'(?:r\#)?[A-Za-z_][A-Za-z_0-9]*|::|\S',
    re.DOTALL,
)
IDENT = re.compile(r'[A-Za-z_][A-Za-z_0-9]*\Z')


def tokens(source):
    """Ignore comments and literals without losing diagnostic line numbers."""
    offset, line = 0, 1
    while match := TOKEN.search(source, offset):
        line += source.count('\n', offset, match.start())
        word, end = match.group(), match.end()
        if word == '/*':
            depth = 1
            for comment in re.finditer(r'/\*|\*/', source[end:]):
                depth += 1 if comment.group() == '/*' else -1
                if not depth:
                    end += comment.end()
                    break
            else:
                end = len(source)
        elif match.group('hashes') is not None:
            closing = '"' + match.group('hashes')
            stop = source.find(closing, end)
            end = len(source) if stop < 0 else stop + len(closing)
        elif not (word.startswith('//') or '"' in word or word.startswith("'" + '\\')):
            # A complete character literal contains its closing quote; lifetimes do not.
            if not (len(word) > 1 and word.endswith("'")):
                yield (word.removeprefix('r#'), line)
        line += source.count('\n', match.start(), end)
        offset = end


def use_tree(words, index, prefix=(), aliases=False):
    """Expand `crate::{paint::x, layout::{self, y as z}}`."""
    path = prefix
    while index < len(words):
        word, line = words[index]
        if word == '::':
            index += 1
        elif IDENT.fullmatch(word) or word == '*':
            path += (word,)
            index += 1
            if index < len(words) and words[index][0] == '::':
                continue
            break
        elif word == '{':
            result = []
            index += 1
            while index < len(words) and words[index][0] != '}':
                leaves, index = use_tree(words, index, path, aliases)
                result.extend(leaves)
                if index < len(words) and words[index][0] == ',':
                    index += 1
            return result, index + 1
        else:
            return [], index + 1
    if path and path[-1] == 'self':
        path = path[:-1]
    alias = path[-1] if path else None
    if index < len(words) and words[index][0] == 'as':
        alias = words[index + 1][0]
        index += 2
    return [(path, line, alias) if aliases else (path, line)], index


def file_module(filename):
    parts = filename.removesuffix('.rs').split('/')
    if parts[-1] in ('mod', 'lib'):
        parts.pop()
    return tuple(parts)


def absolute(path, module, layers):
    if not path:
        return ()
    if path[0] in ('crate', '$crate'):
        return path[1:]
    if path[0] == 'self':
        return module + path[1:]
    if path[0] == 'super':
        parents = 0
        while parents < len(path) and path[parents] == 'super':
            parents += 1
        return module[:max(0, len(module) - parents)] + path[parents:]
    # Unqualified use roots denote local modules (e.g. style's own `text`),
    # not same-named phases at the crate root.
    return module + path


def dependencies(source, filename, layers):
    """Yield (absolute module path, line) for use trees and qualified paths."""
    words = list(tokens(source))
    module, scopes = file_module(filename), []
    index = 0
    while index < len(words):
        word, line = words[index]
        if word == 'use':
            leaves, index = use_tree(words, index + 1)
            for path, at in leaves:
                if resolved := absolute(path, module, layers):
                    yield ('::'.join(resolved), at)
            continue
        if word == '{':
            scopes.append(module)
            if index >= 2 and words[index - 2][0] == 'mod':
                module += (words[index - 1][0],)
        elif word == '}' and scopes:
            module = scopes.pop()
        elif word in ('crate', 'self', 'super'):
            path, end = (word,), index + 1
            while end + 1 < len(words) and words[end][0] == '::':
                if not IDENT.fullmatch(words[end + 1][0]):
                    break
                path += (words[end + 1][0],)
                end += 2
            if len(path) > 1:
                if resolved := absolute(path, module, layers):
                    yield ('::'.join(resolved), line)
                index = end
                continue
        index += 1
