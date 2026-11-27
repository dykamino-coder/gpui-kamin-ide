"""Process-memory trends retain elapsed time and private commit independently of working-set peaks."""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from memprof_ws_trend import Trend


class TrendTests(unittest.TestCase):
    def test_private_growth_is_visible_while_working_set_falls(self):
        trend = Trend()
        trend.observe(0.1, 100, 200)
        trend.observe(1.2, 90, 300)
        trend.observe(1.3, 80, 400)
        report = trend.report()
        self.assertEqual(report["first"]["private_usage"], 200)
        self.assertEqual(report["last"]["private_usage"], 400)
        self.assertEqual([row["working_set"] for row in report["recent"]], [100, 90, 80])
        self.assertEqual(report["last"]["elapsed_s"], 1.3)

    def test_long_watch_keeps_bounded_tail_and_original_start(self):
        trend = Trend()
        for i in range(10000):
            trend.observe(i / 10, i, i * 2)
        report = trend.report()
        self.assertLessEqual(len(report["recent"]), 301)
        self.assertEqual(report["first"]["elapsed_s"], 0)
        self.assertEqual(report["last"]["private_usage"], 19998)
        self.assertGreater(report["recent"][0]["elapsed_s"], 600)

    def test_empty_watch_has_no_observations(self):
        self.assertIsNone(Trend().report())


if __name__ == "__main__":
    unittest.main()
