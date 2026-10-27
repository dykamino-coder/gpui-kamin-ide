"""Preflight and apply an exact source plan without overwriting unrelated files."""
from pathlib import Path


def target(root, relative):
    root = Path(root).resolve()
    path = (root / relative).resolve()
    if path == root or not path.is_relative_to(root):
        raise ValueError(f'Migration path escapes the checkout: {relative}')
    # Resolve existing reparse points as well: a repository path must never
    # turn an exact rewrite into a write in another worktree or user directory.
    return path


def preflight(root, changes):
    root = Path(root).resolve()
    sources, destinations = {}, {}
    for change in changes:
        old, new = target(root, change.old), target(root, change.new)
        if old in sources or new in destinations:
            raise ValueError(f'Duplicate migration path: {change.old} -> {change.new}')
        sources[old] = change
        destinations[new] = change
        for parent in new.parents:
            if parent == root:
                break
            if parent.exists() and not parent.is_dir():
                raise ValueError(f'Destination parent is not a directory: {parent.relative_to(root)}')
        if old.exists() and not old.is_file():
            raise ValueError(f'Source is not a regular file: {change.old}')
        actual = old.read_bytes() if old.is_file() else None
        expected = change.before.encode('utf-8')
        # Newly generated roots/manifests have old == new and no input file.
        if actual != expected and not (actual is None and old == new and not expected):
            raise ValueError(f'Source changed since planning: {change.old}')
        if old != new and new.exists():
            raise ValueError(f'Destination already exists: {change.new}')
    if set(sources) & set(destinations) != {p for p, c in sources.items() if c.old == c.new}:
        raise ValueError('Migration contains a rename chain; an explicit staging rule is required')


def apply(root, changes, write=None):
    """Validate the whole plan first; restore only exact own writes on failure.

    No wildcard deletion or directory cleanup is performed. The caller must
    ensure Cargo is idle and the tree is stable before invoking the operation.
    An interrupted process can leave a partial tree, so production invocation
    additionally requires a clean Git tree for an inspectable recovery diff.
    """
    changes = list(changes)
    preflight(root, changes)
    write = write or (lambda path, data: path.write_bytes(data))
    written, removed = [], []
    try:
        for change in changes:
            path = target(root, change.new)
            previous = path.read_bytes() if path.is_file() else None
            path.parent.mkdir(parents=True, exist_ok=True)
            # Record before calling the writer: a writer may fail partway.
            written.append((path, previous))
            write(path, change.after.encode('utf-8'))
        # No source is removed until every destination write has succeeded.
        for change in changes:
            if change.old != change.new:
                path = target(root, change.old)
                removed.append((path, path.read_bytes()))
                path.unlink()
    except BaseException:
        for path, data in reversed(removed):
            path.write_bytes(data)
        for path, data in reversed(written):
            if data is None:
                if path.exists():
                    path.unlink()
            else:
                path.write_bytes(data)
        raise
