"""Проверка сопоставимости GPU до любых RGB verdicts (ошибка инфраструктуры: exit 2)."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct

SCHEMA = 1
DEVICE_FIELDS = (
    "luid_high_part", "luid_low_part", "vendor_id", "device_id", "subsystem_id",
    "revision", "adapter_flags", "driver_version", "feature_level", "creation_flags",
    "factory_debug", "backend",
)
FRAME_FIELDS = (
    "target_format", "sample_count", "sample_quality", "gamma_ratios",
    "grayscale_enhanced_contrast",
)
CAPTURE_FIELDS = (
    "width", "height", "scale_factor", "path", "stage", "text_mode", "color_space",
    "animation_time_ms", "animation_frames",
)


class InfrastructureError(ValueError):
    """Отсутствующая аттестация никогда не превращается в потерю пикселей."""


def require(condition, message):
    if not condition:
        raise InfrastructureError(f"INCOMPARABLE_GPU_CONTEXT: {message}")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def sidecar(report):
    return Path(str(report) + ".gpu.json")


def fields(value, names):
    require(isinstance(value, dict), "expected an object")
    require(all(k in value for k in names), f"missing fields: {set(names) - value.keys()}")
    return {key: value[key] for key in names}


def target_fields(context):
    target = fields(context, FRAME_FIELDS)
    require(len(target["gamma_ratios"]) == 4, "invalid gamma vector")
    # Rust serde и JS печатают один f32 разным числом десятичных знаков.
    # Сравниваются точные биты GPU constants; RGB сюда не попадает.
    def bits(value):
        require(isinstance(value, (int, float)) and math.isfinite(value), "invalid GPU constant")
        return struct.pack("<f", value).hex()
    target["gamma_ratios"] = [bits(value) for value in target["gamma_ratios"]]
    target["grayscale_enhanced_contrast"] = bits(target["grayscale_enhanced_contrast"])
    return target


def load(report, exe=None, pair_list=None):
    """Проверить все epochs и происхождение; вернуть единственный context bucket."""
    report = Path(report)
    try:
        value = json.loads(sidecar(report).read_text(encoding="utf-8"))
        require(value.get("schema") == SCHEMA and value.get("complete") is True,
                f"{report}: missing/old/incomplete manifest")
        require(value["report_sha256"] == sha(report), f"{report}: changed report")
        require(bool(value["boot_session"]), "missing boot session")
        if exe:
            require(value["exe"]["sha256"] == sha(exe), "resume executable SHA differs")
        if pair_list:
            require(value["list_sha256"] == sha(pair_list), "resume pair-list SHA differs")
        if value.get("kind") == "aggregate":
            require(bool(value["children"]), "no worker manifests")
            contexts = [load(child) for child in value["children"]]
            rows = {}
            for child in value["children"]:
                metadata = json.loads(sidecar(child).read_text(encoding="utf-8"))
                require(metadata["exe"]["sha256"] == value["exe"]["sha256"], "worker executable differs")
                for line in Path(child).read_text(encoding="utf-8").splitlines():
                    parts = line.split("|")
                    if len(parts) >= 3:
                        rows["|".join(parts[:2])] = "|".join(parts[2:])
            merged = {}
            for line in report.read_text(encoding="utf-8").splitlines():
                parts = line.split("|")
                if len(parts) >= 3:
                    merged["|".join(parts[:2])] = "|".join(parts[2:])
            require(rows == merged, "merged verdicts differ from attested workers")
            for context in contexts[1:]:
                equal(contexts[0], context)
            require(contexts[0]["boot_session"] == value["boot_session"], "worker boot differs")
            return contexts[0]
        fields(value, ("run_id", "pid", "process_start_ms", "exe", "source_revision", "list_sha256"))
        require(value["pid"] > 0 and value["process_start_ms"] > 0 and value["run_id"], "unknown process")
        require(value["exe"]["sha256"] and value["exe"]["path"] and value["exe"]["basename"], "unknown exe")
        request = value["requested"]
        require(request["strict"] is True and request["fallback_reason"] is None, "non-strict GPU request")
        resolved = fields(request["resolved"], ("high_part", "low_part"))
        capture = fields(value["capture"], CAPTURE_FIELDS)
        require(capture["path"] == "gpu-readback", "unattested capture path")
        require(capture["animation_time_ms"] is not None, "animation clock is not pinned")
        require(capture["scale_factor"] > 0, "invalid scale")
        require(value["frames"], "no completed captures")
        bucket = None
        hashes = set()
        previous_generation = 0
        used_generations = set()
        for index, frame in enumerate(value["frames"], 1):
            require(frame["sequence"] == index, "missing/reordered capture")
            ctx = frame["context"]
            device = fields(ctx["device"], DEVICE_FIELDS)
            require(ctx["device"]["queried_from_device"] is True, "identity was not queried from device")
            generation = ctx["device"]["generation"]
            require(generation > 0 and generation >= previous_generation, "unknown/reordered generation")
            previous_generation = generation
            used_generations.add(str(generation))
            audit = value["generations"][str(generation)]
            equal(audit["device"], ctx["device"])
            require(audit["driver_dlls"], "missing per-generation UMD audit")
            require(ctx["shader_executables"] and all(len(h) == 64 for h in ctx["shader_executables"]), "missing shader audit")
            equal(audit["shader_executables"], ctx["shader_executables"])
            require(device["driver_version"] not in (None, "", "unknown"), "unknown UMD version")
            require(device["adapter_flags"] & 2 == 0, "software fallback")
            require(device["luid_high_part"] == resolved["high_part"] and
                    device["luid_low_part"] == resolved["low_part"], "requested/actual LUID mismatch")
            require(frame["width"] == capture["width"] * capture["scale_factor"] and
                    frame["height"] == capture["height"] * capture["scale_factor"], "capture geometry differs")
            current = {"device": device, "target": target_fields(ctx),
                       "width": frame["width"], "height": frame["height"]}
            if bucket is not None:
                equal(bucket, current)
            bucket = current
            require(len(frame["bgra_sha256"]) == 64, "missing capture SHA")
            hashes.add(frame["bgra_sha256"])
        for png in value["pngs"]:
            require(png["bgra_sha256"] in hashes, "PNG has no originating GPU readback")
            require(sha(png["path"]) == png["sha256"], "PNG SHA differs")
        drivers = value["driver_dlls"]
        require(drivers and all(len(d["sha256"]) == 64 and d["path"] and d["version"] for d in drivers), "unknown loaded UMD")
        require(used_generations == value["generations"].keys(), "unused/missing generation audit")
        for audit in value["generations"].values():
            equal(drivers, audit["driver_dlls"])
        policy = fields(value["shader_policy"], ("debug_assertions", "backend"))
        require(isinstance(policy["debug_assertions"], bool) and policy["backend"], "unknown shader policy")
        bucket.update(boot_session=value["boot_session"], capture=capture,
                      driver_dlls=drivers, shader_policy=policy)
        return bucket
    except (KeyError, TypeError, OSError, OverflowError, UnicodeError, struct.error, json.JSONDecodeError) as error:
        raise InfrastructureError(f"INCOMPARABLE_GPU_CONTEXT: {report}: {error}") from error


def equal(base, candidate):
    differences = []

    def walk(a, b, path):
        if isinstance(a, dict) and isinstance(b, dict):
            for key in sorted(a.keys() | b.keys()):
                walk(a.get(key), b.get(key), f"{path}.{key}")
        elif a != b:
            differences.append(f"{path}: base={a!r} candidate={b!r}")

    walk(base, candidate, "context")
    require(not differences, "\n" + "\n".join(differences))


def compare(base, candidate):
    equal(load(base), load(candidate))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base")
    parser.add_argument("candidate", nargs="?")
    args = parser.parse_args()
    try:
        compare(args.base, args.candidate) if args.candidate else load(args.base)
    except InfrastructureError as error:
        parser.exit(2, f"{error}\n")
