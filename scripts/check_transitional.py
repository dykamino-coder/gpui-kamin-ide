"""Check explicit temporary paths in HTML and vendor Rust comments.

Each TRANSITIONAL(task): reason needs one exact file/task/reason registry entry.
Line numbers are diagnostic only. The Git merge-base inventory may only shrink.
Installation without a base registry can inventory existing code only.
"""

import argparse
from collections import Counter
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from rust_layer_paths import TOKEN
from html_source_inventory import PACKAGES, read as html_sources


REPO = Path(__file__).resolve().parent.parent
REGISTRY = 'scripts/transitional_registry.json'
ROOTS = (*dict.fromkeys(str(PurePosixPath(prefix).parent)
                        for prefix in PACKAGES.values()), 'vendor')
TASK = r'[A-Za-z0-9][A-Za-z0-9_.-]*'
MARKER = re.compile(rf'\bTRANSITIONAL\(({TASK})\):[ \t]*([^\n]+)')


def comment_spans(source):
    """Yield whole comment spans, skipping Rust literals and nesting."""
    offset = 0
    while match := TOKEN.search(source, offset):
        word, end = match.group(), match.end()
        if word.startswith('//'):
            yield match.start(), end
        elif word == '/*':
            depth = 1
            for delimiter in re.finditer(r'/\*|\*/', source[end:]):
                depth += 1 if delimiter.group() == '/*' else -1
                if not depth:
                    end += delimiter.end()
                    break
            else:
                end = len(source)
            yield match.start(), end
        elif match.group('hashes') is not None:
            closing = '"' + match.group('hashes')
            stop = source.find(closing, end)
            end = len(source) if stop < 0 else stop + len(closing)
        offset = end


def comments(source):
    """Yield comment text and its line, removing nested delimiters."""
    for start, end in comment_spans(source):
        text = source[start:end]
        text = text[2:] if text.startswith('//') else re.sub(r'/\*|\*/', '  ', text)
        yield text, source.count('\n', 0, start) + 1


def anchors(source):
    """Map markers to the first nonempty code line after their whole comment."""
    spans = list(comment_spans(source))
    code = list(source)
    for start, end in spans:
        code[start:end] = ['\n' if char == '\n' else ' ' for char in source[start:end]]
    code = ''.join(code)
    result = {}
    for start, end in spans:
        for task, reason, line in markers(source[start:end]):
            key = (task, reason, source.count('\n', 0, start) + line)
            result[key] = next((raw[len(masked) - len(masked.lstrip()):].strip() for raw, masked in
                                zip(source[end:].splitlines(), code[end:].splitlines())
                                if masked.strip()), None)
    return result


def markers(source):
    """Yield task/reason/line; a malformed annotation must not hide debt."""
    for comment, line in comments(source):
        for index, text in enumerate(comment.splitlines()):
            if not re.search(r'\bTRANSITIONAL\b', text):
                continue
            match = MARKER.search(text)
            if (not match or not match[2].strip()
                    or len(re.findall(r'\bTRANSITIONAL\b', text)) != 1):
                raise ValueError(f'line {line + index}: malformed TRANSITIONAL marker')
            yield match[1], match[2].strip(), line + index


def inventory(root):
    found = []
    sources = html_sources(root)
    files = {name: item.text for name, item in sources.items()}
    physical = {item.physical for item in sources.values()}
    # Preserve examples and all vendor Rust in addition to phase sources.
    for folder in ROOTS:
        for filename in sorted((root / folder).rglob('*.rs')):
            relative = filename.relative_to(root).as_posix()
            if relative not in physical:
                files[relative] = filename.read_text(encoding='utf-8')
    for relative, text in sorted(files.items()):
        try:
            for task, reason, line in markers(text):
                found.append((relative, task, reason, line))
        except ValueError as error:
            raise ValueError(f'{relative}: {error}') from error
    return found


def entries(data):
    if not isinstance(data, list):
        raise ValueError('registry must be a JSON array')
    result = set()
    for item in data:
        if not isinstance(item, dict) or set(item) != {'file', 'task', 'reason'}:
            raise ValueError('registry entries need exactly file, task and reason')
        if any(not isinstance(value, str) or not value.strip()
               or value != value.strip() or '\n' in value or '\r' in value
               for value in item.values()):
            raise ValueError(f'registry values must be nonempty single-line strings: {item}')
        file, task, reason = item['file'], item['task'], item['reason']
        path = PurePosixPath(file)
        if (not any(file.startswith(folder + '/') for folder in ROOTS)
                or path.suffix != '.rs' or path.as_posix() != file
                or '..' in path.parts or re.search(r'[\\*?\[\]:]', file)):
            raise ValueError(f'registry needs an exact scoped Rust file: {file}')
        if not re.fullmatch(TASK, task):
            raise ValueError(f'invalid registry task: {task}')
        key = (file, task, reason)
        if key in result:
            raise ValueError(f'duplicate registry entry: {key}')
        result.add(key)
    return result


def growth(root, base, current):
    revision = subprocess.check_output(
        ['git', '-C', str(root), 'merge-base', base, 'HEAD'], text=True,
    ).strip()
    files = subprocess.check_output(
        ['git', '-C', str(root), 'ls-tree', '--name-only', revision, '--', REGISTRY],
        text=True,
    )
    if not files.strip():
        return installation(root, revision, current)
    previous = entries(json.loads(subprocess.check_output(
        ['git', '-C', str(root), 'show', f'{revision}:{REGISTRY}'], encoding='utf-8',
    )))
    if len(current) > len(previous):
        return [f'transitional inventory grew: {len(previous)} -> {len(current)}; '
                f'shrink registry (base {revision})']
    return []


def installation(root, revision, current):
    """A first registry may label old code, never introduce a temporary path."""
    errors = []
    before = html_sources(root, revision)
    now = html_sources(root)
    for file in sorted({file for file, _, _ in current}):
        present = subprocess.check_output(
            ['git', '-C', str(root), 'ls-tree', '--name-only', revision, '--', file],
            text=True,
        ).strip()
        previous = subprocess.check_output(
            ['git', '-C', str(root), 'show', f'{revision}:{file}'], encoding='utf-8',
        ) if present else ''
        if file in before:
            present, previous = file, before[file].text
        old_lines = {line.strip() for line in previous.splitlines() if line.strip()}
        source = now[file].text if file in now else (
            (root / file).read_text(encoding='utf-8') if (root / file).exists() else '')
        following = anchors(source)
        for task, reason, line in markers(source):
            if ((file, task, reason) in current
                    and (not present or following[task, reason, line] not in old_lines)):
                errors.append(f'{file}:{line}: installation requires existing base code after '
                              f'TRANSITIONAL({task}) (base {revision})')
    return errors


def check(root, base=None):
    registered = entries(json.loads((root / REGISTRY).read_text(encoding='utf-8')))
    found = inventory(root)
    counts = Counter((file, task, reason) for file, task, reason, _ in found)
    errors = []
    for key, count in sorted(counts.items()):
        file, task, reason = key
        lines = ','.join(str(line) for f, t, r, line in found if (f, t, r) == key)
        if key not in registered:
            errors.append(f'{file}:{lines}: unregistered TRANSITIONAL({task}): {reason}')
        if count > 1:
            errors.append(f'{file}:{lines}: duplicate transitional identity; '
                          'use a distinct reason for each temporary path')
    for file, task, reason in sorted(registered - counts.keys()):
        errors.append(f'stale registry entry: {file} TRANSITIONAL({task}): {reason}; '
                      'shrink registry')
    if base:
        errors.extend(growth(root, base, registered))
    return errors, len(found), len(registered)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', type=Path, default=REPO)
    parser.add_argument('--base', default='origin/main',
                        help='Git base revision (default: origin/main); count may only shrink')
    args = parser.parse_args()
    try:
        errors, found, registered = check(args.root.resolve(), args.base)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f'transitional paths: {error}', file=sys.stderr)
        return 1
    for error in errors:
        print(error, file=sys.stderr)
    print(f'transitional paths: {len(errors)} errors; {found} markers '
          f'in {registered} registry entries')
    return bool(errors)


if __name__ == '__main__':
    sys.exit(main())

# KaminIDE crate-split guard adapter schema 1
