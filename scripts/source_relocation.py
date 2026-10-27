"""Plan source ownership and preserve compile-time include targets on relocation."""
import posixpath
from pathlib import PurePosixPath
import re

from rewrite_paths import lexemes

PACKAGE_DIRS = {'kamin-html': 'html', 'kamin-style': 'html-style',
                'kamin-layout': 'html-layout', 'kamin-paint': 'html-paint',
                'kamin-font-facts': 'font-facts'}


def normalized(path):
    result = posixpath.normpath(str(path))
    if result == '..' or result.startswith('../') or result.startswith('/'):
        raise ValueError(f'Path escapes workspace: {path}')
    return result


def source_locations(names, owners):
    result = {}
    for name in sorted(names):
        root = name.split('/')[0].removesuffix('.rs')
        if name == 'lib.rs':
            owner = 'kamin-html'
        elif root not in owners:
            raise ValueError(f'Unassigned source owner: {name}')
        else:
            owner = owners[root]
        if owner not in PACKAGE_DIRS:
            raise ValueError(f'Unknown package directory: {owner}')
        package = PACKAGE_DIRS[owner]
        result['crates/html/src/' + name] = 'crates/' + package + '/src/' + name
    if len(set(result.values())) != len(result):
        raise ValueError('Two sources map to the same destination')
    return result


def rust_literal(value):
    # Reject escapes rather than applying Python's different string semantics.
    if re.fullmatch(r'"[^"\\\r\n]*"', value):
        return value[1:-1]
    match = re.fullmatch(r'r(?P<hashes>#*)"(?P<body>.*?)"(?P=hashes)', value, re.S)
    if match:
        return match['body']
    raise ValueError(f'Unsupported include path literal: {value[:80]}')


def relocate_includes(source, old_file, locations, exists):
    """Rewrite only include macro operands; verify targets and known env paths.

    Locations are workspace-relative. exists is a set of original input files.
    Literal includes retain the exact target even across crate ownership. The
    single current CARGO_MANIFEST_DIR form references a sibling crate asset;
    its target is checked at both old and new manifest locations.
    """
    new_file = locations.get(old_file, old_file)
    tokens = list(lexemes(source))
    words = [(word, start, end) for word, _, start, end in tokens
             if word is not None or (source[start:end].startswith(('"', 'r"', 'r#')))]
    edits, checked = [], []
    for index, (word, start, end) in enumerate(words):
        if word not in ('include', 'include_str', 'include_bytes'):
            continue
        if [w[0] for w in words[index + 1:index + 3]] != ['!', '(']:
            continue
        remaining = words[index + 3:]
        if not remaining:
            raise ValueError(f'Missing include operand: {old_file}')
        operand, left, right = remaining[0]
        if operand is None:
            if len(remaining) < 2 or remaining[1][0] != ')':
                raise ValueError(f'Unsupported include operands: {old_file}:{left}')
            literal = rust_literal(source[left:right])
            target = normalized(PurePosixPath(old_file).parent / literal)
            if target not in exists:
                raise ValueError(f'Missing include target: {old_file}: {target}')
            new_target = locations.get(target, target)
            replacement = posixpath.relpath(new_target, str(PurePosixPath(new_file).parent))
            if replacement != literal:
                edits.append((left, right, '"' + replacement + '"'))
            checked.append((target, new_target))
        elif operand == 'concat':
            # Treat a macro expression as a contract, never as a string guess.
            stop = index + 3
            depth = 1
            while stop < len(words) and depth:
                depth += (words[stop][0] == '(') - (words[stop][0] == ')')
                stop += 1
            expression = source[left:words[stop - 1][1]]
            match = re.fullmatch(r'concat!\(\s*env!\("CARGO_MANIFEST_DIR"\),\s*'
                                 r'"(?P<suffix>[^"\\]+)"\s*\)\s*', expression)
            if not match:
                raise ValueError(f'Unsupported include expression: {old_file}:{left}')
            def manifest(filename):
                parts = PurePosixPath(filename).parts
                if parts[:1] != ('crates',) or len(parts) < 4 or parts[2] != 'src':
                    raise ValueError(f'Unexpected package path: {filename}')
                return PurePosixPath(*parts[:2])
            suffix = match['suffix'].lstrip('/')
            target = normalized(manifest(old_file) / suffix)
            new_target = normalized(manifest(new_file) / suffix)
            if target not in exists or new_target != locations.get(target, target):
                raise ValueError(f'Manifest-relative include changes target: {old_file}: {target}')
            checked.append((target, new_target))
        else:
            raise ValueError(f'Unsupported include expression: {old_file}:{left}')
    for left, right, replacement in sorted(edits, reverse=True):
        source = source[:left] + replacement + source[right:]
    return source, checked
