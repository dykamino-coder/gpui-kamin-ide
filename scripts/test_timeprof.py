"""Tests of scripts/timeprof.py: folding samples by phase, subsystem and focus; the stack unwinder."""

import io
import json
import pathlib
import struct
import sys
import tempfile
import threading
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import timeprof  # noqa: E402
import timeprof_unwind as unwind  # noqa: E402

NAMES = {
    0x10: ("kamin_html::layout::grid::place",), 0x11: ("kamin_html::layout::grid::place",),
    0x20: ("taffy::compute::grid::track_sizing", "taffy::compute::grid::compute"),
    0x30: ("kamin_html::paint::fill",), 0x40: ("engine_bench::run::frame",),
    0x50: ("alloc::vec::Vec::push<u32>", "kamin_style::cascade::apply"),
    0x60: ("ntdll!NtWaitForSingleObject",),
}


class FakeSymbols:
    """Address n resolves to the inline chain NAMES[n]; the sampled pc arrives one byte up."""

    def frames(self, address):
        return NAMES.get(address) or NAMES[address - 1]


def sample_file(lines):
    handle = tempfile.NamedTemporaryFile("w", suffix=".samples", delete=False, encoding="utf-8")
    handle.write("\n".join(["# engine_bench cpu samples v1 interval_us=1000 phases=style,layout,paint", *lines,
                            "# end samples=0 run_us=9000 stopped_us=900 main=1 truncated=0"]) + "\n")
    handle.close()
    return handle.name


LINES = [
    "s 1 1 1 1000 1000 10 20 40",  # layout: place < track_sizing(inlined) < compute < frame
    "s 1 1 1 2000 2000 20 40",
    "s 1 1 2 500 100 60 30 40",  # paint, mostly waiting
    "s 1 2 1 3000 3000 11 40",
    "s 1 2 0 700 700 50 40",  # style through a std container
    "s 7 2 1 1000 400 20 40",  # another thread: counted by its CPU time
    "s 1 3 - 100 100 40",  # between frames
]


class Folding(unittest.TestCase):
    def profile(self, **scope):
        path = sample_file(LINES)
        self.addCleanup(pathlib.Path(path).unlink)
        return timeprof.Profile("x.exe", path, symbols=FakeSymbols(), **scope)

    def test_phases_sum_main_thread_wall_time_and_split_cpu_from_waiting(self):
        report = timeprof.build_report(self.profile(), None, 8, 10, {"cand": {"layout": 6.5}})
        phases = {row["phase"]: row for row in report["phases"]}
        self.assertEqual([row["phase"] for row in report["phases"]], ["layout", "style", "paint", "outside"])
        self.assertEqual(phases["layout"]["cand_us"], 6000)
        self.assertEqual(phases["layout"]["cand_frames"], 2)
        self.assertEqual(phases["layout"]["cand_other_threads_us"], 400)
        self.assertEqual(phases["layout"]["cand_bench_ms"], 6.5)
        self.assertEqual(phases["layout"]["cand_frame_median_us"], 3000)
        self.assertEqual(phases["style"]["cand_frame_median_us"], 700)
        self.assertEqual((phases["paint"]["cand_cpu_us"], phases["paint"]["cand_wait_us"]), (100, 400))
        json.dumps(report)

    def test_the_bench_side_is_its_median_frame(self):
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False, encoding="utf-8") as handle:
            handle.write("launcher line\n" + json.dumps({"modes": {"relayout": {"samples_ms": [
                {"layout": 4.0, "first_frame_total": 9.0}, {"layout": 6.0}, {"layout": 50.0}]}}}))
        self.addCleanup(pathlib.Path(handle.name).unlink)
        self.assertEqual(timeprof.bench_phases(handle.name), {"layout": 6.0})
        self.assertIsNone(timeprof.bench_phases(handle.name + ".missing"))

    def test_sites_and_subsystems_follow_memprof_rules_with_inlined_frames(self):
        report = timeprof.build_report(self.profile(), None, 8, 10)
        subsystems = {(row["phase"], row["subsystem"]): row["cand_us"] for row in report["subsystems"]}
        self.assertEqual(subsystems[("layout", "layout")], 6000)
        self.assertEqual(subsystems[("layout", "other-threads/layout")], 400)
        self.assertEqual(subsystems[("style", "style")], 700)
        sites = {(row["phase"], row["site"]): row["cand_us"] for row in report["sites"]
                 if not row["subsystem"].startswith("other-threads")}
        self.assertEqual(sites[("layout", "kamin_html::layout::grid::place")], 4000)
        self.assertEqual(sites[("layout", "taffy::compute::grid::track_sizing")], 2000)
        self.assertEqual(sites[("style", "kamin_style::cascade::apply")], 700)
        inclusive = {(row["phase"], row["function"]): row["cand_us"] for row in report["functions"]}
        self.assertEqual(inclusive[("layout", "taffy::compute::grid::compute")], 3400)
        waits = {(row["phase"], row["where"]): row["cand_us"] for row in report["leaves"]}
        self.assertEqual(waits[("paint", "wait")], 500)

    def test_phase_and_frames_narrow_the_samples(self):
        report = timeprof.build_report(self.profile(phase="layout", frames=(2, 2)), None, 8, 10)
        self.assertEqual([(row["phase"], row["cand_us"]) for row in report["phases"]], [("layout", 3000)])

    def test_focus_reports_the_callees_of_a_function(self):
        profile = self.profile(focus="grid::compute")
        report = timeprof.build_report(profile, None, 8, 10)
        self.assertEqual(report["focus"]["cand_us"], 3400)
        callees = {row["path"]: row["cand_us"] for row in report["callees"]}
        self.assertEqual(callees["taffy::compute::grid::compute"], 3400)
        self.assertEqual(callees["taffy::compute::grid::compute > taffy::compute::grid::track_sizing"], 3400)
        self.assertEqual(callees["taffy::compute::grid::compute > taffy::compute::grid::track_sizing > "
                                 "kamin_html::layout::grid::place"], 1000)
        out = io.StringIO()
        timeprof.render(report, out)
        self.assertIn("time under grid::compute: cand 3.40", out.getvalue())

    def test_base_side_gives_deltas(self):
        cand, base = self.profile(), self.profile(frames=(1, 1))
        report = timeprof.build_report(cand, base, 8, 10)
        layout = next(row for row in report["phases"] if row["phase"] == "layout")
        self.assertEqual((layout["cand_us"], layout["base_us"], layout["delta_us"]), (6000, 3000, 3000))
        out = io.StringIO()
        timeprof.render(report, out)
        self.assertIn("+3.00", out.getvalue())


class FakeImage:
    """One function at 0x1000..0x1100: push rbx; sub rsp, 0x20 (prolog of 5 bytes), and a leaf at 0x2000."""

    base, end, path = 0x10000, 0x20000, "fake"

    def function(self, rva):
        return (0x1000, 0x1100, 0x5000) if 0x1000 <= rva < 0x1100 else None

    def info(self, _address):
        return 5, 0, 0, ((5, unwind.ALLOC_SMALL, 3, 0x20), (1, unwind.PUSH_NONVOL, 3, 0)), None

    def read(self, _rva, _length):
        return b"\x90" * 40

    def primary(self, function):
        return function[0]


class Unwinding(unittest.TestCase):
    def walker(self):
        walker = unwind.Unwinder()
        walker.bases, walker.images = [FakeImage.base], [FakeImage()]
        return walker

    def stack(self, *words):
        return unwind.Stack(0x7000, b"".join(struct.pack("<Q", word) for word in words))

    def test_a_frame_is_undone_by_its_unwind_steps(self):
        registers = [0] * 16
        registers[unwind.RSP] = 0x7000
        # 0x20 bytes of locals, the saved rbx, the return address into the leaf's caller, then the end.
        stack = self.stack(0, 0, 0, 0, 0xB0B, 0x12050, 0)
        pcs = self.walker().walk(registers, 0x11040, stack)
        self.assertEqual(pcs, [0x11040, 0x12050])

    def test_in_the_prolog_only_the_executed_steps_are_undone(self):
        registers = [0] * 16
        registers[unwind.RSP] = 0x7000
        # After `push rbx` (offset 1), before the allocation: only the push is undone.
        self.assertEqual(self.walker().walk(registers, 0x11001, self.stack(0xB0B, 0x12050, 0)), [0x11001, 0x12050])

    def test_a_leaf_function_returns_through_the_top_of_the_stack(self):
        registers = [0] * 16
        registers[unwind.RSP] = 0x7000
        stack = self.stack(0x11040, 0, 0, 0, 0, 0xB0B, 0x12050, 0)
        self.assertEqual(self.walker().walk(registers, 0x12000, stack), [0x12000, 0x11040, 0x12050])

    def test_the_walk_stops_where_the_copied_stack_ends(self):
        registers = [0] * 16
        registers[unwind.RSP] = 0x7000
        self.assertEqual(self.walker().walk(registers, 0x11040, self.stack(0, 0)), [0x11040])

    def test_an_epilog_is_simulated(self):
        image = FakeImage()
        image.read = lambda _rva, _length: b"\x48\x83\xc4\x20\x5b\xc3" + b"\x90" * 30  # add rsp,20; pop rbx; ret
        walker = unwind.Unwinder()
        walker.bases, walker.images = [image.base], [image]
        registers = [0] * 16
        registers[unwind.RSP] = 0x7000
        pcs = walker.walk(registers, 0x110F0, self.stack(0, 0, 0, 0, 0xB0B, 0x12050, 0))
        self.assertEqual(pcs, [0x110F0, 0x12050])


@unittest.skipUnless(sys.platform == "win32", "samples a Windows thread")
class LiveSampling(unittest.TestCase):
    def test_a_waiting_thread_of_this_process_is_walked_to_its_start(self):
        import os
        import timeprof_sampler as sampler

        stop = threading.Event()
        worker = threading.Thread(target=stop.wait)
        worker.start()
        self.addCleanup(worker.join)
        self.addCleanup(stop.set)
        taken = sampler.Sampler(os.getpid(), 0.001)
        self.addCleanup(taken.close)
        taken.refresh_threads()
        taken.refresh_modules()
        entry = taken.threads[worker.native_id]
        registers, pc, stack, _position, cut = taken.capture(entry, False)
        pcs = taken.unwinder.walk(registers, pc, stack)
        modules = [pathlib.Path(taken.unwinder.image(pc).path).name.lower() for pc in pcs if taken.unwinder.image(pc)]
        self.assertFalse(cut)
        self.assertGreater(len(pcs), 5)
        self.assertEqual(modules[0], "ntdll.dll")  # the wait itself
        self.assertEqual(modules[-1], "ntdll.dll")  # RtlUserThreadStart
        self.assertTrue(any(name.startswith("python3") for name in modules))


if __name__ == "__main__":
    unittest.main()
