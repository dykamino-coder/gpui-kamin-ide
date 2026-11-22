"""Unit tests: record schema and cache keys, scheduler choice, metric definitions, adaptive repeats, diff noise.
python -I -m unittest discover -s tools/engine-compare/tests"""
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from ec import metrics as M  # noqa: E402
from ec import records as REC  # noqa: E402
from ec.adapters import Policy  # noqa: E402
from ec.config import CLASSES, Page, lane_mask  # noqa: E402
from ec.report import noise_verdict  # noqa: E402
from ec.scheduler import Task, pick  # noqa: E402


def page(tmp, pid='p'):
    d = Path(tmp) / pid; d.mkdir()
    for f in ('base.html', 'vis.html', 'probe.html'):
        (d / f).write_text(f'<p>{f}</p>', encoding='utf-8')
    return Page(pid, 'syn', d, {})


class Records(unittest.TestCase):
    def test_key_changes_with_every_input_and_only_with_it(self):
        k = REC.unit_key('ours', 'v1', 'h', 'gpu4', 'all', 'd')
        self.assertEqual(k, REC.unit_key('ours', 'v1', 'h', 'gpu4', 'all', 'd'))
        for i in range(6):
            args = ['ours', 'v1', 'h', 'gpu4', 'all', 'd']; args[i] += 'x'
            self.assertNotEqual(k, REC.unit_key(*args))

    def test_page_hash_follows_sources_but_not_derived_variants(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = page(tmp); h = p.hash
            (p.dir / 'C.html').write_text('derived', encoding='utf-8')
            self.assertEqual(h, Page(p.id, p.kind, p.dir, {}).hash)
            (p.dir / 'base.html').write_text('<p>changed</p>', encoding='utf-8')
            self.assertNotEqual(h, Page(p.id, p.kind, p.dir, {}).hash)

    def test_save_load_and_every_empty_cell_needs_a_reason(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = page(tmp)
            rec = REC.new_record('chrome', 'v', 'all', p, 'gpu4', 'd', 'bounds')
            REC.row(rec, 'R')['time_ms'] = 1.5
            REC.row(rec, 'S')
            self.assertEqual(REC.validate(rec), ['S: neither a number nor a reason'])
            REC.row(rec, 'S')['empty'] = 'page does not scroll'
            self.assertEqual(REC.validate(rec), [])
            REC.save(Path(tmp) / 'store', rec)
            back = REC.load(Path(tmp) / 'store', 'chrome', 'gpu4', 'p', rec['key'])
            self.assertEqual(back['rows']['R']['time_ms'], 1.5)
            self.assertEqual(len(list(REC.scan(Path(tmp) / 'store'))), 1)


class Scheduler(unittest.TestCase):
    def t(self, eng, gpu=False, session=None, exclusive=False):
        return Task(eng, None, 'gpu4', 'all', 'k', gpu, session, exclusive)

    def test_same_session_first(self):
        q = [self.t('a', session=('a', 'gpu4')), self.t('b', session=('b', 'gpu4'))]
        self.assertEqual(pick(q, ('b', 'gpu4'), 0, 2, False), 1)
        self.assertEqual(pick(q, None, 0, 2, False), 0)

    def test_gpu_limit_and_exclusive_units(self):
        q = [self.t('g', gpu=True), self.t('k', exclusive=True), self.t('c')]
        self.assertEqual(pick(q, None, 2, 2, True), 2)
        self.assertEqual(pick(q[:2], None, 2, 2, False), 1)
        self.assertIsNone(pick(q[:2], None, 2, 2, True))  # exclusive waits for the other lanes

    def test_job_pid_list_counts_sit_in_the_first_slot(self):
        from ec.winproc import job_pids
        buf = [(3 << 32) | 3, 101, 202, 303, 0]
        self.assertEqual(job_pids(buf, 4), [101, 202, 303])
        self.assertEqual(job_pids(buf, 2), [101, 202])

    def test_lane_masks_shift_class_masks(self):
        self.assertEqual(lane_mask('gpu4', 8), 0xF00)
        self.assertEqual(lane_mask('soft1', 4), 0x80)
        self.assertEqual(set(CLASSES), {'gpu4', 'gpu2', 'soft2', 'soft1'})


class Metrics(unittest.TestCase):
    def test_ours_frame_subtracts_capture_copy_and_cold_host(self):
        j = {'modes': {'cold': {'samples_ms': [{'first_frame_total': 10, 'pipeline_other': 1, 'host_resources': 2}] * 3,
                                'diagnostic_scopes': {'samples_ms': [{'capture_copy': 0.5}] * 3}}}}
        r = M.ours_frame(j, 'cold')
        self.assertEqual((r['req'], r['raf']), (9.5, 6.5))

    def test_ours_cpu_counts_phases_except_pipeline_other(self):
        s = {'phases': {'layout': {'process_cpu_100ns': 20000}, 'pipeline_other': {'process_cpu_100ns': 90000}}}
        self.assertEqual(M.ours_cpu({'modes': {'edit': {'process_resources': {'samples': [s, s]}}}}, 'edit'), 2.0)

    def test_cycle_cpu_uses_the_calibration(self):
        rep = {'modes': {'relayout': {'diagnostic_scopes': {'samples_ms': [{'process_cpu_mcycles': 3.0}, {'process_cpu_mcycles': 6.0}]}}}}
        self.assertEqual(M.ours_cpu_cycles(rep, 'relayout', {'cycles_per_cpu_ms': 3e6}), 1.5)
        self.assertIsNone(M.ours_cpu_cycles(rep, 'relayout', {}))

    def test_oneshot_cpu_is_the_page_minus_the_empty_page_in_cycle_time(self):
        page = [{'cpu_s': 0.30, 'cycles': 900e6}, {'cpu_s': 0.32, 'cycles': 960e6}, {'cpu_s': 0.31, 'cycles': 930e6}]
        blank = [{'cpu_s': 0.10, 'cycles': 300e6}] * 3
        self.assertAlmostEqual(M.oneshot_cpu_ms(page, blank), 210.0, places=3)  # 3e6 cycles per ms
        self.assertIsNone(M.oneshot_cpu_ms(blank, page))
        self.assertIsNone(M.oneshot_cpu_ms(page, []))

    def test_ours_steady_takes_the_window_median_and_notes_a_busy_page(self):
        quiet = {'page': {'median': {'rss_bytes': 50}, 'end': {'rss_bytes': 70}, 'quiet': True, 'busy_windows': 2}}
        self.assertEqual(M.ours_steady(quiet), (50, ''))
        old = {'page': {'end': {'rss_bytes': 70}}}  # benches before the median
        self.assertEqual(M.ours_steady(old), (70, ''))
        busy = {'page': {'median': {'rss_bytes': 60}, 'quiet': False, 'busy_windows': 10, 'unexpected_frames': 4}}
        value, note = M.ours_steady(busy)
        self.assertEqual(value, 60); self.assertIn('kept painting: 4 frames', note)
        self.assertEqual(M.ours_steady({'error': 'x'})[0], None)
        self.assertEqual(M.ours_steady(None), (None, ''))

    def test_scroll_scenario_is_the_browser_probe_pass_down_to_one_step(self):
        self.assertEqual(M.scroll_steps({'scrollHeight': 600}), 0)
        self.assertEqual(M.scroll_steps({'scrollHeight': 719}), 0)
        self.assertEqual(M.scroll_steps({'scrollHeight': 720}), 1)
        self.assertEqual(M.scroll_steps({'scrollHeight': 50000}), 14)
        self.assertIsNone(M.scroll_steps({}))
        one = {'probe': {'cases': {'S': {'frame': {'median': 2, 'moved': 1}, 'frameReq': {'median': 3}}}}}
        self.assertEqual(M.browser_visit('firefox', one)['S']['raf'], None)  # no afterPaint: Firefox raf unknown
        self.assertEqual(M.browser_visit('chromium', dict(one, trace={'windows': {'frame:S': {'RasterTask': {'sumMs': 0}}}}))['S']['raf'], 2)

    def test_servo_frame_is_edit_plus_screenshot_minus_screenshot(self):
        res = {'probe': {'cases': {'R': {'frame': {'median': 0.3}, 'frameReq': {'median': 0.5}}, 'C': {'frame': {'median': 1}}}},
               'shots': {'idle': {'ms': [20, 21, 22]}, 'R': {'ms': [30, 31, 33]}, 'C': {'ms': [19, 20]}}}
        sv = M.browser_visit('servo', res)
        self.assertEqual(sv['R']['value'], 10)  # 31 - 21: the probe clocks are not used
        self.assertIsNone(sv['C']['value'])     # not above the screenshot alone

    def test_browser_visit_definitions(self):
        res = {'twoRafMs': 40, 'probe': {'frames': 10, 'cases': {
            'R': {'frame': {'median': 2}, 'frameReq': {'median': 5}},
            'S': {'frame': {'median': 1, 'moved': 0}, 'frameReq': {'median': 1}}}},
            'trace': {'windows': {'frame:R': {'RasterTask': {'sumMs': 10}, 'Display::DrawAndSwap': {'sumMs': 0}},
                                  'req:R': {'RasterTask': {'sumMs': 20}}}}}
        v = M.browser_visit('chromium', res)
        self.assertEqual((v['R']['raf'], v['R']['req'], v['R']['value']), (3.0, 7.0, 3.0))
        self.assertIsNone(v['S']['value'])  # no whole step
        self.assertEqual(v['cold']['value'], 40)
        ff = M.browser_visit('firefox', {'probe': {'cases': {'C': {'frame': {'median': 2}, 'frameReq': {'median': 4}, 'afterPaint': {'median': 3}}}}})
        self.assertEqual((ff['C']['raf'], ff['C']['req']), (3, 5))

    def test_cpu_windows_and_tree_memory(self):
        marks = {'nav': (0.0, 1.0, 0, 0), 'loaded': (1.0, 1.5, 0, 0), 'steady_end': (3.0, 1.6, 0, 0),
                 'cpu_R_0': (4.0, 2.0, 0, 0), 'cpu_R_1': (5.0, 2.3, 0, 0)}
        c = M.cpu_windows(marks, {'R': 30})
        self.assertAlmostEqual(c['R'], 10.0); self.assertAlmostEqual(c['cold'], 500.0)
        samples = [(0.5, 0, 100, 0), (1.6, 0, 300, 0), (2.0, 0, 200, 0), (2.5, 0, 220, 0), (4.0, 0, 900, 0)]
        self.assertEqual(M.tree_memory(samples, marks), (900, 220))


class NarrowLanes(unittest.TestCase):
    def test_soft1_runs_on_one_core_lanes_after_the_wide_classes(self):
        import tempfile
        from ec.scheduler import Lane, Task, groups
        with tempfile.TemporaryDirectory() as d:
            wide = [Lane(i, b, Path(d) / f'w{i}') for i, b in enumerate([0, 4, 8])]
            mk = lambda cls: Task('e', None, cls, 'all', 'k', False, None, False)
            todo = [mk('gpu4'), mk('soft1'), mk('soft2'), mk('soft1')]
            self.assertEqual(groups(todo, wide, d, False), [(wide, todo)])
            (l1, t1), (l2, t2) = groups(todo, wide, d, True)
            self.assertEqual([t.cls for t in t1], ['gpu4', 'soft2']); self.assertEqual([t.cls for t in t2], ['soft1'] * 2)
            self.assertEqual([x.mask('soft1') for x in l2], [0x2 << b for b in (0, 2, 4, 6, 8, 10)])
            self.assertEqual(len({x.id for x in l2} | {x.id for x in wide}), 9)
            self.assertEqual(wide[1].mask('soft1'), 0x8 << 4)   # the same one thread of an idle core
            with self.assertRaises(ValueError):
                l2[0].mask('gpu2')


class Repeats(unittest.TestCase):
    def test_policy(self):
        p = Policy()
        self.assertEqual(p.want([10, 10.2, 9.9]), 3)        # stable after the minimum: stop
        self.assertEqual(p.want([2.0]), 3)                  # the minimum first, fast rows too
        self.assertEqual(p.want([10, 20, 15]), 4)           # unstable: one more at a time
        self.assertEqual(p.want([10, 20, 15, 11, 12, 30, 9, 25]), 8)  # the cap
        self.assertEqual(p.want([10, 10.1, 30, 10.2]), 4)   # MAD ignores one outlier
        self.assertEqual(p.want([None]), 1)                 # failures are not retried
        self.assertAlmostEqual(M.rel_mad([10, 11, 9]), 0.1)
        self.assertIsNone(M.rel_mad([5]))

    def test_noise_verdict(self):
        self.assertEqual(noise_verdict([10, 10.5, 10.2], [8, 8.2, 8.1])[0], 'better')
        self.assertEqual(noise_verdict([10, 12, 11], [11, 13, 12])[0], 'noise')   # ranges overlap
        self.assertEqual(noise_verdict([10, 10.1], [10.3, 10.4])[0], 'noise')     # inside the 5 % floor
        self.assertEqual(noise_verdict([10, 10.1], [13, 13.2])[0], 'worse')



class Native(unittest.TestCase):
    def test_phase_plan_uses_the_shared_edits_and_the_scenario_steps(self):
        from ec.adapters.native import phase_plan
        with tempfile.TemporaryDirectory() as tmp:
            p = page(tmp)
            self.assertEqual(phase_plan(p, 'R'), {'name': 'R', 'mode': 'resize', 'widths': [640, 800]})
            c = phase_plan(p, 'C')
            self.assertEqual((c['selector'], c['attribute'], c['value'], c['observe'], c['visibility']),
                             ('html', 'class', 'alt', '[data-deep]', 'offscreen'))
            self.assertTrue(c['input'].endswith('C.html') and (p.dir / 'C.html').exists())
            d = phase_plan(p, 'Dvis')
            self.assertEqual((d['selector'], d['observe'], d['visibility']), ('@middle', 'b4-e', 'visible'))
            self.assertTrue(d['input'].endswith('vis.html'))
            self.assertEqual(phase_plan(p, 'S', 5)['steps'], 5)

    def test_fill_cpu_per_frame_named_refusals_and_cold_sample(self):
        from ec.adapters.native import fill
        with tempfile.TemporaryDirectory() as tmp:
            rec = REC.new_record('blitz', 'v', 'all', page(tmp), 'soft2', 'd', 'b')
            rep = {'phases': [
                {'name': 'cold', 'status': 'ok', 'frames_ms': [400.0], 'cpu': {'cpu_ms': 450.0, 'cycles': 9}},
                {'name': 'R', 'status': 'ok', 'frames_ms': [10.0, 20.0, 30.0, 40.0], 'series_elapsed_ms': 510,
                 'cpu': {'cpu_ms': 120.0, 'cycles': 400}},
                {'name': 'C', 'status': 'unsupported', 'frames_ms': [],
                 'error': 'native target has an empty layout rectangle; C:\\x\\geometry.rs:398'},
                {'name': 'S', 'status': 'ok', 'frames_ms': [5.0, 6.0]}]}
            self.assertEqual(fill(rec, rep, 'Blitz', want_steps=3), (400.0, 450.0))
            r = rec['rows']
            self.assertEqual((r['R']['time_ms'], r['R']['cpu_ms'], r['R']['cpu_cycles']), (25.0, 30.0, 100.0))
            self.assertIn('empty layout rectangle', r['C']['empty']); self.assertNotIn('C:\\x', r['C']['empty'])
            self.assertIn('no per-phase CPU', r['S']['cpu_empty']); self.assertIn('2 of the', r['S']['note'])
            self.assertEqual(REC.validate(rec), [])


class WarmUp(unittest.TestCase):
    def test_one_dropped_run_per_engine_before_its_first_unit(self):
        from ec.scheduler import Lane, Runner

        class Ad:
            exclusive = False; calls = []

            def version(self):
                return 'v'

            def needs_gpu(self, cls):
                return False

            def close(self, lane):
                pass

            def measure(self, lane, pg, cls, part, policy):
                self.calls.append(policy.min_repeats)
                return {'key': 'k', 'rows': {}, 'engine': 'x', 'cls': cls, 'page': pg.id}

        class Cfg:
            raw = {}; store = None

        with tempfile.TemporaryDirectory() as tmp:
            Cfg.store = Path(tmp) / 'store'; p = page(tmp); ad = Ad()
            lanes = [Lane(i, 4 * i, Path(tmp) / f'l{i}') for i in range(2)]
            r = Runner(Cfg, {'x': ad}, Policy(), lanes, 2, log=lambda m: None)
            r.run([Task('x', p, c, 'all', 'k', False, None, False) for c in ('soft2', 'soft1', 'gpu4')])
            self.assertEqual(ad.calls.count(1), 1)  # the quick warm-up, once
            self.assertEqual(ad.calls.count(3), 3)  # every measured unit
            self.assertEqual(ad.calls[0], 1)


class Settle(unittest.TestCase):
    def test_median_interval_from_order_statistics(self):
        self.assertIsNone(M.median_ci([1.0]))
        self.assertEqual(M.median_ci([3, 1, 2]), (1, 3, 0.75))  # too few values: the full range
        lo, hi, cov = M.median_ci(list(range(14)))
        self.assertEqual((lo, hi), (3, 10)); self.assertGreaterEqual(cov, 0.9)

    def test_row_settles_on_a_narrow_interval_or_a_decided_place(self):
        p = Policy()
        self.assertIsNone(p.settled([10.0, 10.1]))  # fewer than 3
        self.assertEqual(p.settled([10.0, 10.1, 10.2, 9.9, 10.0]), 'ci')
        wide = [10, 13, 8, 12, 9]
        self.assertIsNone(p.settled(wide))
        self.assertEqual(p.settled(wide, ours=5.0), 'place')  # whole interval above ours by more than 3 %
        self.assertIsNone(p.settled(wide, ours=10.0))
        tight = [10.0, 10.1, 10.2, 9.9, 10.0]
        self.assertIsNone(p.settled(tight, visit_medians=[9.0, 11.0]))  # visits disagree by 20 %: not settled
        self.assertAlmostEqual(M.accuracy(tight, [9.0, 11.0]), 0.1)

    def test_browser_samples_take_the_lower_clock_with_raster_per_frame(self):
        res = {'probe': {'frames': 2, 'cases': {
            'R': {'frame': {'samples': [4.0, 6.0]}, 'frameReq': {'samples': [8.0, 9.0]}},
            'S': {'frame': {'samples': [1.0], 'moved': 0}}}},
            'trace': {'windows': {'frame:R': {'RasterTask': {'sumMs': 2.0}}, 'req:R': {'RasterTask': {'sumMs': 0.0}}}}}
        s = M.browser_samples('chrome', res)
        self.assertEqual(s['R'], {'value': 6.0, 'samples': [5.0, 7.0], 'clock': 'raf'})
        self.assertNotIn('S', s)  # no whole scroll step: no sample

class UnitTime(unittest.TestCase):
    def test_timeouts_are_counted_per_page(self):
        from types import SimpleNamespace as NS
        from ec.report import L, add_pass, unit_table
        cut = {'wall_s': 61.0, 'rows': {'cold': {'empty': 'timeout: no result within 60 s'}}, 'memory': {}}
        ok = {'wall_s': 9.0, 'rows': {'cold': {'time_ms': 5.0, 'repeats': [5.0]}}, 'memory': {}}
        self.assertTrue(M.timed_out(cut)); self.assertFalse(M.timed_out(ok))
        self.assertTrue(M.timed_out({'rows': {'R': {'empty': 'hangs: no result within 300 s'}}}))  # older records
        ads = {'ours': NS(e={'adapter': 'ours'})}
        passes = {}
        for cls, rec in (('gpu2', cut), ('soft1', ok)):
            ps = passes.setdefault((cls, 'ours'), {'wall': 0.0, 'engine': 0.0, 'pages': set(), 'units': 0, 'missing': 0})
            add_pass(ps, rec, NS(engine='ours', page=NS(id='p1')), ads, [])
        self.assertEqual(passes[('gpu2', 'ours')]['per_page']['p1'], [61.0, 61.0])
        self.assertEqual(passes[('soft1', 'ours')]['per_page']['p1'], [9.0, 0.0])
        h = unit_table(passes, L['en'], 'ours')
        self.assertIn('<td class="num">70 <small class="cap">timeout 61 s</small></td>', h)
        self.assertIn('<td class="num r">61</td>', h)


if __name__ == '__main__':
    unittest.main()
