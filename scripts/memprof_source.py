"""Verify profiler build inputs without borrowing a parent checkout's identity."""

import hashlib
import pathlib
import subprocess


def git(repo, *args):
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True)
    if result.returncode:
        raise SystemExit(result.stderr.decode(errors="replace"))
    return result.stdout


def digest(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def blob(data):
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def linked(path):
    return path.is_symlink() or bool(getattr(path.lstat(), "st_file_attributes", 0) & 0x400)


def export(repo, commit, source):
    """An export must match every blob and contain no additional build inputs.

    Blobs must match byte for byte. Submodules and symlinks fail closed:
    neither a followed link nor an unaudited nested tree is source evidence.
    Generated target output is excluded, never added to the source digest.
    """
    source = pathlib.Path(source).resolve()
    commit = git(repo, "rev-parse", "--verify", f"{commit}^{{commit}}").decode().strip()
    entries = {}
    tree = git(repo, "ls-tree", "-rz", "--full-tree", commit)
    for row in tree.split(b"\0"):
        if not row:
            continue
        meta, raw_name = row.split(b"\t", 1)
        mode, kind, oid = meta.decode().split()
        name = raw_name.decode("utf-8")
        if kind != "blob" or mode not in ("100644", "100755"):
            raise SystemExit(f"unsupported export entry: {name} ({mode} {kind})")
        path = source / name
        if not path.resolve().is_relative_to(source) or not path.is_file() or linked(path):
            raise SystemExit(f"missing or linked export entry: {name}")
        data = path.read_bytes()
        if oid != blob(data):
            raise SystemExit(f"export differs from {commit}: {name}")
        entries[name] = oid
    # Cargo build scripts can read arbitrary extra files, not only *.rs.
    pending = [source]
    while pending:
        directory = pending.pop()
        for path in directory.iterdir():
            name = path.relative_to(source).as_posix()
            if name == "target":
                continue
            if linked(path) or not path.resolve().is_relative_to(source):
                raise SystemExit(f"linked export input: {name}")
            if path.is_dir():
                pending.append(path)
            elif name not in entries:
                raise SystemExit(f"untracked export input: {name}")
    return {"kind": "verified-export", "commit": commit, "dirty": False,
            "tree": git(repo, "rev-parse", f"{commit}^{{tree}}").decode().strip(),
            "verified_blobs": len(entries), "tree_listing_sha256": hashlib.sha256(tree).hexdigest()}


def identity(worktree, repo=None, commit=None):
    worktree = pathlib.Path(worktree).resolve()
    if bool(repo) != bool(commit):
        raise SystemExit("--source-repo and --source-commit must be provided together")
    if repo:
        return export(pathlib.Path(repo).resolve(), commit, worktree)
    top = pathlib.Path(git(worktree, "rev-parse", "--show-toplevel").decode().strip()).resolve()
    if top != worktree:
        raise SystemExit("source is not a checkout root; use --source-repo and --source-commit for an export")
    return {"kind": "checkout", "commit": git(worktree, "rev-parse", "HEAD").decode().strip(),
            "dirty": bool(git(worktree, "status", "--porcelain", "--untracked-files=all")),
            "tree": git(worktree, "rev-parse", "HEAD^{tree}").decode().strip()}
