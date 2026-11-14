import io
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import bench_build  # noqa: E402
import memprof  # noqa: E402


def build(features, **changes):
    record = {"features": features, "profile": "release", "opt_level": "3", "debug_assertions": False,
              "profile_overrides": []}
    record.update(changes)
    return record


COUNTERS = ["engine-bench", "timing-only", "work-counters"]
AUDIT = ["allocation-audit", "engine-bench", "heap-report", "timing-only", "work-counters"]


class Differences(unittest.TestCase):
    def test_equal_builds_have_no_differences(self):
        self.assertEqual(bench_build.differences(build(AUDIT), build(list(AUDIT))), [])
        self.assertEqual(bench_build.require_same(build(AUDIT), build(AUDIT)), [])

    def test_feature_sets_are_named_by_side(self):
        found = bench_build.differences(build(["timing-only"]), build(COUNTERS))
        self.assertEqual(found, ["features: base only -, candidate only ['engine-bench', 'work-counters']"])

    def test_profile_and_overrides_count(self):
        found = bench_build.differences(build(AUDIT), build(AUDIT, profile="agent", opt_level="0",
                                                              profile_overrides=["lto=off"]))
        self.assertEqual([line.split(":")[0] for line in found], ["profile", "opt_level", "profile_overrides"])

    def test_a_mismatch_stops_unless_allowed(self):
        with self.assertRaises(SystemExit) as stopped:
            bench_build.require_same(build(COUNTERS), build(AUDIT))
        self.assertIn("allocation-audit", str(stopped.exception))
        self.assertIn(bench_build.ALLOW, str(stopped.exception))
        out = io.StringIO()
        self.assertTrue(bench_build.require_same(build(COUNTERS), build(AUDIT), allow=True, out=out))
        self.assertIn("differently built", out.getvalue())

    def test_an_unknown_side_is_reported_not_refused(self):
        out = io.StringIO()
        self.assertEqual(bench_build.require_same(None, build(AUDIT), out=out), [])
        self.assertIn("the base does not report its build", out.getvalue())


class Reports(unittest.TestCase):
    OLD = {"work_counters_enabled": True,
           "modes": {"relayout": {"process_resources": {"samples": [{"rust_allocation_audit_enabled": False}]}}}}

    def test_a_report_with_a_build_record_returns_it(self):
        self.assertEqual(bench_build.of_report({"build": build(AUDIT)}), build(AUDIT))

    def test_an_older_report_shows_counters_and_audit(self):
        self.assertEqual(bench_build.of_report(self.OLD),
                         {"shown_features": {"work-counters": True, "allocation-audit": False}})
        self.assertIsNone(bench_build.of_report({"modes": {}}))

    def test_an_older_report_is_compared_with_a_new_one_on_what_it_shows(self):
        old = bench_build.of_report(self.OLD)
        self.assertEqual(bench_build.differences(old, build(COUNTERS)), [])
        self.assertEqual(bench_build.differences(old, build(AUDIT)), ["allocation-audit: base False, candidate True"])
        self.assertEqual(bench_build.differences(build(["timing-only"]), old),
                         ["work-counters: base False, candidate True"])

    def test_an_exe_without_the_flag_is_unknown(self):
        self.assertIsNone(bench_build.of_exe(pathlib.Path(__file__).with_name("no-such-engine_bench.exe")))
        self.assertIsNone(bench_build.of_exe(sys.executable))


class MemprofDiff(unittest.TestCase):
    def report(self, features):
        return {"schema": memprof.SCHEMA, "mode": "live", "depth": 8, "runs": {"cand": {}}, "builds": {"cand": build(features)},
                "subsystems": [], "sites": [], "stacks": []}

    def test_saved_reports_of_different_builds_are_not_diffed(self):
        import json
        import tempfile
        import types
        with tempfile.TemporaryDirectory() as folder:
            paths = []
            for name, features in (("cand", AUDIT), ("base", COUNTERS)):
                path = pathlib.Path(folder) / f"{name}.json"
                path.write_text(json.dumps(self.report(features)), encoding="utf-8")
                paths.append(str(path))
            args = types.SimpleNamespace(cand=paths[0], base=paths[1], top=5, allow_feature_mismatch=False)
            with self.assertRaises(SystemExit):
                memprof.diff_command(args)


if __name__ == "__main__":
    unittest.main()
