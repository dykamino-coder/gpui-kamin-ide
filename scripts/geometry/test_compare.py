"""Geometry matching must preserve DOM identity and fragment cardinality."""

import copy
import unittest

from compare import compare, index


def snapshot():
    rect = dict(x=0, y=0, width=10, height=20)
    return {"schema_version": 1,
            "viewport": dict(width=300, height=600, device_scale_factor=1),
            "elements": [dict(path=[1, 0], tag="div", id="a", **{"class": ""},
                              border_box=rect, client_rects=[rect], padding_boxes=[rect],
                              content_boxes=[rect], font_size=16, line_height="normal")]}


class Comparison(unittest.TestCase):
    def test_fragment_count_is_exact_even_with_large_tolerance(self):
        a = snapshot()
        b = copy.deepcopy(a)
        b["elements"][0]["client_rects"].append(dict(x=30, y=0, width=10, height=20))
        result = compare(a, b, absolute=100)
        self.assertEqual(result["mismatched_counts_by_property"], {"client_rects": 1})
        self.assertEqual(result["elements"][0]["differences"][0]["property"],
                         "client_rects.length")

    def test_paths_are_exact_and_duplicate_paths_are_invalid(self):
        a = snapshot()
        b = copy.deepcopy(a)
        b["elements"][0]["path"] = [1, 1]
        totals = compare(a, b)["totals"]
        self.assertEqual(totals["matched_elements"], 0)
        self.assertEqual(totals["missing_in_engine"], 1)
        self.assertEqual(totals["missing_in_chromium"], 1)
        a["elements"].append(a["elements"][0])
        with self.assertRaises(ValueError):
            index(a)

    def test_boxes_use_tolerance_but_count_properties_once_per_element(self):
        a = snapshot()
        b = copy.deepcopy(a)
        b["elements"][0]["border_box"]["x"] += 0.4
        self.assertEqual(compare(a, b)["totals"]["mismatched_elements"], 0)
        b["elements"][0]["border_box"]["width"] += 5
        result = compare(a, b, absolute=0.1)
        self.assertEqual(result["mismatched_counts_by_property"]["border_box"], 1)
        self.assertEqual(result["worst_elements"][0]["max_abs_delta"], 5)

    def test_viewport_or_nonfinite_geometry_is_rejected(self):
        a = snapshot()
        b = copy.deepcopy(a)
        b["viewport"]["width"] = 400
        with self.assertRaises(ValueError):
            compare(a, b)
        b = copy.deepcopy(a)
        b["elements"][0]["font_size"] = float("nan")
        with self.assertRaises(ValueError):
            compare(a, b)


if __name__ == "__main__":
    unittest.main()
