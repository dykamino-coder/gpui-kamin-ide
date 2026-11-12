"""Compare geometry snapshots using original DOM childNodes index paths."""

import argparse
import collections
import json
import math
from pathlib import Path


RECTS = ("border_box",)
FRAGMENTS = ("client_rects", "padding_boxes", "content_boxes")
AXES = ("x", "y", "width", "height")


def index(snapshot):
    if snapshot.get("schema_version") != 1:
        raise ValueError("unsupported geometry schema")
    result = {}
    for element in snapshot["elements"]:
        path = tuple(element["path"])
        if any(type(part) is not int or part < 0 for part in path):
            raise ValueError("invalid DOM index path")
        if path in result:
            raise ValueError(f"duplicate DOM index path: {path}")
        result[path] = element
    return result


def compare(engine, chromium, absolute=0.5, relative=0.0, worst=20):
    if engine["viewport"] != chromium["viewport"]:
        raise ValueError("viewport/device scale factor mismatch")
    left, right = index(engine), index(chromium)
    counts = collections.Counter()
    elements = []
    for path in sorted(left.keys() | right.keys()):
        a, b = left.get(path), right.get(path)
        differences = []

        def difference(prop, av, bv, exact=False):
            numeric = not exact and type(av) in (int, float) and type(bv) in (int, float)
            if numeric:
                if not math.isfinite(av) or not math.isfinite(bv):
                    raise ValueError(f"non-finite geometry at {path}: {prop}")
                delta = av - bv
                tolerance = absolute + relative * max(abs(av), abs(bv))
                if abs(delta) <= tolerance:
                    return
            elif av == bv:
                return
            else:
                delta = None
                tolerance = None
            differences.append({"property": prop, "engine": av, "chromium": bv,
                                "delta": delta, "tolerance": tolerance})

        if a is None or b is None:
            difference("element_presence", a is not None, b is not None)
        else:
            for prop in ("tag", "id", "class", "font_size", "line_height"):
                difference(prop, a[prop], b[prop])
            for prop in RECTS:
                if a[prop] is None or b[prop] is None:
                    difference(prop, a[prop], b[prop])
                else:
                    for axis in AXES:
                        difference(f"{prop}.{axis}", a[prop][axis], b[prop][axis])
            for prop in FRAGMENTS:
                difference(f"{prop}.length", len(a[prop]), len(b[prop]), exact=True)
                for i, (ar, br) in enumerate(zip(a[prop], b[prop])):
                    for axis in AXES:
                        difference(f"{prop}[{i}].{axis}", ar[axis], br[axis])
        if differences:
            # Counts are elements per property, not numbers of bad coordinates.
            properties = {d["property"].split(".")[0].split("[")[0] for d in differences}
            counts.update(properties)
            elements.append({"path": list(path), "tag": (a or b)["tag"],
                             "id": (a or b)["id"], "differences": differences,
                             "max_abs_delta": max((abs(d["delta"]) for d in differences
                                                   if d["delta"] is not None), default=0)})
    ranked = sorted(elements, key=lambda e: (-e["max_abs_delta"], e["path"]))
    return {"schema_version": 1, "viewport": engine["viewport"],
            "tolerances": {"absolute": absolute, "relative": relative},
            "totals": {"engine_elements": len(left), "chromium_elements": len(right),
                       "matched_elements": len(left.keys() & right.keys()),
                       "mismatched_elements": len(elements),
                       "missing_in_engine": len(right.keys() - left.keys()),
                       "missing_in_chromium": len(left.keys() - right.keys())},
            "mismatched_counts_by_property": dict(sorted(counts.items())),
            "worst_elements": ranked[:worst], "elements": elements}


def nonnegative(value):
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise argparse.ArgumentTypeError("expected a finite nonnegative number")
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("engine", type=Path)
    parser.add_argument("chromium", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--absolute", type=nonnegative, default=0.5)
    parser.add_argument("--relative", type=nonnegative, default=0.0)
    parser.add_argument("--worst", type=int, default=20)
    args = parser.parse_args()
    if args.worst < 0:
        parser.error("--worst must be nonnegative")
    result = compare(json.loads(args.engine.read_text(encoding="utf-8")),
                     json.loads(args.chromium.read_text(encoding="utf-8")),
                     args.absolute, args.relative, args.worst)
    args.output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps({key: value for key, value in result.items() if key not in
                      ("elements", "worst_elements")}, sort_keys=True))


if __name__ == "__main__":
    main()
