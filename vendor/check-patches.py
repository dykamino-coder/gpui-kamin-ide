#!/usr/bin/env python3
"""Rebuild every clean upstream base from pinned sources and round-trip each vendor patch.

Crate archives come from crates.io and are verified by SHA-256; Git sources are
fetched by full commit id, which verifies their content. Each base is then passed
to `vendor/regenerate-patch.py --check`, so a stale patch, a patch that no longer
applies, or a vendor tree that differs from base + patch fails the run.
"""

import argparse
import hashlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
REGENERATE = ROOT / "vendor" / "regenerate-patch.py"

# crates.io archives: name, version, SHA-256 from the crates.io index.
CRATES = {
    "gpui-pre-0.3.8": "d7f0bc3ebba070f8981b3f8e905a12f47d27794a30d029e59cdb9edf7a4b3360",
    "gpui-pre-windows-0.3.8": "371f9bb13f97e3bea2f8b8e5e81232bb74d735153c244ae67f7400e743925b1c",
    "gpui-base-0.7.1": "3cf698677b253118459f552fe15513858c669d79a25837a94e480db2ae330050",
    "gpui-component-0.7.1": "bc8b7def8e74b0d822d3e96f6b28fb5a2c20eb7a83d30d22fa59ca8088d1159b",
    "html5ever-0.40.1": "456a1a377e608e555d22ddab27ac0114bc7a7b4199078108e34c2aeae6c9b130",
    "markup5ever_rcdom-0.39.0+unofficial": "3ac010f19d6c4af81eeb4018a39d7a115de9d285af45c126a4ac02e6fc5716b7",
    "resvg-0.48.1": "67e3803f97b999e80cbf7c6ecdd07a8102204d92e1633cf48783720c521196bd",
    "usvg-0.48.1": "977d0a4abdef933f424a99fe09f95576e089b90aebc6f016a3bc813762493e91",
}
RESVG_GIT = ("https://github.com/linebender/resvg", "77b12f4951a7b75d759c42e8c9497cff7c839c3c")
TAFFY_GIT = ("https://github.com/DioxusLabs/taffy", "2d936b7701f64b0fe95e50f1bfdcfc65a9f88f6f")

# vendor crate -> archive it starts from (taffy is packaged from Git instead).
ARCHIVE_BASES = {
    "gpui": "gpui-pre-0.3.8",
    "gpui-windows": "gpui-pre-windows-0.3.8",
    "gpui-base": "gpui-base-0.7.1",
    "gpui-component": "gpui-component-0.7.1",
    "html5ever": "html5ever-0.40.1",
    "markup5ever_rcdom": "markup5ever_rcdom-0.39.0+unofficial",
    "resvg": "resvg-0.48.1",
    "usvg": "usvg-0.48.1",
}
ALL = [*ARCHIVE_BASES, "taffy"]


def unpack(data, dest):
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        archive.extractall(dest, filter="data")


def fetch_crate(stem, work):
    dest = work / "crates" / stem
    if dest.is_dir():
        return dest
    name, version = stem.rsplit("-", 1)
    url = f"https://static.crates.io/crates/{name}/{stem.replace('+', '%2B')}.crate"
    with urllib.request.urlopen(url, timeout=120) as response:
        data = response.read()
    digest = hashlib.sha256(data).hexdigest()
    if digest != CRATES[stem]:
        raise SystemExit(f"{stem}: SHA-256 {digest} does not match the pinned {CRATES[stem]}")
    unpack(data, work / "crates")
    if not dest.is_dir():
        raise SystemExit(f"{stem}: archive did not contain {stem}/ (version {version})")
    return dest


def fetch_git(url, commit, work):
    dest = work / "git" / commit[:12]
    if (dest / ".git").is_dir():
        return dest
    dest.mkdir(parents=True)
    git = ["git", "-c", "advice.detachedHead=false", "-C", str(dest)]
    subprocess.run([*git, "init", "-q"], check=True)
    subprocess.run([*git, "fetch", "-q", "--depth", "1", url, commit], check=True)
    subprocess.run([*git, "checkout", "-q", "FETCH_HEAD"], check=True)
    actual = subprocess.check_output([*git, "rev-parse", "HEAD"], text=True).strip()
    if actual != commit:
        raise SystemExit(f"{url}: fetched {actual}, expected {commit}")
    return dest


def svg_base(crate, work):
    """crates.io 0.48.1 archive whose src/ is replaced by the pinned resvg commit."""
    archive = fetch_crate(ARCHIVE_BASES[crate], work)
    base = work / "bases" / crate
    shutil.copytree(archive, base)
    shutil.rmtree(base / "src")
    shutil.copytree(fetch_git(*RESVG_GIT, work) / "crates" / crate / "src", base / "src")
    return base


def taffy_base(work):
    """`cargo package` of the pinned Taffy commit (docs/VENDORING.md)."""
    checkout = fetch_git(*TAFFY_GIT, work)
    target = work / "taffy-target"
    # Package with the repository toolchain: the normalized manifest depends on Cargo.
    toolchain = tomllib.loads((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))
    env = dict(os.environ, CARGO_TARGET_DIR=str(target),
               RUSTUP_TOOLCHAIN=toolchain["toolchain"]["channel"])
    subprocess.run(["cargo", "package", "--allow-dirty", "--no-verify", "--quiet"],
                   cwd=checkout, env=env, check=True)
    packages = sorted((target / "package").glob("taffy-*.crate"))
    if len(packages) != 1:
        raise SystemExit(f"taffy: expected one packaged crate, found {packages}")
    out = work / "bases" / "taffy"
    out.mkdir(parents=True)
    unpack(packages[0].read_bytes(), out)
    base = out / packages[0].name.removesuffix(".crate")
    (base / "Cargo.lock").write_bytes(recorded_lock("taffy", work))
    return base


def recorded_lock(crate, work):
    """Lockfile `cargo package` produced when the patch was generated.

    Packaging resolves the newest compatible dependencies, so a fresh lockfile
    depends on the registry date rather than the pinned commit. Recover the
    recorded one by reversing the patch's lockfile section on the vendor copy;
    every other file of the base is still verified independently.
    """
    rel = f"vendor/{crate}/Cargo.lock"
    patch = (ROOT / "vendor" / f"{crate}-kaminide.patch").read_bytes()
    header = f"diff --git a/{rel} b/{rel}\n".encode()
    lock = (ROOT / rel).read_bytes().replace(b"\r\n", b"\n")
    start = patch.find(header)
    if start < 0:
        return lock
    end = patch.find(b"\ndiff --git ", start + len(header))
    section = patch[start:] if end < 0 else patch[start:end + 1]
    temp = work / "lock-reverse"
    shutil.rmtree(temp, ignore_errors=True)
    (temp / "vendor" / crate).mkdir(parents=True)
    (temp / rel).write_bytes(lock)
    subprocess.run(["git", "apply", "-R", "--unidiff-zero", "--whitespace=nowarn", "-"],
                   input=section, cwd=temp, check=True)
    return (temp / rel).read_bytes()


def base_for(crate, work):
    if crate == "taffy":
        return taffy_base(work)
    if crate in ("resvg", "usvg"):
        return svg_base(crate, work)
    return fetch_crate(ARCHIVE_BASES[crate], work)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("crates", nargs="*", help=f"vendor crates to check (default: {' '.join(ALL)})")
    parser.add_argument("--work", type=Path, help="reusable download/base directory")
    args = parser.parse_args()
    crates = args.crates or ALL
    unknown = sorted(set(crates) - set(ALL))
    if unknown:
        parser.error(f"unknown vendor crates: {', '.join(unknown)}")
    with tempfile.TemporaryDirectory(prefix="vendor-bases-") as temp:
        work = (args.work or Path(temp)).resolve()
        shutil.rmtree(work / "bases", ignore_errors=True)
        (work / "bases").mkdir(parents=True)
        scratch = work / "scratch"
        scratch.mkdir(exist_ok=True)
        failed = []
        for crate in crates:
            base = base_for(crate, work)
            result = subprocess.run([sys.executable, str(REGENERATE), crate, str(base),
                                     "--scratch", str(scratch), "--check"], cwd=ROOT)
            if result.returncode:
                failed.append(crate)
        if failed:
            raise SystemExit(f"Vendor patch round-trip failed: {', '.join(failed)}")


if __name__ == "__main__":
    main()
