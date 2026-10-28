#!/usr/bin/env python3
"""Build every target of the vendored GPUI forks standalone and run their tests.

`vendor/gpui`, `vendor/gpui-base` and `vendor/gpui-component` are excluded
from the root workspace (docs/VENDORING.md), so `cargo clippy/test
--workspace` never compiles their unit tests, integration tests, benches or
examples, and Cargo resolves dev-dependencies only for the root package of a
build. This check builds each fork as its own root with the root
`[patch.crates-io]` overrides passed through `--config`, so the forks and the
platform crate resolve to the same local forks as the application.

Each run is seeded with the root `Cargo.lock`: shared packages keep the
application's exact versions and only the fork's dev-only packages are
resolved. The seeded lockfile is removed afterwards; nothing is written into
the vendor tree.
"""

import argparse
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def root_manifest() -> dict:
    return tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))


def crate_features(manifest: dict) -> dict[str, str]:
    # Standalone feature sets from docs/VENDORING.md: the forks' local recursive
    # layout uses StackSafe; gpui-component also needs the application's
    # tree-sitter features so the highlighting tests are compiled.
    component = manifest["workspace"]["dependencies"]["gpui-component"]["features"]
    return {
        "gpui": "stacker,test-support",
        "gpui-base": "gpui/stacker",
        "gpui-component": ",".join(["gpui/stacker", *component]),
    }


def patch_configs(manifest: dict) -> list[str]:
    args = []
    for name, spec in manifest["patch"]["crates-io"].items():
        path = (ROOT / spec["path"]).resolve().as_posix()
        args += ["--config", f"patch.crates-io.{name}.path='{path}'"]
    return args


def cargo(*args: str) -> None:
    command = ["cargo", *args]
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def check(crate: str, features: str, options, patches: list[str]) -> None:
    directory = ROOT / "vendor" / crate
    lock = directory / "Cargo.lock"
    if lock.exists():
        raise SystemExit(f"{lock} exists; the check must not overwrite a lockfile it did not create")
    common = [
        "--manifest-path", str(directory / "Cargo.toml"),
        "--features", features,
        "--target-dir", options.target_dir,
        "-j", options.jobs,
        *patches,
    ]
    shutil.copyfile(ROOT / "Cargo.lock", lock)
    try:
        cargo("check", "--all-targets", "--keep-going", *common)
        if not options.no_test:
            cargo("test", "--lib", "--tests", *common)
    finally:
        lock.unlink(missing_ok=True)


def main() -> int:
    manifest = root_manifest()
    features = crate_features(manifest)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("crates", nargs="*", help=f"forks to check (default: {' '.join(features)})")
    parser.add_argument("--jobs", default="4")
    parser.add_argument("--target-dir", default=str(ROOT / "target" / "vendor-gpui"))
    parser.add_argument("--no-test", action="store_true", help="only build all targets")
    options = parser.parse_args()
    unknown = sorted(set(options.crates) - set(features))
    if unknown:
        parser.error(f"unknown forks: {' '.join(unknown)}")
    patches = patch_configs(manifest)
    failed = []
    for crate in options.crates or features:
        try:
            check(crate, features[crate], options, patches)
        except subprocess.CalledProcessError:
            failed.append(crate)
    if failed:
        print("standalone GPUI fork checks failed:", " ".join(failed), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
