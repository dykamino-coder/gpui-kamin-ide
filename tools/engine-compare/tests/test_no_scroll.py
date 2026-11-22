"""Non-scrollable pages have localized empty cells, never crash verdicts."""
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from ec import metrics as M
from ec.adapters import Policy
from ec.adapters.browser import Browser
from ec.adapters.ours import Ours
from ec.config import CLASSES, Page
from ec.report import L, cpu_why, render, why


class NoScroll(unittest.TestCase):
    def test_known_short_pages_do_not_launch_a_scroll_process(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'base.html').write_text('<p>short</p>', encoding='utf-8')
            lane = SimpleNamespace(id=0, work=root, mask=lambda cls: 8)
            for plan in (False, True):
                ad = Ours('ours', None, dict(version='test', exe='unused', plan=plan))
                for height in (480, 600, 719):
                    page = Page('short', 'syn', root, dict(scrollHeight=height))
                    for cls in CLASSES:
                        with self.subTest(plan=plan, height=height, cls=cls), patch.object(ad, '_run') as run:
                            rec = ad.measure(lane, page, cls, 'all' if plan else 'time', Policy(), ['S'])
                            run.assert_not_called()
                            self.assertEqual(rec['rows']['S']['empty'], M.NO_SCROLL)
                            self.assertIsNone(rec['rows']['S'].get('time_ms'))

    def test_browser_absent_scroll_precedes_startup_and_preserves_other_errors(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            lane = SimpleNamespace(id=0, work=root, mask=lambda cls: 8)
            for browser in ('chrome', 'edge', 'firefox', 'servo'):
                ad = Browser(browser, None, dict(version='test', exe='unused'))
                for height in (480, 600, 719):
                    page = Page('short', 'syn', root, dict(scrollHeight=height))
                    for cls in CLASSES:
                        with self.subTest(browser=browser, height=height, cls=cls):
                            with patch.object(ad, '_session', side_effect=RuntimeError('startup failed')) as start:
                                rec = ad.measure(lane, page, cls, 'all', Policy(), ['S'])
                                start.assert_not_called()
                                self.assertEqual(rec['rows']['S']['empty'], M.NO_SCROLL)
                                rec = ad.measure(lane, page, cls, 'all', Policy(), ['R', 'S'])
                                start.assert_called_once()
                                self.assertEqual(rec['rows']['S']['empty'], M.NO_SCROLL)
                                self.assertEqual(rec['rows']['R']['empty'], 'startup failed')

    def test_one_whole_step_control_still_runs_every_class(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'base.html').write_text('<div style="height:720px"></div>', encoding='utf-8')
            page = Page('one-step', 'syn', root, dict(scrollHeight=720))
            lane = SimpleNamespace(id=0, work=root, mask=lambda cls: 8)
            for cls in CLASSES:
                for plan in (False, True):
                    ad = Ours('ours', None, dict(version='test', exe='unused', plan=plan))
                    with self.subTest(cls=cls, plan=plan), patch.object(ad, '_run', return_value=(None, 'control reached', 0)) as run:
                        rec = ad.measure(lane, page, cls, 'all' if plan else 'time', Policy(), ['S'])
                        run.assert_called_once()
                        self.assertEqual(rec['rows']['S']['empty'], 'control reached')
                for browser in ('chrome', 'edge', 'firefox', 'servo'):
                    ad = Browser(browser, None, dict(version='test', exe='unused'))
                    with self.subTest(cls=cls, browser=browser):
                        with patch.object(ad, '_session', side_effect=RuntimeError('control reached')) as start:
                            rec = ad.measure(lane, page, cls, 'all', Policy(), ['S'])
                            start.assert_called_once()
                            self.assertEqual(rec['rows']['S']['empty'], 'control reached')

    def test_named_reason_and_other_empty_reasons(self):
        for lang, reason in (('ru', 'страница не прокручивается'), ('en', 'page does not scroll')):
            with self.subTest(lang=lang):
                row = dict(empty=M.NO_SCROLL)
                self.assertEqual(why(row, L[lang]), reason)
                self.assertEqual(cpu_why(row, L[lang]), reason)
                self.assertEqual(why(dict(empty='not_measured'), L[lang]), L[lang]['not_measured'])
                self.assertEqual(why(dict(empty='crashed: test'), L[lang]), 'crashed: test')

    def test_report_keeps_64_scroll_cells_empty_and_still_counts_a_crash(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'base.html').write_text('<p>short</p>', encoding='utf-8')
            pages = {f'short-{i}': Page(f'short-{i}', 'syn', root, {}) for i in range(16)}
            engines = {'ours': {'adapter': 'ours'}, **{b: {'adapter': 'browser'} for b in ('chrome', 'edge', 'firefox', 'servo')}}
            cfg = SimpleNamespace(pages=pages, engines=engines, causes=None, store=root)
            rows, memory = [], {}
            for cls in CLASSES:
                for page in pages:
                    for engine in engines:
                        memory[(cls, page, engine)] = dict(peak_bytes=1024, steady_bytes=1024)
                        for phase in M.PHASES:
                            row = dict(cls=cls, page=page, kind='syn', engine=engine, phase=phase)
                            row.update(dict(empty=M.NO_SCROLL) if phase == 'S' else dict(time_ms=10, cpu_ms=1))
                            rows.append(row)
            for lang in ('ru', 'en'):
                with self.subTest(lang=lang):
                    out = render(cfg, rows, memory, {}, {}, list(CLASSES), L[lang], '')
                    self.assertIn(L[lang]['ours_failsum'].format(n=0, t=0, c=0, m=0), out)
                    self.assertNotIn('class="r fail"', out)
                    self.assertIn(L[lang]['no_scroll'], out)
                    self.assertNotIn(M.NO_SCROLL, out)
            scroll = next(row for row in rows if row['phase'] == 'S')
            scroll['empty'] = 'crashed: test'
            out = render(cfg, rows, memory, {}, {}, list(CLASSES), L['ru'], '')
            self.assertIn(L['ru']['ours_failsum'].format(n=2, t=1, c=1, m=0), out)
            self.assertIn('class="r fail" title="crashed: test"', out)


if __name__ == '__main__':
    unittest.main()
