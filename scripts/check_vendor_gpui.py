#!/usr/bin/env python3
"""Build every target of the vendored GPUI fork standalone and run its tests.

`vendor/gpui` is excluded from the root workspace (docs/VENDORING.md), so
`cargo clippy/test --workspace` never compiles its unit tests or examples, and
Cargo resolves dev-dependencies only for the root package of a build. This
check builds the fork as its own root with the root `[patch.crates-io]`
overrides passed through `--config`, so GPUI and its platform crate resolve to
the same local forks as the application.

The run is seeded with the root `Cargo.lock`: shared packages keep the
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
CRATE = ROOT / "vendor" / "gpui"
FEATURES = "stacker,test-support"


def patch_configs() -> list[str]:
    manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    args = []
    for name, spec in manifest["patch"]["crates-io"].items():
        path = (ROOT / spec["path"]).resolve().as_posix()
        args += ["--config", f"patch.crates-io.{name}.path='{path}'"]
    return args


def cargo(*args: str) -> None:
    command = ["cargo", *args]
    print("+", " ".join(command), flush=True)
    subprocess.run(command, cwd=ROOT, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", default="4")
    parser.add_argument("--target-dir", default=str(ROOT / "target" / "vendor-gpui"))
    parser.add_argument("--no-test", action="store_true", help="only build all targets")
    options = parser.parse_args()
    lock = CRATE / "Cargo.lock"
    if lock.exists():
        print(f"{lock} exists; the check must not overwrite a lockfile it did not create")
        return 2
    common = [
        "--manifest-path", str(CRATE / "Cargo.toml"),
        "--features", FEATURES,
        "--target-dir", options.target_dir,
        "-j", options.jobs,
        *patch_configs(),
    ]
    shutil.copyfile(ROOT / "Cargo.lock", lock)
    try:
        cargo("check", "--all-targets", "--keep-going", *common)
        if not options.no_test:
            cargo("test", "--lib", "--tests", *common)
    except subprocess.CalledProcessError as error:
        return error.returncode or 1
    finally:
        lock.unlink(missing_ok=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
