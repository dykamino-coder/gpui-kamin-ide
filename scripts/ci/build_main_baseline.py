"""Build the exact Windows dev gate and both html-test feature configurations."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


REPOSITORY = "dykamino-coder/gpui-kamin-ide"
CI_ROOT = Path("C:/Users/Public/Documents/KaminIDE/main-baseline")
SHADERS = ("shaders.hlsl", "color_text_raster.hlsl", "alpha_correction.hlsl")


def ci_checkout(source, sha, root=CI_ROOT):
    """Keep debug GPUI's compiled absolute shader path reproducible on consumers."""
    dest = root / sha
    subprocess.run(["git", "clone", "--config", "core.longpaths=true",
                    "--no-local", "--depth", "1", "--no-checkout",
                    str(source), str(dest)], check=True)
    subprocess.run(["git", "-C", str(dest), "checkout", "--detach", sha], check=True)
    if subprocess.check_output(["git", "-C", str(dest), "rev-parse", "HEAD"],
                               text=True).strip() != sha:
        raise ValueError("CI source copy SHA mismatch")
    return dest


def output(*args):
    return subprocess.check_output(args, text=True).strip()


def digest(path):
    with path.open("rb") as stream:
        checksum = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(chunk)
        return checksum.hexdigest()


def cargo_binary(args, kind, name, log):
    started = time.monotonic()
    command = ["cargo", *args, "--message-format=json"]
    with log.open("w", encoding="utf-8") as stream:
        subprocess.run(command, stdout=stream, check=True)
    binaries = set()
    for line in log.read_text(encoding="utf-8").splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        target = event.get("target", {})
        if (event.get("reason") == "compiler-artifact"
                and target.get("name") == name
                and kind in target.get("kind", [])
                and event.get("executable")):
            binaries.add(Path(event["executable"]))
    if len(binaries) != 1:
        raise RuntimeError(f"Expected one {name} executable, got {binaries}")
    return binaries.pop(), round(time.monotonic() - started, 3), command


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--wptrun", type=Path, help="Already built by build_shared.sh")
    parser.add_argument("--ci-layout", action="store_true", help="Use the per-SHA runtime shader path")
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Windows x64 MSVC is required")
    # CI's quality job overrides these; inheriting them would change the gate build.
    for key in os.environ:
        if key.startswith(("CARGO_PROFILE_", "CARGO_TARGET_")) or key in {
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS",
            "RUSTC_WRAPPER", "CARGO_INCREMENTAL", "CARGO_BUILD_TARGET",
        }:
            parser.error(f"Unset {key} before building the baseline")
    os.environ["CARGO_BUILD_JOBS"] = "4"
    sha = output("git", "rev-parse", "HEAD")
    if os.environ.get("GITHUB_SHA", sha) != sha:
        parser.error("Checkout does not match GITHUB_SHA")
    if output("git", "status", "--porcelain", "--untracked-files=no"):
        parser.error("Tracked sources must be clean")
    rustc = output("rustc", "-vV")
    if "host: x86_64-pc-windows-msvc" not in rustc:
        parser.error("Expected x86_64-pc-windows-msvc host")
    dest = args.output.resolve()
    prebuilt = args.wptrun.resolve() if args.wptrun else None
    if args.ci_layout:
        if prebuilt:
            parser.error("--ci-layout requires compiling wptrun in the CI source layout")
        started = time.monotonic()
        os.chdir(ci_checkout(Path.cwd(), sha))
        layout_seconds = round(time.monotonic() - started, 3)
    else:
        layout_seconds = None
    dest.mkdir(parents=True, exist_ok=False)
    entries, builds = [], []

    def save(source, filename, seconds, command, features, profile):
        target = dest / filename
        shutil.copy2(source, target)
        entries.append({"name": filename, "sha256": digest(target),
                        "size": target.stat().st_size, "features": features,
                        "profile": profile})
        builds.append({"command": command, "seconds": seconds})

    if args.wptrun:
        binary, seconds = prebuilt, None
        command = ["build_shared.sh", "<worktree>", "<output>"]
    else:
        # Same command/profile/features and implicit host target as build_shared.sh.
        # Measured gate host ~/.cargo/config.toml sets dev.debug=line-tables-only.
        # Reproduce that effective profile on a clean CI runner, without --config.
        os.environ["CARGO_PROFILE_DEV_DEBUG"] = "line-tables-only"
        os.environ["CARGO_TARGET_DIR"] = str(Path("target/fast").resolve())
        binary, seconds, command = cargo_binary(
            ["build", "--example", "wptrun", "--features", "kamin-html/wpt-harness"],
            "example", "wptrun", dest / "wptrun-build.jsonl")
        del os.environ["CARGO_TARGET_DIR"]
        del os.environ["CARGO_PROFILE_DEV_DEBUG"]
    save(binary, "main.exe", seconds, command, ["wpt-harness"], "dev")
    for features, filename in [([], "html-tests.exe"),
                               (["wpt-harness"], "html-tests-wpt.exe")]:
        command = ["html-test", "--locked", "--no-run"]
        if features:
            command += ["--features", ",".join(features)]
        binary, seconds, command = cargo_binary(
            command, "lib", "kamin_html", dest / f"{filename}.jsonl")
        save(binary, filename, seconds, command, features, "agent")
    if output("git", "rev-parse", "HEAD") != sha or output(
            "git", "status", "--porcelain", "--untracked-files=no"):
        parser.error("Tracked sources changed during the build")
    shader_root = Path("vendor/gpui-windows/src").resolve()
    resources = []
    for name in SHADERS:
        shutil.copy2(shader_root / name, dest / name)
        resources.append({"name": name, "sha256": digest(dest / name),
                          "size": (dest / name).stat().st_size})
    manifest = {
        "schema": 2, "repository": REPOSITORY, "commit": sha,
        "tree": output("git", "rev-parse", "HEAD^{tree}"),
        "target": "x86_64-pc-windows-msvc", "rustc": rustc,
        "gate_debug": "line-tables-only",
        "shader_root": shader_root.as_posix(), "resources": resources,
        "layout_seconds": layout_seconds,
        "files": entries, "builds": builds,
        "inputs": {name: digest(Path(name)) for name in (
            "Cargo.toml", "Cargo.lock", ".cargo/config.toml", "rust-toolchain.toml")},
    }
    (dest / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"commit": sha, "files": entries, "builds": builds}, indent=2))


if __name__ == "__main__":
    main()
