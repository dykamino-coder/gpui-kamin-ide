"""Resolve physical Rust module files to declared module paths, for all cfgs.

No macro expansion is attempted. Unsupported #[path] expressions are rejected;
the migration must never silently derive module identity from a filename.
"""
from dataclasses import dataclass
from pathlib import PurePosixPath
import re

from rewrite_paths import lexemes


@dataclass(frozen=True)
class Module:
    path: tuple
    filename: str
    inline: bool
    declaration_file: str | None
    declaration_start: int | None


def code_mask(source):
    result, offset = [], 0
    for word, _, start, end in lexemes(source):
        result.append(source[offset:start])
        result.append(source[start:end] if word is not None else
                      ''.join('\n' if c == '\n' else ' ' for c in source[start:end]))
        offset = end
    result.append(source[offset:])
    return ''.join(result)


DECL = re.compile(r'(?P<attrs>(?:#\s*\[[^\]]*\]\s*)*)'
                  r'(?P<vis>pub(?:\s*\([^)]*\))?\s+)?mod\s+'
                  r'(?P<name>[A-Za-z_][A-Za-z_0-9]*)\s*(?P<end>[;{])')


def modules(sources):
    """Return modules and file -> canonical module path. All cfg branches count."""
    declarations, files = {}, {}

    def load(filename, path, declaration_file=None, start=None):
        if filename not in sources:
            raise ValueError(f'missing module file: {filename} ({"::".join(path)})')
        if filename in files:
            raise ValueError(f'module file has two declarations: {filename}')
        files[filename] = path
        declarations[path] = Module(path, filename, False, declaration_file, start)
        relative = PurePosixPath(filename)
        directory = relative.parent if relative.name in ('lib.rs', 'mod.rs') else relative.with_suffix('')
        visit(filename, path, 0, len(sources[filename]), directory, False)

    def visit(filename, path, begin, end, directory, inline):
        source = sources[filename]
        mask = code_mask(source)
        cursor = begin
        while cursor < end:
            # Only inspect declarations at this module scope; skip other bodies.
            declaration = DECL.match(mask, cursor)
            if declaration:
                name = declaration.group('name')
                child = path + (name,)
                if child in declarations:
                    raise ValueError(f'duplicate module path: {"::".join(child)}')
                raw_attrs = source[declaration.start('attrs'):declaration.end('attrs')]
                path_attrs = re.findall(r'#\s*\[\s*path\s*=\s*"([^"\\]+)"\s*\]', raw_attrs)
                if re.search(r'#\s*\[\s*path\b', raw_attrs) and len(path_attrs) != 1:
                    raise ValueError(f'unsupported module path attribute: {filename}:{cursor}')
                if declaration.group('end') == '{':
                    if path_attrs:
                        raise ValueError(f'#[path] on inline module needs explicit handling: {filename}')
                    stop = matching_brace(mask, declaration.end() - 1)
                    declarations[child] = Module(child, filename, True, filename, cursor)
                    visit(filename, child, declaration.end(), stop, directory / name, True)
                    cursor = stop + 1
                else:
                    if path_attrs:
                        if inline:
                            raise ValueError(f'#[path] inside inline module needs explicit handling: {filename}')
                        relative = PurePosixPath(path_attrs[0])
                        if relative.is_absolute() or '..' in relative.parts:
                            raise ValueError(f'unsafe module path: {filename}: {relative}')
                        child_file = str(PurePosixPath(filename).parent / relative)
                    else:
                        candidates = [str(directory / (name + '.rs')), str(directory / name / 'mod.rs')]
                        found = [p for p in candidates if p in sources]
                        if len(found) != 1:
                            raise ValueError(f'expected one module file for {"::".join(child)}: {found}')
                        child_file = found[0]
                    load(child_file, child, filename, cursor)
                    cursor = declaration.end()
                continue
            if mask[cursor] == '{':
                cursor = matching_brace(mask, cursor) + 1
            else:
                cursor += 1

    load('lib.rs', ())
    return declarations, files


def matching_brace(mask, start):
    depth = 0
    for cursor in range(start, len(mask)):
        depth += (mask[cursor] == '{') - (mask[cursor] == '}')
        if depth == 0:
            return cursor
    raise ValueError('unclosed Rust body')
