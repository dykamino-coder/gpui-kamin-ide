"""Tests of scripts/memprof.py: allocator frames, sites, subsystems, live folding and sampling weights."""

import json
import math
import pathlib
import sys
import tempfile
import unittest

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
            "# end frame=2 after=build_elements",
            "# dropped 0",
        ])
        totals = {key[0]: value for key, value in run.groups.items()}
        self.assertEqual(totals, {"style": [1.0, 64.0], "layout": [1.0, 8.0]})
        self.assertEqual(run.end, "end frame=2 after=build_elements")
        self.assertEqual(run.meta()["bytes"], 72)

    def test_allocated_mode_counts_every_allocation(self):
        run = self.run_of(["a 1 layout 100 10 1 2", "f 1 layout 100 10", "a 1 layout 50 10 1 2"], live=False)
        self.assertEqual(list(run.groups.values()), [[2.0, 150.0]])

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


if __name__ == "__main__":
    unittest.main()
