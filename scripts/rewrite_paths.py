"""Draft migration primitive: span-aware Rust paths, preserving cfg attributes."""
import re
from bisect import bisect_left

from rust_layer_paths import TOKEN, IDENT, absolute, file_module, use_tree


def lexemes(source):
    offset, line = 0, 1
    while match := TOKEN.search(source, offset):
        line += source.count('\n', offset, match.start())
        word, end = match.group(), match.end()
        keep = True
        if word == '/*':
            depth = 1
            for comment in re.finditer(r'/\*|\*/', source[end:]):
                depth += 1 if comment.group() == '/*' else -1
                if not depth:
                    end += comment.end()
                    break
            else:
                end = len(source)
            keep = False
        elif match.group('hashes') is not None:
            stop = source.find('"' + match.group('hashes'), end)
            end = len(source) if stop < 0 else stop + 1 + len(match.group('hashes'))
            keep = False
        elif word.startswith('//') or '"' in word or (len(word) > 1 and word.endswith("'")):
            keep = False
        yield (word.removeprefix('r#') if keep else None), line, match.start(), end
        line += source.count('\n', match.start(), end)
        offset = end


def spans(source):
    return (item for item in lexemes(source) if item[0] is not None)


def rewrite(source, filename, current, owners, resolve=None, module_path=None):
    """owners maps original root modules to crate names; original roots remain.

    Resolve, when supplied, must yield one canonical path for every local
    reference. Ambiguous aliases are a preflight failure, never guessed.
    """
    tokens = list(lexemes(source))
    words = [item for item in tokens if item[0] is not None]
    comments = [(left, right) for word, _, left, right in tokens
                if word is None and source[left:right].startswith(('//', '/*'))]
    comment_starts = [left for left, _ in comments]
    plain = [(word, line) for word, line, _, _ in words]
    module, scopes, edits = (file_module(filename) if module_path is None else tuple(module_path)), [], []
    raw = {word for word, _, start, end in words
           if source[start:end].startswith('r#')}

    def render(path):
        return '::'.join('r#' + part if part in raw else part for part in path)

    def with_comments(start, end, text):
        # Keep exact comment text, including nested block comments. Moving it
        # ahead of a reconstructed path preserves Rust semantics and cfg scope.
        selected = comments[bisect_left(comment_starts, start):bisect_left(comment_starts, end)]
        return ''.join(source[left:right] + '\n' for left, right in selected
                       if right <= end) + text

    def target(path):
        if not path or path[0] not in ('crate', 'self', 'super'):
            return None
        full = absolute(path, module, owners)
        if resolve:
            full = tuple(resolve(full))
        if not full or full[0] not in owners:
            raise ValueError(f'{filename}: unassigned local path {"::".join(path)}')
        owner = owners[full[0]]
        root = 'crate' if owner == current else owner.replace('-', '_')
        return render((root, *full))

    index = 0
    while index < len(words):
        word, _, start, end = words[index]
        if word == 'use' and index + 1 < len(words) and words[index + 1][0] != '<':
            leaves, after = use_tree(plain, index + 1, aliases=True)
            if not leaves or after >= len(words) or words[after][0] != ';':
                raise ValueError(f'{filename}: unsupported use tree at line {words[index][1]}')
            rendered, changed = [], False
            for path, _, alias in leaves:
                moved = target(path)
                value = moved or render(path)
                changed |= moved is not None
                if alias and alias != path[-1]:
                    value += ' as ' + render((alias,))
                rendered.append(value)
            if changed:
                if any(plain[n][0] == '::' and plain[n - 1][0] in ('use', '{', ',')
                       for n in range(index + 1, after)):
                    raise ValueError(f'{filename}: mixed absolute extern use needs explicit ownership')
                # Keep one item: its cfg/lint/doc attributes still cover ALL leaves.
                text = rendered[0] if len(rendered) == 1 else '{' + ', '.join(rendered) + '}'
                left, right = words[index + 1][2], words[after][2]
                edits.append((left, right, with_comments(left, right, text)))
            index = after + 1
            continue
        if word == '{':
            scopes.append(module)
            if index >= 2 and words[index - 2][0] == 'mod':
                module += (words[index - 1][0],)
        elif word == '}' and scopes:
            module = scopes.pop()
        elif word in ('crate', 'self', 'super') and index + 1 < len(words) and words[index + 1][0] == '::':
            if index and words[index - 1][0] == '$':
                raise ValueError(f'{filename}: $crate macro path needs explicit ownership')
            path, after = [word], index + 1
            while after + 1 < len(words) and words[after][0] == '::' and IDENT.fullmatch(words[after + 1][0]):
                path.append(words[after + 1][0])
                after += 2
            moved = target(tuple(path))
            if moved:
                right = words[after - 1][3]
                edits.append((start, right, with_comments(start, right, moved)))
            index = after
            continue
        index += 1
    for start, end, text in reversed(edits):
        source = source[:start] + text + source[end:]
    return source
