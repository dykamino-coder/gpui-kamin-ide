"""Синтетические проверки инфраструктурного gate без окна, драйвера и WPT corpus."""

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from wpt_context import InfrastructureError, compare, load, sha, sidecar
import wpt_runner


def manifest(report, exe, pair_list):
    device = dict(luid_high_part=0, luid_low_part=42, device_name="Hardware GPU",
                  vendor_id=1, device_id=2, subsystem_id=3, revision=4, adapter_flags=0,
                  driver_version="1.2.3.4", feature_level=0xB100, creation_flags=0x22,
                  factory_debug=True, backend="d3d11", generation=1, queried_from_device=True)
    shaders = ["a" * 64]
    context = dict(device=device, target_format=87, sample_count=1, sample_quality=0,
                   gamma_ratios=[1., 0., 0., 0.], grayscale_enhanced_contrast=1., shader_executables=shaders)
    drivers = [dict(name="driver.dll", path="C:/synthetic/driver.dll", sha256="b" * 64, version="1.2.3.4")]
    return dict(schema=1, complete=True, run_id="123-456", pid=123, process_start_ms=456,
                exe=dict(path=str(exe), basename=exe.name, sha256=sha(exe)),
                source_revision="synthetic", boot_session="boot-1", list_sha256=sha(pair_list),
                requested=dict(selector="luid:00000000:0000002A", strict=True, fallback_reason=None,
                               resolved=dict(high_part=0, low_part=42)),
                capture=dict(width=800, height=600, scale_factor=1., path="gpu-readback", stage="before-present",
                             text_mode="grayscale", color_space="raw-unorm-no-colorspace-transform",
                             animation_time_ms=125, animation_frames=None),
                frames=[dict(sequence=1, context=context, width=800, height=600, bgra_sha256="c" * 64)],
                generations={"1": dict(device=copy.deepcopy(device), driver_dlls=drivers, shader_executables=shaders)},
                pngs=[], driver_dlls=drivers, shader_policy=dict(debug_assertions=True, backend="d3dcompiler"),
                report_sha256=sha(report))


class ContextTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="wpt-context-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.exe, self.pairs = self.root / "main.exe", self.root / "pairs.txt"
        self.exe.write_bytes(b"synthetic executable")
        self.pairs.write_text("test|ref\n", encoding="utf-8")
        self.base, self.new = self.root / "base.rep", self.root / "new.rep"
        for report in (self.base, self.new):
            report.write_text("test|ref|0.00\n", encoding="utf-8")
            self.write(report, manifest(report, self.exe, self.pairs))

    @staticmethod
    def write(report, value):
        sidecar(report).write_text(json.dumps(value), encoding="utf-8")

    def change(self, mutate):
        value = json.loads(sidecar(self.new).read_text(encoding="utf-8"))
        mutate(value)
        self.write(self.new, value)

    def test_matching_context_allows_different_executable_identity(self):
        self.change(lambda v: v["exe"].update(sha256="d" * 64, basename="flip.exe"))
        compare(self.base, self.new)

    def test_missing_old_incomplete_and_corrupt_manifest_fail_closed(self):
        original = sidecar(self.new).read_bytes()
        mutations = [lambda v: v.update(schema=0), lambda v: v.update(complete=False),
                     lambda v: v.update(boot_session=None), lambda v: v.update(report_sha256="d" * 64),
                     lambda v: v.update(frames=[]), lambda v: v.pop("generations")]
        for mutate in mutations:
            with self.subTest(mutate=mutate):
                sidecar(self.new).write_bytes(original)
                self.change(mutate)
                with self.assertRaisesRegex(InfrastructureError, "INCOMPARABLE_GPU_CONTEXT"):
                    compare(self.base, self.new)
        sidecar(self.new).write_bytes(b"\xff")
        with self.assertRaises(InfrastructureError):
            compare(self.base, self.new)
        sidecar(self.new).unlink()
        with self.assertRaises(InfrastructureError):
            compare(self.base, self.new)

    def test_every_material_device_field_is_a_hard_boundary(self):
        original = sidecar(self.new).read_bytes()
        values = dict(luid_low_part=43, driver_version="2.0.0.0", vendor_id=4, device_id=5,
                      feature_level=0xB000, creation_flags=0x20, factory_debug=False, backend="warp")
        for key, replacement in values.items():
            with self.subTest(key=key):
                sidecar(self.new).write_bytes(original)
                def mutate(value):
                    value["frames"][0]["context"]["device"][key] = replacement
                    value["generations"]["1"]["device"][key] = replacement
                    if key == "luid_low_part":
                        value["requested"]["resolved"]["low_part"] = replacement
                self.change(mutate)
                with self.assertRaises(InfrastructureError):
                    compare(self.base, self.new)

    def test_target_and_capture_modes_are_hard_boundaries(self):
        original = sidecar(self.new).read_bytes()
        for area, key, value in [("target", "sample_count", 4), ("target", "sample_quality", 1),
                                 ("target", "target_format", 91), ("target", "gamma_ratios", [2., 0., 0., 0.]),
                                 ("capture", "scale_factor", 2.),
                                 ("capture", "stage", "after-present"), ("capture", "text_mode", "subpixel"),
                                 ("capture", "animation_time_ms", 126), ("capture", "color_space", "srgb")]:
            sidecar(self.new).write_bytes(original)
            self.change(lambda v: (v["frames"][0]["context"] if area == "target" else v["capture"]).update({key: value}))
            with self.assertRaises(InfrastructureError):
                compare(self.base, self.new)

    def test_missing_driver_and_non_strict_requests_are_infrastructure_errors(self):
        original = sidecar(self.new).read_bytes()
        for mutate in [lambda v: v.update(driver_dlls=[]),
                       lambda v: v["driver_dlls"][0].update(version=""),
                       lambda v: v["requested"].update(strict=False),
                       lambda v: v["requested"].update(fallback_reason="device creation failed"),
                       lambda v: v["requested"]["resolved"].update(low_part=43),
                       lambda v: v["shader_policy"].update(debug_assertions=False)]:
            sidecar(self.new).write_bytes(original)
            self.change(mutate)
            with self.assertRaises(InfrastructureError):
                compare(self.base, self.new)

    def test_gpu_constants_compare_exact_f32_bits_across_json_encoders(self):
        for report, value in [(self.base, 0.46666667), (self.new, 0.46666666865348816)]:
            data = json.loads(sidecar(report).read_text(encoding="utf-8"))
            data["frames"][0]["context"]["gamma_ratios"][0] = value
            self.write(report, data)
        compare(self.base, self.new)
        self.change(lambda v: v["frames"][0]["context"].update(gamma_ratios=[0.46666669845581055, 0., 0., 0.]))
        with self.assertRaises(InfrastructureError):
            compare(self.base, self.new)

    def test_gate_rejects_context_before_interpreting_pixel_loss(self):
        calls = []
        def run(exe, pair_list, report, selector, session, resume):
            calls.append(report)
            report.write_text("test|ref|" + ("0.00" if len(calls) == 1 else "1.00") + "\n", encoding="utf-8")
            data = manifest(report, exe, pair_list)
            if len(calls) == 2:
                data["capture"]["text_mode"] = "subpixel"
            self.write(report, data)
        with patch.object(wpt_runner, "resolve", return_value="luid:00000000:0000002A"), \
             patch.object(wpt_runner, "boot_session", return_value="boot-1"), patch.object(wpt_runner, "run", side_effect=run):
            with self.assertRaisesRegex(InfrastructureError, "INCOMPARABLE_GPU_CONTEXT"):
                wpt_runner.gate(self.exe, self.exe, self.root / "gate", [self.pairs])
        self.assertEqual(len(calls), 2)

    def test_recovery_preserves_old_frame_and_rejects_adapter_drift(self):
        def recover(value):
            frame = copy.deepcopy(value["frames"][0])
            frame["sequence"] = 2
            frame["context"]["device"]["generation"] = 2
            value["frames"].append(frame)
            value["generations"]["2"] = copy.deepcopy(value["generations"]["1"])
            value["generations"]["2"]["device"]["generation"] = 2
        self.change(recover)
        compare(self.base, self.new)
        self.change(lambda v: v["frames"][1]["context"]["device"].update(luid_low_part=43))
        with self.assertRaises(InfrastructureError):
            compare(self.base, self.new)

    def test_resume_provenance_and_orphan_png_are_rejected(self):
        self.exe.write_bytes(b"changed exe")
        with self.assertRaisesRegex(InfrastructureError, "executable SHA"):
            load(self.new, self.exe, self.pairs)
        self.pairs.write_text("different|pair\n", encoding="utf-8")
        with self.assertRaisesRegex(InfrastructureError, "pair-list SHA"):
            load(self.new, pair_list=self.pairs)
        self.change(lambda v: v["pngs"].append(dict(path="missing.png", sha256="d" * 64, bgra_sha256="e" * 64)))
        with self.assertRaisesRegex(InfrastructureError, "originating GPU readback"):
            load(self.new)

    def test_gate_runs_base_even_when_candidate_is_all_green(self):
        calls = []
        def run(exe, pair_list, report, selector, session, resume):
            calls.append((exe, selector, session))
            report.write_text("test|ref|0.00\n", encoding="utf-8")
            self.write(report, manifest(report, exe, pair_list))
        with patch.object(wpt_runner, "resolve", return_value="luid:00000000:0000002A") as resolve, \
             patch.object(wpt_runner, "boot_session", return_value="boot-1"), patch.object(wpt_runner, "run", side_effect=run):
            self.assertEqual(wpt_runner.gate(self.exe, self.exe, self.root / "gate", [self.pairs]), 0)
        self.assertEqual(len(calls), 2)
        self.assertEqual(calls[0], calls[1])
        resolve.assert_called_once()

    def test_unknown_identity_and_fallback_fail_even_if_both_reports_match(self):
        for key, replacement in [("adapter_flags", 2), ("queried_from_device", False),
                                 ("driver_version", "unknown")]:
            with self.subTest(key=key):
                value = manifest(self.new, self.exe, self.pairs)
                value["frames"][0]["context"]["device"][key] = replacement
                value["generations"]["1"]["device"][key] = replacement
                for report in (self.base, self.new):
                    self.write(report, value)
                with self.assertRaises(InfrastructureError):
                    compare(self.base, self.new)

    def test_merged_report_cannot_hide_changed_workers_or_verdicts(self):
        report = self.root / "merged.rep"
        report.write_bytes(self.base.read_bytes())
        value = dict(schema=1, complete=True, kind="aggregate", exe=dict(sha256=sha(self.exe)),
                     list_sha256=sha(self.pairs), boot_session="boot-1", report_sha256=sha(report),
                     children=[str(self.base), str(self.new)])
        self.write(report, value)
        load(report, self.exe, self.pairs)
        self.new.write_text("test|ref|1.00\n", encoding="utf-8")
        self.change(lambda v: v.update(report_sha256=sha(self.new)))
        with self.assertRaisesRegex(InfrastructureError, "merged verdicts"):
            load(report)
        self.new.write_bytes(self.base.read_bytes())
        self.change(lambda v: v.update(report_sha256=sha(self.new)))
        self.change(lambda v: v["exe"].update(sha256="d" * 64))
        with self.assertRaisesRegex(InfrastructureError, "worker executable"):
            load(report)


if __name__ == "__main__":
    unittest.main()
