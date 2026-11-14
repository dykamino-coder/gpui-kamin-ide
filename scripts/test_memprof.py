"""Tests of scripts/memprof.py: allocator frames, sites, subsystems, live folding and sampling weights."""

import io
import json
import math
import pathlib
import sys
import tempfile
import unittest
import unittest.mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof  # noqa: E402

COUNTED = "engine_bench::allocations::counted::impl$0::alloc"
RUNNER = "reblessive::stack::runner::Runner::finish<alloc::vec::Vec<gpui::element::AnyElement,alloc::alloc::Global> >"
BLOCKS = "kamin_html::render::blocks::context::blocks"


class FakeSymbols:
    """Address n resolves to NAMES[n]."""

    def __init__(self, names):
        self.names = names

    def name(self, address):
        return self.names[address]


class AllocatorFrames(unittest.TestCase):
    def test_generic_global_argument_does_not_hide_the_caller(self):
        # inval7 probe7 fold-f3: the old search cut everything up to Runner::finish<..., Global>.
        names = ["engine_bench::allocations::probe::write", COUNTED, "engine_bench::_::__rust_realloc",
                 "__rustc::__rust_alloc",
                 "alloc::raw_vec::finish_grow", "reblessive::stack::Stack::drive_top_task", RUNNER, BLOCKS]
        site, owner, owned, stack = memprof.classify(names)
        self.assertEqual(site, "reblessive::stack::Stack::drive_top_task")
        self.assertEqual(owner, "recursion-stacks")
        self.assertEqual(stack[1], RUNNER)

    def test_only_the_leading_run_of_allocator_frames_is_dropped(self):
        names = [COUNTED, "kamin_html::text::paragraph::model::Paragraph::new",
                 "alloc::rc::Rc::new<kamin_html::Outer>", "kamin_html::paint::x"]
        kept, owned = memprof.strip_allocator(names)
        self.assertEqual(kept, names[1:])
        self.assertIsNone(owned)

    def test_rc_new_names_the_owned_type_and_leaves_the_caller_as_site(self):
        names = [COUNTED, "alloc::alloc::exchange_malloc",
                 "alloc::rc::Rc::new<kamin_style::style::decoration_paint::DecorationPaint>",
                 "kamin_style::style::cascade::inherit::paint::inherit_paint"]
        site, owner, owned, _ = memprof.classify(names)
        self.assertEqual(site, "kamin_style::style::cascade::inherit::paint::inherit_paint")
        self.assertEqual(owner, "style")
        self.assertEqual(owned, "kamin_style::style::decoration_paint::DecorationPaint")

    def test_std_containers_are_skipped_for_the_site_but_kept_in_the_stack(self):
        names = [COUNTED, "alloc::raw_vec::RawVecInner::reserve", "alloc::vec::Vec::push<u32>",
                 "alloc::string::impl$6::clone", "kamin_html::layout::reuse::store::Store::same_tree"]
        site, owner, _, stack = memprof.classify(names)
        self.assertEqual(site, "kamin_html::layout::reuse::store::Store::same_tree")
        self.assertEqual(owner, "retained")
        self.assertEqual(stack, [site])

    def test_subsystem_is_the_first_known_module_from_the_site_up(self):
        arena = [COUNTED, "gpui::arena::Arena::allocate_next_chunk", "gpui::window::with_element_arena",
                 "kamin_html::text::paragraph::x"]
        self.assertEqual(memprof.classify(arena)[1], "elements")
        unknown = [COUNTED, "foo::bar", "kamin_html::layout::table::rows::table_rows"]
        self.assertEqual(memprof.classify(unknown)[1], "layout")
        self.assertEqual(memprof.classify([COUNTED, "foo::bar"])[1], "other")


def probe_file(lines, header="# engine_bench alloc stacks v3 base=0 filter=*@0-2 until=build_elements sample=0"):
    handle = tempfile.NamedTemporaryFile("w", suffix=".stacks", delete=False, encoding="utf-8")
    handle.write("\n".join([header, *lines]) + "\n")
    handle.close()
    return handle.name


NAMES = {1: COUNTED, 2: "kamin_html::layout::a", 3: "kamin_html::paint::b", 4: "kamin_style::c"}


class Folding(unittest.TestCase):
    def run_of(self, lines, live=True, **header):
        path = probe_file(lines, **header)
        self.addCleanup(pathlib.Path(path).unlink)
        return memprof.Run("x.exe", path, live, symbols=FakeSymbols(NAMES))

    def test_live_heap_drops_freed_and_reallocated_blocks(self):
        run = self.run_of([
            "a 1 layout 100 10 1 2",
            "a 1 paint 30 20 1 3",
            "f 2 paint 30 20",
            "r 2 paint 64 30 1 4",  # realloc of a block from before the window: its free is unknown
            "f 2 layout 100 10",
            "a 2 layout 8 10 1 2",  # same address again after its free
            "# end frame=2 after=build_elements live=72",
            "# dropped 0",
        ])
        totals = {key[0]: value for key, value in run.groups.items()}
        self.assertEqual(totals, {"style": [1.0, 64.0], "layout": [1.0, 8.0]})
        self.assertEqual(run.end, "end frame=2 after=build_elements live=72")
        self.assertEqual(run.meta()["audit_live_bytes_at_end"], 72)
        self.assertEqual(run.meta()["bytes"], 72)

    def test_allocated_mode_counts_every_allocation(self):
        run = self.run_of(["a 1 layout 100 10 1 2", "f 1 layout 100 10", "a 1 layout 50 10 1 2"], live=False)
        self.assertEqual(list(run.groups.values()), [[2.0, 150.0]])

    def test_frames_keep_the_blocks_allocated_in_them_and_every_free_applies(self):
        lines = [
            "a 1 layout 100 10 1 2",  # before the frames
            "a 2 paint 30 20 1 3",
            "a 2 style 50 40 1 4",
            "f 3 style 50 40",  # freed after the frames: not live at the end
            "a 3 layout 7 50 1 2",  # after the frames
        ]
        path = probe_file(lines)
        self.addCleanup(pathlib.Path(path).unlink)
        live = memprof.Run("x.exe", path, True, frames=(2, 2), symbols=FakeSymbols(NAMES))
        self.assertEqual({key[0]: value for key, value in live.groups.items()}, {"paint": [1.0, 30.0]})
        allocated = memprof.Run("x.exe", path, False, frames=(2, 2), symbols=FakeSymbols(NAMES))
        self.assertEqual({key[0]: value for key, value in allocated.groups.items()},
                         {"paint": [1.0, 30.0], "style": [1.0, 50.0]})
        report = memprof.build_report(live, None, True, 8, 10)
        self.assertEqual(report["runs"]["cand"]["frames"], [2, 2])
        out = io.StringIO()
        memprof.render(report, out)
        self.assertIn("only blocks allocated in frames 2-2", out.getvalue())

    def test_both_commands_take_frames(self):
        seen = []
        for name in ("run_command", "report_command"):
            original = getattr(memprof, name)
            self.addCleanup(setattr, memprof, name, original)
            setattr(memprof, name, lambda args: seen.append((args.command, args.frames)))
        memprof.main(["run", "--exe", "c.exe", "--out", "o", "--at", "paint@4", "--frames", "2-3", "--", "p.html"])
        memprof.main(["report", "c.exe", "c.stacks", "--frames", "2"])
        self.assertEqual(seen, [("run", "2-3"), ("report", "2")])
        self.assertEqual(memprof.parse_frames("2"), (2, 2))

    def test_sampled_records_are_divided_by_their_probability(self):
        header = "# engine_bench alloc stacks v3 base=0 filter=* until=- sample=1000"
        run = self.run_of(["a 1 layout 100 10 1 2", "a 1 layout 5000 20 1 2"], header=header)
        (count, size), = run.groups.values()
        p100, p5000 = -math.expm1(-0.1), -math.expm1(-5.0)
        self.assertAlmostEqual(count, 1 / p100 + 1 / p5000)
        self.assertAlmostEqual(size, 100 / p100 + 5000 / p5000)

    def test_report_diff_orders_by_absolute_byte_delta(self):
        cand = self.run_of(["a 1 layout 100 10 1 2", "a 1 paint 9000 20 1 3"])
        base = self.run_of(["a 1 layout 400 10 1 2"])
        report = memprof.build_report(cand, base, True, 8, 10)
        self.assertEqual([r["subsystem"] for r in report["subsystems"]], ["paint", "layout"])
        self.assertEqual(report["subsystems"][1]["delta_bytes"], -300)
        self.assertEqual(report["schema"], memprof.SCHEMA)
        json.dumps(report)

    def test_owner_report_reconciles_with_the_live_heap_at_its_start(self):
        header = "# engine_bench alloc stacks v3 base=0 filter=* until=- sample=0"
        run = self.run_of([
            "a 1 layout 100 10 1 2",
            "a 1 paint 30 20 1 3",
            "a 1 paint 7 30 1 3",
            "a 1 style 50 40 1 4",
            "f 1 paint 7 30",
            "o dom/nodes 10 100",
            "a 2 outside 999 50 1 4",  # allocated during the walk: after the snapshot
            "o layout/taffy 20 24",  # a wrong size is reported, the block still matches
            "o layout/taffy 0 50",  # no address: an estimate claims the live block of its size
            "o style/computed 30 7",  # freed before the report: unmatched
        ], header=header)
        owners = run.owners
        self.assertEqual(owners["paths"], {"dom/nodes": 100, "layout/taffy": 74, "style/computed": 7})
        self.assertEqual(owners["live_bytes"], 180)
        self.assertEqual(owners["matched_bytes"], 130)
        self.assertEqual(owners["matched_by_size_bytes"], 50)
        self.assertEqual(owners["estimated_bytes"], 50)
        self.assertEqual(owners["unmatched_bytes"], 7)
        self.assertEqual(owners["size_mismatches"], 1)
        report = memprof.build_report(run, None, True, 8, 10)
        self.assertEqual([r["owner"] for r in report["owners"]], ["dom/nodes", "layout/taffy", "style/computed"])
        self.assertEqual(report["unreported_sites"], [])
        json.dumps(report)


class FoldedSymbols(FakeSymbols):
    """Symbols of an image whose linker folded NAMES[2] with two other functions."""

    def __init__(self, names, functions=10, folded=True):
        super().__init__(names)
        group = sorted([names[2], "taffy::tree::TaffyTree::with_capacity", "zzz::other"])
        self.result = (functions - 2, functions, {name: group for name in group} if folded else {})

    def scan(self):
        return self.result


class SymbolQuality(unittest.TestCase):
    def report(self, symbols):
        path = probe_file(["a 1 layout 100 10 1 2", "a 1 paint 30 20 1 3"])
        self.addCleanup(pathlib.Path(path).unlink)
        report = memprof.build_report(memprof.Run("x.exe", path, True, symbols=symbols), None, True, 8, 10)
        out = io.StringIO()
        memprof.render(report, out)
        json.dumps(report)
        return report, out.getvalue()

    def test_folded_functions_are_warned_about_and_marked_in_the_rows(self):
        report, text = self.report(FoldedSymbols(NAMES))
        self.assertEqual(report["runs"]["cand"]["symbols"],
                         {"functions": 10, "folded_addresses": 1, "folded_names": 3})
        self.assertIn("WARNING cand: the exe was linked with identical-code folding (ICF): 3 functions share 1",
                      text)
        self.assertIn("kamin_html::layout::a {ICF 3}", text)
        self.assertNotIn("kamin_html::paint::b {ICF", text)
        self.assertIn("    = taffy::tree::TaffyTree::with_capacity", text)
        self.assertEqual(list(report["folded_sites"]), ["kamin_html::layout::a"])

    def test_an_image_without_folding_has_no_warning(self):
        report, text = self.report(FoldedSymbols(NAMES, folded=False))
        self.assertEqual(report["warnings"], [])
        self.assertNotIn("ICF", text)

    def test_a_pdb_without_function_records_is_warned_about(self):
        _, text = self.report(FoldedSymbols(NAMES, functions=0, folded=False))
        self.assertIn("WARNING cand: the PDB beside the exe has no function records", text)


class RunCommand(unittest.TestCase):
    def setUp(self):
        self.out = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(__import__("shutil").rmtree, self.out, True)
        for name in ("Symbols",):
            self.addCleanup(setattr, memprof, name, getattr(memprof, name))
        memprof.Symbols = lambda exe, base: FakeSymbols(NAMES)
        original = memprof.subprocess.run
        self.addCleanup(setattr, memprof.subprocess, "run", original)
        self.seen = []

        def bench(command, env, **_):
            # The previous side's probe file must be gone before the next side writes its own.
            self.seen.append(sorted(p.name for p in self.out.glob("*.stacks")))
            pathlib.Path(env["ENGINE_BENCH_ALLOC_STACKS_OUT"]).write_text(
                "# engine_bench alloc stacks v3 base=0 filter=*@0-2 until=paint sample=0\na 1 layout 100 10 1 2\n",
                encoding="utf-8")
            return __import__("types").SimpleNamespace(returncode=0)

        memprof.subprocess.run = bench

    def run_memprof(self, *extra):
        with unittest.mock.patch("sys.stdout", io.StringIO()), unittest.mock.patch("sys.stderr", io.StringIO()):
            memprof.main(["run", "--exe", "c.exe", "--base-exe", "b.exe", "--at", "paint@2", "--out", str(self.out),
                          "--min-free-gb", "0", *extra, "--", "p.html"])

    def test_each_probe_file_is_removed_once_folded(self):
        self.run_memprof()
        self.assertEqual(self.seen, [[], []])
        self.assertEqual(list(self.out.glob("*.stacks")), [])
        report = json.loads((self.out / "report.json").read_text(encoding="utf-8"))
        self.assertEqual(report["subsystems"][0]["cand_bytes"], 100)
        self.assertIn("by subsystem", (self.out / "report.txt").read_text(encoding="utf-8"))

    def test_keep_stacks_leaves_the_probe_files(self):
        self.run_memprof("--keep-stacks")
        self.assertEqual(sorted(p.name for p in self.out.glob("*.stacks")), ["base.stacks", "cand.stacks"])

    def test_a_hold_file_stops_the_run_before_the_bench_starts(self):
        hold = self.out / "HOLD-PAIRS"
        hold.write_text("someone measures", encoding="utf-8")
        with unittest.mock.patch.dict(memprof.os.environ, {"KAMIN_BENCH_HOLD": str(self.out / "HOLD*")}):
            with self.assertRaises(SystemExit) as stop:
                self.run_memprof()
            self.assertIn("HOLD-PAIRS", str(stop.exception))
            self.assertEqual(self.seen, [])
            self.run_memprof("--ignore-hold")
        self.assertEqual(len(self.seen), 2)

    def test_an_exact_run_needs_free_disk_space(self):
        with self.assertRaises(SystemExit) as stop:
            with unittest.mock.patch("sys.stderr", io.StringIO()):
                memprof.main(["run", "--exe", "c.exe", "--at", "paint@2", "--out", str(self.out),
                              "--min-free-gb", "1e9", "--", "p.html"])
        self.assertIn("GiB free", str(stop.exception))
        self.assertEqual(self.seen, [])


class Build(unittest.TestCase):
    def test_flags_keep_the_config_and_add_noicf(self):
        import memprof_build

        root = pathlib.Path(tempfile.mkdtemp())
        self.addCleanup(__import__("shutil").rmtree, root, True)
        (root / ".cargo").mkdir()
        (root / ".cargo" / "config.toml").write_text(
            '[target.x86_64-pc-windows-msvc]\nrustflags = ["-C", "link-args=/STACK:8388608"]\n', encoding="utf-8")
        flags = memprof_build.rustflags(root)
        self.assertEqual(flags, ["-C", "link-args=/STACK:8388608", "-C", "link-arg=/OPT:NOICF"])
        self.assertEqual(memprof_build.expected_stack(flags), 8388608)
        self.assertEqual(memprof_build.rustflags(root / "absent"), ["-C", "link-arg=/OPT:NOICF"])

    def test_the_repository_config_keeps_its_stack_in_the_profiler_build(self):
        import memprof_build

        flags = memprof_build.rustflags(pathlib.Path(__file__).resolve().parent.parent)
        self.assertEqual(memprof_build.expected_stack(flags), 8 << 20)
        self.assertEqual(flags[-2:], memprof_build.NOICF)

    @unittest.skipUnless(sys.platform == "win32", "reads a PE image")
    def test_stack_reserve_is_read_from_the_pe_header(self):
        import memprof_build

        self.assertGreater(memprof_build.stack_reserve(sys.executable), 0)


@unittest.skipUnless(sys.platform == "win32", "memprof ws reads Windows process memory")
class WorkingSet(unittest.TestCase):
    def test_pages_are_classified_by_region_type_and_reservation(self):
        import memprof_ws as ws

        res = lambda size, guard=False, wc=False: {"size": size, "guard": guard, "wc": wc}  # noqa: E731
        self.assertEqual(ws.category(ws.MEM_IMAGE, res(1 << 30)), "image")
        self.assertEqual(ws.category(ws.MEM_MAPPED, res(1 << 30)), "mapped")
        self.assertEqual(ws.category(ws.MEM_PRIVATE, res(1 << 20, guard=True)), "stack")
        self.assertEqual(ws.category(ws.MEM_PRIVATE, res(1 << 20, wc=True)), "wc/nocache")
        self.assertEqual(ws.category(ws.MEM_PRIVATE, res(ws.BIG_RESERVATION)), "big-res")
        self.assertEqual(ws.category(ws.MEM_PRIVATE, res(ws.BIG_RESERVATION - 1)), "small-res")

    def test_own_process_breakdown_covers_its_working_set(self):
        import ctypes
        import os
        import memprof_ws as ws

        h = ws.k32.OpenProcess(ws.PROCESS_QUERY_INFORMATION | ws.PROCESS_VM_READ, False, os.getpid())
        try:
            b = ws.breakdown(h)
            pmc = ws.PMC()
            pmc.cb = ctypes.sizeof(pmc)
            self.assertTrue(ws.psapi.GetProcessMemoryInfo(h, ctypes.byref(pmc), pmc.cb))
        finally:
            ws.k32.CloseHandle(h)
        total = sum(b["categories"].values())
        self.assertGreater(b["categories"].get("image", 0) + b["categories"].get("image (shared)", 0), 0)
        # Resident pages and the reported working set are taken moments apart.
        self.assertLess(abs(total - pmc.WorkingSetSize), pmc.WorkingSetSize // 4)


if __name__ == "__main__":
    unittest.main()
