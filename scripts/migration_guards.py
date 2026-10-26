"""Pure, versioned adapters for path-based guards during crate extraction.

The same current source inputs are used on every invocation. No historical
snapshot or debt baseline is embedded. Unknown input shapes fail before writes.
"""
import ast
import hashlib
from pathlib import Path
from migration_plan import Change
GUARDS = ('check_computed.py', 'check_layer_imports.py', 'check_transitional.py', 'check_conventions.py', 'computed_manifest.py')
HELPERS = ('html_source_inventory.py', 'html_guard_paths.py')
MARKER = '# KaminIDE crate-split guard adapter schema 1\n'

def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f'Guard source drift: expected one occurrence of {old!r}')
    if old.startswith(('import ', 'from ')) and '\n' not in old.rstrip('\n'):
        if source.splitlines().count(old.rstrip('\n')) != 1:
            raise ValueError(f'Guard import source drift: {old!r}')
    return source.replace(old, new)

def transform(inputs):
    """Return five adapted scripts; a second pass preserves exact bytes."""
    for name in GUARDS:
        ast.parse(inputs[name], filename=name)
    marked = [inputs[name].endswith(MARKER) for name in GUARDS]
    if any(marked):
        if not all(marked):
            raise ValueError('Partially adapted source guards; inspect before migration')
        return {name: inputs[name] for name in GUARDS}
    result = {}

    def original(name):
        return inputs[name]

    def emit(name, replacements):
        source = original(name)
        for old, new in replacements:
            if source.count(old) != 1:
                raise ValueError(f'{name}: expected one exact replacement: {old!r}')
            source = replace_once(source, old, new)
        result[name] = source
    emit('check_computed.py', [('from computed_access import PREFIX, aggregate, guarded', 'from computed_access import aggregate, guarded\nfrom html_source_inventory import texts'), ("    if revision:\n        with tarfile.open(fileobj=io.BytesIO(git(root, 'archive', revision, PREFIX))) as archive:\n            return {p.name: archive.extractfile(p).read().decode('utf-8')\n                    for p in archive if p.isfile() and p.name.endswith('.rs')}\n    return {p.relative_to(root).as_posix(): p.read_text(encoding='utf-8')\n            for p in (root / PREFIX).rglob('*.rs')}", '    return texts(root, revision)'), ('import io\n', ''), ('import tarfile\n', '')])
    emit('check_layer_imports.py', [("STAGE = {'dom': 0, 'style': 1, 'layout': 2, 'text': 2, 'paint': 3, 'svg': 3, 'render': 4}", "STAGE = {'dom': 0, 'box_facts': 0, 'style': 1, 'layout': 2, 'text': 2, 'paint': 3, 'svg': 3, 'render': 4}"), ('from rust_layer_exports import Exports', 'from rust_layer_exports import Exports\nfrom html_source_inventory import PREFIX, read\nfrom html_guard_paths import normalize'), ("    folder = root / 'crates/html/src'\n    sources = {p.relative_to(folder).as_posix(): p.read_text(encoding='utf-8')\n               for p in sorted(folder.rglob('*.rs'))}", '    sources = {name.removeprefix(PREFIX): normalize(item.text)\n               for name, item in read(root).items()}')])
    emit('check_transitional.py', [('from rust_layer_paths import TOKEN', 'from rust_layer_paths import TOKEN\nfrom html_source_inventory import PACKAGES, read as html_sources'), ("ROOTS = ('crates/html', 'vendor')", "ROOTS = (*dict.fromkeys(str(PurePosixPath(prefix).parent)\n                        for prefix in PACKAGES.values()), 'vendor')"), ("    # Scan all vendor Rust so adapter moves/renames cannot silently escape.\n    for folder in ROOTS:\n        for filename in sorted((root / folder).rglob('*.rs')):\n            relative = filename.relative_to(root).as_posix()\n            try:\n                for task, reason, line in markers(filename.read_text(encoding='utf-8')):\n                    found.append((relative, task, reason, line))\n            except ValueError as error:\n                raise ValueError(f'{relative}: {error}') from error", "    sources = html_sources(root)\n    files = {name: item.text for name, item in sources.items()}\n    physical = {item.physical for item in sources.values()}\n    # Preserve examples and all vendor Rust in addition to phase sources.\n    for folder in ROOTS:\n        for filename in sorted((root / folder).rglob('*.rs')):\n            relative = filename.relative_to(root).as_posix()\n            if relative not in physical:\n                files[relative] = filename.read_text(encoding='utf-8')\n    for relative, text in sorted(files.items()):\n        try:\n            for task, reason, line in markers(text):\n                found.append((relative, task, reason, line))\n        except ValueError as error:\n            raise ValueError(f'{relative}: {error}') from error"), ('    errors = []\n    for file in sorted({file for file, _, _ in current}):', '    errors = []\n    before = html_sources(root, revision)\n    now = html_sources(root)\n    for file in sorted({file for file, _, _ in current}):'), ("        old_lines = {line.strip() for line in previous.splitlines() if line.strip()}\n        source = (root / file).read_text(encoding='utf-8') if (root / file).exists() else ''\n", "        if file in before:\n            present, previous = file, before[file].text\n        old_lines = {line.strip() for line in previous.splitlines() if line.strip()}\n        source = now[file].text if file in now else (\n            (root / file).read_text(encoding='utf-8') if (root / file).exists() else '')\n")])
    source = original('check_conventions.py')
    source = replace_once(source, 'from collections import defaultdict', 'from collections import defaultdict\nimport difflib')
    at = source.index('\ndef git(')
    source = source[:at] + '\nsys.path.insert(0, str(Path(__file__).resolve().parent))\nfrom html_source_inventory import texts, PREFIX\nfrom html_guard_paths import logical_path, normalize\n\n' + source[at:]
    start, end = (source.index('def tree_sources('), source.index('\n\nRULES ='))
    tree = ast.dump(ast.parse(source[start:end]), include_attributes=False)
    if hashlib.sha256(tree.encode()).hexdigest() != '0e39820697399bf37336423696913b96b096d69008a2b60e342e7cd1558df3df':
        raise ValueError('Conventions tree_sources structure changed; review adapter')
    source = source[:start] + 'def tree_sources(revision):\n    """Use phase identities so a crate move does not reset inherited debt."""\n    return {name: normalize(text) for name, text in texts(REPO, revision).items()}\n\n\ndef convention_diff(base, head, old, now):\n    """Preserve other crates/vendor; diff engine files by stable phase identity."""\n    skip = False\n    for line in git("diff", "-U0", f"{base}..{head}", "--", "crates", "vendor").decode(\n            "utf-8", "replace").splitlines():\n        if line.startswith("diff --git "):\n            names = line.split()[2:4]\n            skip = any(logical_path(name[2:]).startswith(PREFIX) for name in names)\n        if not skip:\n            yield line\n    for name in sorted(old.keys() | now.keys()):\n        if old.get(name) == now.get(name):\n            continue\n        yield from difflib.unified_diff(old.get(name, "").splitlines(),\n                                        now.get(name, "").splitlines(),\n                                        fromfile="a/" + name, tofile="b/" + name, n=0)\n' + source[end:]
    source = replace_once(source, '    diff_range = f"{base}..{head}"', '    sources, old_sources = tree_sources(head), tree_sources(base)')
    source = replace_once(source, '    diff = git("diff", "-U0", diff_range, "--", "crates", "vendor").decode("utf-8", "replace")\n    for line in diff.splitlines():', '    for line in convention_diff(base, head, old_sources, sources):')
    source = replace_once(source, '    changed = git("diff", "--name-only", "--diff-filter=AM", diff_range,\n                  "--", "crates/html/src").decode().splitlines()\n    changed = [f for f in changed if f.endswith(".rs")]', '    changed = [name for name in sources if sources[name] != old_sources.get(name)]')
    source = replace_once(source, '        sources, old_sources = tree_sources(head), tree_sources(base)\n', '')
    if 'diff_range' in source:
        raise ValueError('Conventions replacement missed a Git range')
    result['check_conventions.py'] = source
    emit('computed_manifest.py', [('import re\n', 'import re\nimport sys\n\nsys.path.insert(0, str(Path(__file__).resolve().parent))\nfrom html_source_inventory import texts\n'), ('SRC = ROOT / "crates/html/src"\n', ''), ('FIELDS = SRC / "style/computed/fields.rs"', 'DEFINITIONS = "crates/html/src/style/computed/fields.rs"'), ('def fields():', 'def fields(source):'), ('FIELDS.read_text(encoding="utf-8").splitlines()', 'source.splitlines()'), ('    definitions = fields()', '    sources = texts(ROOT)\n    definitions = fields(sources[DEFINITIONS])'), ('    for path in sorted(SRC.rglob("*.rs")):\n        for line, text in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):', '    for path, source in sorted(sources.items(), key=lambda item: Path(item[0])):\n        for line, text in enumerate(source.splitlines(), 1):'), ('sites[name].append([relative(path), line, kind, stripped])', 'sites[name].append([path, line, kind, stripped])'), ('apis.append([relative(path), line, stripped])', 'apis.append([path, line, stripped])'), ('FIELDS.read_text(encoding="utf-8").encode()', 'sources[DEFINITIONS].replace("\\r\\n", "\\n").encode()'), ('def relative(path):\n    return path.relative_to(ROOT).as_posix()\n\n\n', '')])
    for name, text in result.items():
        ast.parse(text, filename=name)
    return {name: result[name].rstrip('\n') + '\n\n' + MARKER for name in GUARDS}

def plan(root, helper_root=None):
    """Include guard adapters and helpers in the same exact migration plan."""
    root = Path(root)
    helper_root = Path(helper_root or Path(__file__).resolve().parent)
    inputs = {name: (root / 'scripts' / name).read_bytes().decode('utf-8') for name in GUARDS}
    after = transform(inputs)
    for name in HELPERS:
        after[name] = (helper_root / name).read_bytes().decode('utf-8')
        path = root / 'scripts' / name
        inputs[name] = path.read_bytes().decode('utf-8') if path.is_file() else ''
    return [Change('scripts/' + name, 'scripts/' + name, inputs[name], text) for name, text in after.items() if inputs[name] != text]
