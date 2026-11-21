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

    def test_browser_visit_definitions(self):
        res = {'twoRafMs': 40, 'probe': {'frames': 10, 'cases': {
            'R': {'frame': {'median': 2}, 'frameReq': {'median': 5}},
            'S': {'frame': {'median': 1, 'moved': 2}, 'frameReq': {'median': 1}}}},
            'trace': {'windows': {'frame:R': {'RasterTask': {'sumMs': 10}, 'Display::DrawAndSwap': {'sumMs': 0}},
                                  'req:R': {'RasterTask': {'sumMs': 20}}}}}
        v = M.browser_visit('chromium', res)
        self.assertEqual((v['R']['raf'], v['R']['req'], v['R']['value']), (3.0, 7.0, 3.0))
        self.assertIsNone(v['S']['value'])  # fewer than 3 whole steps
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


class Repeats(unittest.TestCase):
    def test_policy(self):
        p = Policy()
        self.assertEqual(p.want([10, 10.2, 9.9]), 3)
        self.assertEqual(p.want([2.0]), 8)          # fast rows get 8
        self.assertEqual(p.want([10, 20, 15]), 5)   # noisy rows get more
        self.assertEqual(p.want([None]), 1)         # failures are not retried

    def test_noise_verdict(self):
        self.assertEqual(noise_verdict([10, 10.5, 10.2], [8, 8.2, 8.1])[0], 'better')
        self.assertEqual(noise_verdict([10, 12, 11], [11, 13, 12])[0], 'noise')   # ranges overlap
        self.assertEqual(noise_verdict([10, 10.1], [10.3, 10.4])[0], 'noise')     # inside the 5 % floor
        self.assertEqual(noise_verdict([10, 10.1], [13, 13.2])[0], 'worse')


if __name__ == '__main__':
    unittest.main()
