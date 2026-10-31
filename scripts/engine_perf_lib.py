"""Проверка полноты наблюдений и масштабирования без порогов wall-clock."""

import math
import csv

PHASES = ("host_resources", "parse", "style", "document_other", "build_elements",
          "layout_request", "layout", "prepaint", "paint", "raster_gpu_readback", "pipeline_other")
COUNTERS = ("text_position_queries", "text_position_steps", "text_position_build_bytes",
            "multicol_probes", "multicol_child_layouts", "multicol_trial_restores",
            "measure_cache_hits", "measure_cache_misses", "layout_cache_hits", "layout_cache_misses",
            "paint_cache_hits", "paint_cache_misses", "resource_cache_hits", "resource_cache_misses",
            "rebuilt_nodes", "full_tree_walks", "tree_nodes_visited", "subtree_queries",
            "subtree_nodes_visited", "style_match_attempts")


def validate(report, runs):
    if report["schema_version"] != 3 or report["work_counters_enabled"] is not True:
        raise ValueError("requires engine_bench schema 3 with work-counters enabled")
    if report["runs_per_mode"] != runs or set(report["modes"]) != {"cold", "warm"}:
        raise ValueError("missing cold/warm samples")
    for mode, data in report["modes"].items():
        samples = data["work_counters"]["samples"]
        if len(samples) != runs or len(data["samples_ms"]) != runs:
            raise ValueError("incomplete samples")
        for sample in samples:
            if set(sample) != set(PHASES):
                raise ValueError("incomplete phases")
            for phase, counts in sample.items():
                if mode == "warm" and phase in PHASES[:4]:
                    if counts is not None:
                        raise ValueError("warm parse/style must be absent")
                elif not isinstance(counts, dict) or set(counts) != set(COUNTERS) \
                        or any(type(v) is not int or not 0 <= v < 2**64 for v in counts.values()):
                    raise ValueError("incomplete/non-u64 counters")
        for sample in data["samples_ms"]:
            if set(sample) != set(PHASES) | {"first_frame_total"}:
                raise ValueError("incomplete timing phases")
            for phase, elapsed in sample.items():
                absent = mode == "warm" and phase in PHASES[:4]
                if (elapsed is not None if absent else
                        type(elapsed) not in (int, float) or not math.isfinite(elapsed) or elapsed < 0):
                    raise ValueError("invalid/absent timing sample")
        resources = data["process_resources"]["samples"]
        if len(resources) != runs:
            raise ValueError("incomplete process resource samples")
        for sample in resources:
            if sample["error"] is not None or set(sample["phases"]) != set(PHASES):
                raise ValueError("resource observation failed")
            total = 0
            for phase, usage in sample["phases"].items():
                if mode == "warm" and phase in PHASES[:4]:
                    if usage is not None:
                        raise ValueError("warm resource phase must be absent")
                    continue
                for key in ("process_cpu_100ns", "sampled_peak_rss_bytes",
                            "sampled_peak_private_commit_bytes"):
                    if type(usage[key]) is not int or usage[key] < 0:
                        raise ValueError("invalid process resources")
                total += usage["process_cpu_100ns"]
            if total != sample["process_cpu_100ns"]:
                raise ValueError("phase CPU does not conserve process CPU")
        if not any(s["build_elements"]["rebuilt_nodes"] for s in samples):
            raise ValueError("no observed render construction")


def write_table(path, reports):
    """Одна строка на страницу/режим/кадр/фазу: время, ресурсы и целые счётчики."""
    fields = ["page", "mode", "sample", "phase", "wall_ms", "process_cpu_ms",
              "sampled_peak_rss_bytes", "sampled_peak_private_commit_bytes",
              "end_rss_bytes", "end_private_commit_bytes", "sampled_peak_gpu_local_bytes",
              "sampled_peak_gpu_non_local_bytes", "end_gpu_local_bytes",
              "end_gpu_non_local_bytes", "gpu_error", *COUNTERS]
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields, delimiter="\t")
        writer.writeheader()
        for page, report in reports.items():
            for mode, data in report["modes"].items():
                for index, (counts, resources, times) in enumerate(zip(
                        data["work_counters"]["samples"], data["process_resources"]["samples"],
                        data["samples_ms"]), 1):
                    for phase in PHASES:
                        row = dict(page=page, mode=mode, sample=index, phase=phase,
                                   wall_ms=times[phase])
                        usage = resources["phases"][phase]
                        if usage is not None:
                            gpu = usage["sampled_peak_gpu"] or {}
                            end_gpu = usage["end_gpu"] or {}
                            row.update(process_cpu_ms=usage["process_cpu_100ns"] / 10000,
                                       sampled_peak_rss_bytes=usage["sampled_peak_rss_bytes"],
                                       sampled_peak_private_commit_bytes=usage["sampled_peak_private_commit_bytes"],
                                       end_rss_bytes=usage["end"]["rss_bytes"],
                                       end_private_commit_bytes=usage["end"]["private_commit_bytes"],
                                       sampled_peak_gpu_local_bytes=gpu.get("local_bytes"),
                                       sampled_peak_gpu_non_local_bytes=gpu.get("non_local_bytes"),
                                       end_gpu_local_bytes=end_gpu.get("local_bytes"),
                                       end_gpu_non_local_bytes=end_gpu.get("non_local_bytes"),
                                       gpu_error=resources["gpu_error"])
                        row.update(counts[phase] or {})
                        writer.writerow(row)


def value(report, mode, phases, counter):
    # Максимум целых сумм по кадрам, без смешивания разных кадров.
    return max(sum(sample[p][counter] for p in phases)
               for sample in report["modes"][mode]["work_counters"]["samples"])


def check_bounds(bounds, reports):
    results = []
    for bound in bounds:
        result = {"id": bound["id"], "comparisons": []}
        failures = []
        for mode in bound["modes"]:
            for low, high in zip(bound["steps"], bound["steps"][1:]):
                key = f"{mode}:{low['page']}->{high['page']}"
                if low["page"] not in reports or high["page"] not in reports:
                    result["comparisons"].append({"key": key, "status": "MISSING"})
                    continue
                a = value(reports[low["page"]], mode, bound["phases"], bound["counter"])
                b = value(reports[high["page"]], mode, bound["phases"], bound["counter"])
                # b <= a*(N_high/N_low)*(numerator/denominator) + intercept.
                lhs = b * low["size"] * bound["denominator"]
                rhs = a * high["size"] * bound["numerator"] \
                    + bound["intercept"] * low["size"] * bound["denominator"]
                failed = lhs > rhs
                if failed:
                    failures.append(key)
                result["comparisons"].append({"key": key, "low": a, "high": b,
                                               "lhs": lhs, "rhs": rhs,
                                               "status": "FAIL" if failed else "PASS"})
        expected = bound.get("expected_failures", {})
        keys = {c["key"] for c in result["comparisons"]}
        if any(key not in keys or item.get("package") not in ("P3", "P4", "P6")
               or not item.get("reason") for key, item in expected.items()):
            raise ValueError("expected failure requires an exact edge, reason and owner P3/P4/P6")
        missing = any(c["status"] == "MISSING" for c in result["comparisons"])
        new = set(failures) - set(expected)
        retired = set(expected) - set(failures)
        result["expected_failures"] = expected
        result["status"] = ("MISSING" if missing else "FAIL" if new else "XPASS" if retired
                            else "XFAIL" if failures else "PASS")
        results.append(result)
    return results


if __name__ == "__main__":
    import unittest

    class BoundsTests(unittest.TestCase):
        def test_missing_or_disabled_observation_cannot_pass(self):
            import copy
            counts = {p: dict.fromkeys(COUNTERS, 0) for p in PHASES}
            counts["build_elements"]["rebuilt_nodes"] = 1
            report = dict(schema_version=3, work_counters_enabled=True, runs_per_mode=1, modes={})
            for mode in ("cold", "warm"):
                sample = copy.deepcopy(counts)
                times = dict.fromkeys((*PHASES, "first_frame_total"), 1.0)
                if mode == "warm":
                    for phase in PHASES[:4]:
                        sample[phase] = times[phase] = None
                usage = {p: (None if sample[p] is None else
                         dict(process_cpu_100ns=0, sampled_peak_rss_bytes=1,
                              sampled_peak_private_commit_bytes=1)) for p in PHASES}
                report["modes"][mode] = dict(work_counters=dict(samples=[sample]), samples_ms=[times],
                    process_resources=dict(samples=[dict(phases=usage, error=None, process_cpu_100ns=0)]))
            validate(report, 1)
            report["work_counters_enabled"] = False
            with self.assertRaises(ValueError):
                validate(report, 1)
            report["work_counters_enabled"] = True
            del report["modes"]["cold"]["work_counters"]["samples"][0]["paint"]["text_position_steps"]
            with self.assertRaises(ValueError):
                validate(report, 1)

        def test_expected_failure_does_not_exempt_other_edges_or_missing_data(self):
            def report(n):
                return {"modes": {"warm": {"work_counters": {"samples": [{"paint": {"x": n}}]}}}}
            bound = dict(id="x", modes=["warm"], phases=["paint"], counter="x",
                         numerator=1, denominator=1, intercept=0,
                         steps=[dict(page=p, size=n) for p, n in [("a", 1), ("b", 2), ("c", 4)]],
                         expected_failures={"warm:a->b": {"package": "P3", "reason": "subtree rescan"}})
            reports = {p: report(n) for p, n in [("a", 10), ("b", 30), ("c", 50)]}
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "XFAIL")
            reports["c"] = report(70)
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "FAIL")
            reports["b"] = report(20)
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "FAIL")
            reports["c"] = report(40)
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "XPASS")
            del reports["c"]
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "MISSING")

        def test_large_integer_boundary_is_not_rounded(self):
            n = 2**60
            reports = {p: {"modes": {"warm": {"work_counters": {"samples": [{"paint": {"x": v}}]}}}}
                       for p, v in [("a", n), ("b", 2*n+1)]}
            bound = dict(id="x", modes=["warm"], phases=["paint"], counter="x",
                         numerator=1, denominator=1, intercept=0,
                         steps=[dict(page="a", size=1), dict(page="b", size=2)])
            self.assertEqual(check_bounds([bound], reports)[0]["status"], "FAIL")

    unittest.main()
