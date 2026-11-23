"""Peak replay must preserve lifetimes and distinguish roots from allocation sites."""

import pathlib
import argparse
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof
import memprof_peak as peak

HEADER = "# engine_bench alloc stacks v3 base=140000000 filter=* sample=0\n"


class Symbols:
    def name(self, address):
        return f"kamin_html::test::site{address}"


class PeakTests(unittest.TestCase):
    def report(self, lines, header=HEADER):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "probe.stacks"
            path.write_text(header + "".join(lines), encoding="utf-8")
            return peak.reports("test.exe", path, memprof, symbols=Symbols())

    def test_transient_peak_and_surviving_owners_are_separate(self):
        result = self.report([
            "a 1 style 100 aa 1\n", "a 1 layout 300 bb 2\n", "f 1 layout 300 bb\n",
            "o document/style aa 100\n", "g atlas 9999 99\n", "# dropped 0\n"])
        self.assertEqual(result["checkpoint"]["live_bytes"], 400)
        self.assertEqual(result["checkpoint"]["phase"], "layout")
        self.assertEqual(result["peak"]["owners_summary"]["cand"]["matched_bytes"], 100)
        self.assertEqual(result["end"]["owners_summary"]["cand"]["live_bytes"], 100)
        self.assertNotIn("gpu_owners", result["peak"])
        self.assertEqual(sum(row["cand_bytes"] for row in result["peak"]["sites"]), 400)

    def test_reused_address_does_not_inherit_peak_owner(self):
        result = self.report([
            "a 1 style 500 aa 1\n", "f 1 style 500 aa\n", "a 1 paint 10 aa 2\n",
            "o document/new aa 10\n", "g atlas 1 1\n"])
        self.assertNotIn("owners", result["peak"])
        self.assertEqual(result["peak"]["sites"][0]["cand_bytes"], 500)

    def test_reallocation_invalidates_later_owner(self):
        result = self.report([
            "a 1 style 500 aa 1\n", "r 1 paint 10 aa 2\n", "o document/new aa 10\n"])
        self.assertNotIn("owners", result["peak"])

    def test_walker_allocations_do_not_move_peak(self):
        result = self.report([
            "a 1 style 50 aa 1\n", "o document/style aa 50\n",
            "a 1 outside 10000 bb 2\n", "o document/estimate 0 999\n", "# dropped 7\n"])
        self.assertEqual(result["checkpoint"]["live_bytes"], 50)
        self.assertEqual(result["checkpoint"]["dropped_records"], 7)
        self.assertEqual(result["peak"]["owners_summary"]["cand"]["reported_bytes"], 50)

    def test_sampling_is_explicit(self):
        result = self.report(["a 1 style 100 aa 1\n"], HEADER.replace("sample=0", "sample=100"))
        self.assertTrue(result["checkpoint"]["estimated"])
        self.assertEqual(result["checkpoint"]["live_bytes"], 158)

    def test_zeroed_allocations_contribute_to_peak(self):
        result = self.report(["z 1 layout 900 aa 1\n", "f 1 layout 900 aa\n"])
        self.assertEqual(result["checkpoint"]["live_bytes"], 900)

    def test_phase_filtered_stream_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "filter"):
            self.report(["a 1 style 100 aa 1\n"], HEADER.replace("filter=*", "filter=style"))

    def test_multiple_owner_walks_are_rejected(self):
        with self.assertRaisesRegex(SystemExit, "one owner walk"):
            self.report(["a 1 style 100 aa 1\n", "o document aa 100\n",
                         "g atlas 10 10\n", "o document aa 100\n"])

    def test_end_ignores_teardown_and_a_later_audit_counter(self):
        result = self.report(["a 1 style 100 aa 1\n", "o document aa 100\n",
                              "g atlas 10 10\n", "f 1 outside 100 aa\n",
                              "# end frame=1 live=0\n"])
        self.assertEqual(sum(row["cand_bytes"] for row in result["end"]["sites"]), 100)

    def test_v1_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "old.stacks"
            path.write_text(HEADER.replace("v3", "v1"), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "v2"):
                peak.locate(path)

    def test_cli_exports_saved_stacks(self):
        with tempfile.TemporaryDirectory() as directory:
            stacks = pathlib.Path(directory) / "probe.stacks"
            output = pathlib.Path(directory) / "peak.json"
            stacks.write_text(HEADER + "a 1 style 123 aa 1\n", encoding="utf-8")
            with patch.object(memprof, "Symbols", return_value=Symbols()):
                memprof.main(["peak", "test.exe", str(stacks), "--out", str(output)])
            result = json.loads(output.read_text())
            self.assertEqual(result["schema"], "kamin.memprof.peak.v1")
            self.assertEqual(result["checkpoint"]["live_bytes"], 123)

    def test_prepare_requires_a_whole_run_and_enables_owners(self):
        args = argparse.Namespace(peak=True, frames=None, at=None, start=0, owners=False, out="out")
        self.assertEqual(peak.prepare(args), pathlib.Path("out"))
        self.assertTrue(args.owners)
        args.at = "layout@1"
        with self.assertRaisesRegex(SystemExit, "whole run"):
            peak.prepare(args)


if __name__ == "__main__":
    unittest.main()
