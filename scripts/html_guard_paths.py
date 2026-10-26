"""Stable phase paths and explicit engine namespaces for source-based guards.

Normalization is an analysis view only. It never rewrites Rust, comments,
literals, aliases or a namespace embedded inside another qualified path.
"""
import re

from html_source_inventory import PACKAGES, logical_name
from rust_layer_paths import TOKEN, use_tree


NAMESPACES = {'kamin_html', 'kamin_style', 'kamin_layout', 'kamin_paint', 'kamin_font_facts'}
PATH_KEYWORDS = {'use', 'return', 'break', 'as', 'impl', 'for', 'type', 'dyn', 'where', 'in'}


def logical_path(physical):
    for package, prefix in PACKAGES.items():
        if physical.startswith(prefix) and physical.endswith('.rs'):
            return logical_name(package, physical[len(prefix):])
    return physical


def normalize(source):
    """Keep every diagnostic line and comment/literal byte unchanged."""
    offset, words = 0, []
    while match := TOKEN.search(source, offset):
        word, end = match.group(), match.end()
        keep = True
        if word == '/*':
            depth = 1
            for delimiter in re.finditer(r'/\*|\*/', source[end:]):
                depth += 1 if delimiter.group() == '/*' else -1
                if not depth:
                    end += delimiter.end()
                    break
            else:
                end = len(source)
            keep = False
        elif match.group('hashes') is not None:
            closing = '"' + match.group('hashes')
            stop = source.find(closing, end)
            end = len(source) if stop < 0 else stop + len(closing)
            keep = False
        elif word.startswith('//') or '"' in word or (len(word) > 1 and word.endswith("'")):
            keep = False
        if keep:
            words.append((word.removeprefix('r#'), match.start(), end))
        offset = end
    plain = [(word, 0) for word, _, _ in words]
    for i, (word, _, _) in enumerate(words):
        if word == 'use':
            leaves, _ = use_tree(plain, i + 1, aliases=True)
            if any(len(path) == 1 and path[0] in NAMESPACES for path, _, _ in leaves):
                raise ValueError('Whole engine-crate imports require explicit module paths')
        if (word == 'extern' and i + 2 < len(words) and words[i + 1][0] == 'crate'
                and words[i + 2][0] in NAMESPACES):
            raise ValueError('Engine extern-crate aliases require explicit module paths')
    edits = []
    for i, (word, start, end) in enumerate(words):
        if word not in NAMESPACES or i + 1 == len(words) or words[i + 1][0] != '::':
            continue
        if i and words[i - 1][0] == '::':
            # A leading global :: belongs to this crate path; a preceding
            # identifier means the namespace is a member of some other path.
            before = words[i - 2][0] if i >= 2 else None
            if before and ((before.isidentifier() and before not in PATH_KEYWORDS)
                           or before in (')', ']', '>')):
                continue
            edits.append((words[i - 1][1], words[i - 1][2], ''))
        edits.append((start, end, 'crate'))
    for start, end, replacement in sorted(edits, reverse=True):
        source = source[:start] + replacement + source[end:]
    return source
