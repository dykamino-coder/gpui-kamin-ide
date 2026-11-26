"""Sparse allocation labels must never turn exact retention sizes into estimates."""

import io
import pathlib
import struct
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof
import memprof_retain as retain


class RetentionSampling(unittest.TestCase):
    def launch_environment(self, sample, *extra):
        with tempfile.TemporaryDirectory() as directory:
            arguments = ["run", "--exe", "test.exe", "--retainers", "--out", directory]
            if sample is not None:
                arguments += ["--sample", str(sample)]
            arguments += extra
            with patch.object(memprof, "preflight"), patch.object(
                memprof.memprof_guard, "run", side_effect=RuntimeError("capture only")
            ) as launch:
                with self.assertRaisesRegex(RuntimeError, "capture only"):
                    memprof.main([*arguments, "--", "test.html", "--mode", "cold", "--runs", "3"])
            return launch.call_args.kwargs["env"]

    def test_default_and_explicit_zero_still_name_every_allocation(self):
        for interval in (None, 0):
            with self.subTest(interval=interval):
                env = self.launch_environment(interval)
                self.assertNotIn("ENGINE_BENCH_ALLOC_STACKS_SAMPLE", env)
                self.assertIn("ENGINE_BENCH_RETAINERS_OUT", env)

    def test_sampling_only_changes_stack_labels_not_graph_opt_in(self):
        env = self.launch_environment(16384)
        self.assertEqual(env["ENGINE_BENCH_ALLOC_STACKS_SAMPLE"], "16384")
        self.assertEqual(env["ENGINE_BENCH_MEMORY_REPORT"], "1")
        self.assertEqual(env["ENGINE_BENCH_ALLOC_STACKS"], "*")
        self.assertIn("ENGINE_BENCH_RETAINERS_OUT", env)

    def test_phase_graph_closes_probe_before_snapshot_without_late_owner_labels(self):
        with patch.dict("os.environ", {"ENGINE_BENCH_RETAINERS_AT": "layout@99"}):
            env = self.launch_environment(16384, "--at", "paint@42")
        self.assertEqual(env["ENGINE_BENCH_ALLOC_STACKS"], "*@0-42")
        self.assertEqual(env["ENGINE_BENCH_ALLOC_STACKS_UNTIL"], "paint")
        self.assertEqual(env["ENGINE_BENCH_RETAINERS_AT"], "paint@42")
        self.assertNotIn("ENGINE_BENCH_MEMORY_REPORT", env)
        self.assertNotIn("ENGINE_BENCH_RETAINERS_AT", self.launch_environment(0))

    def test_phase_graph_rejects_invalid_or_incomplete_windows(self):
        for extra in (("--at", "paint@0"), ("--at", "paint@bad"),
                      ("--at", "paint@42", "--start", "2")):
            with self.subTest(extra=extra), self.assertRaises(SystemExit):
                self.launch_environment(16384, *extra)

    def test_closed_probe_keeps_labels_without_an_owner_walk(self):
        # Reusing the pointer after closing the window must never rename its old block.
        stream = io.StringIO("a 1 paint 100 1000 1234\n# end frame=1 after=paint live=100\n")
        run = object.__new__(memprof.Run)
        run.version, run.sample, run.until = 3, 0, "paint"
        run.symbols = types.SimpleNamespace(name=lambda address: "test::owner")
        run.gpu, run.estimates, run.lines = {}, [], 0
        run._fold(stream, True, None)
        self.assertEqual(run.held["1000"][1], 100)
        self.assertEqual(run.measured, {})

    def test_unnamed_child_stays_in_exact_graph_and_dominator_size(self):
        # Root -> named owner (100 bytes) -> unsampled child (200 bytes).
        # The allocation report's weight is deliberately unrelated to graph sizes.
        graph = bytearray(retain.MAGIC + struct.pack("<I", 2) + b"{}")
        graph += struct.pack("<4Q", 2, 4, 2, 0)
        for code, values in (
            ("Q", [0x1000, 0x2000]), ("Q", [100, 200]),
            ("I", [0, 1, 1, 2, 2]), ("I", [1, 0]), ("I", [0, 0]),
            ("I", [2, 0, retain.NONE, 2]), ("Q", [300, 200, 300, 0]),
            ("I", [2, 0, retain.NONE, 2]), ("I", [0, 0, 0, 0]),
        ):
            graph += struct.pack(f"<{len(values)}{code}", *values)
        graph += bytes(4)
        run = types.SimpleNamespace(symbols=None, measured={},
                                    held={"1000": (("test", "owner", None, ()), 100000)})
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "test.retain"
            path.write_bytes(graph)
            text = io.StringIO()
            report = retain.report(path, run, text, threshold_mb=0)
            self.assertEqual(report["bytes"], 300)
            self.assertEqual(report["named_bytes"], 100)
            self.assertEqual(report["unnamed_bytes"], 200)
            self.assertFalse(report["graph_sampled"])
            groups = {row["group"]: row for row in report["groups"]}
            self.assertEqual(groups["owner"]["retained_bytes"], 300)
            self.assertEqual(groups["(no stack)"]["self_bytes"], 200)
            self.assertIn("33.33%", text.getvalue())


if __name__ == "__main__":
    unittest.main()
