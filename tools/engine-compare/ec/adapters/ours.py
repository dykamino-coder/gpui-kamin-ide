"""Our engine through the prebuilt ``engine_bench`` executables.

part "time": timing build. cold + memory = one ``--mode both --runs 5 --memory-settle-ms 2000`` process (cold samples,
             peak working set, steady memory after the settle time); every warm phase = one process (the bench takes one
             mode per process), with N samples of the phase inside it.
part "cpu":  counters build, the same phases, CPU per frame from its per-phase process CPU.
Config: {"adapter": "ours", "exe": timing exe, "counters_exe": counters exe, "version": optional label,
         "timeout_s": {"syn": 300, "default": 120}}
"""
import shutil
import time

from .. import metrics as M
from .. import records as REC
from .. import winproc as W
from ..config import CLASSES, file_version
from . import last_error

RULE = 'html.alt body{color:#111}'
EDITS = {'C': ('html.alt [data-deep]{background:#eee}', 'class'), 'D': ('[data-edit]{background:#eee}', 'attr'),
         'Cvis': (None, 'class'), 'Dvis': (None, 'attr')}
BOUNDS = ('ours: one engine_bench process per phase on an offscreen GPU texture (software raster on soft classes); '
          'frame = from the frame request to pixels read back, minus the bench capture copy; cold = cold document in a '
          'warm process; memory = working set of the one process')
BOUNDS_PLAN = ('ours: one engine_bench process per page and class runs every phase (as one browser visit): cold = '
               'document-cold samples, then each warm phase on its own input after its load, adaptive frame series: at '
               'least 3 frames, then until the frame totals are stable (MAD / median <= 3 %) or 500 ms have passed '
               '(scroll: the browsers'' step count); before a phase the page is let go quiet (late decodes) up to 10 windows, a page that keeps '
               'painting is measured with a note; peak memory = the process over the whole visit; steady memory = '
               'median working set over a 2 s idle window after the cold phase; scroll fresh-document check on the first '
               'and the last step; frame = from the frame request to pixels read back, '
               'minus the bench capture copy; CPU = cycle time of all threads of the process over the frame window minus the stand capture window (copy, wake after readback), as browsers: whole tree over the frame loop')
# 2: CPU = all threads over the frame window minus the capture window; 3: one process per page (cold in the plan),
# quiet windows converge instead of refusing, steady memory = window median, scroll fresh check first + last step;
# 4: adaptive series (stable_mad)
DEF = 'ours-4'
STABLE_MAD = 0.03  # the series stop rule, the same as the browsers' repeat visits (adapters.Policy.stable)


class Ours:
    parts = ('time', 'cpu')
    exclusive = False

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e
        # "plan": a bench with --plan measures time, CPU and memory in two processes (part "all");
        # otherwise the old one-mode-per-process bench and a separate counters build (parts "time" and "cpu").
        self.parts = ('all',) if e.get('plan') else ('time', 'cpu')

    def version(self):
        return self.e.get('version') or file_version(self.e['exe']) + '+' + file_version(self.e['counters_exe'])

    def definition(self, part):
        return f'{DEF}-{part}'

    def needs_gpu(self, cls):
        return CLASSES[cls][1] == 'gpu'

    def session(self, lane, cls):
        return None

    def close(self, lane):
        pass

    def _page(self, page, phase):
        if phase in ('Cvis', 'Dvis'):
            return page.file('vis.html')
        if phase in ('C', 'D'):
            f = page.file(f'{phase}.html')
            if not f.exists():
                base = page.file('base.html').read_text(encoding='utf-8')
                f.write_text(base.replace(RULE, EDITS[phase][0], 1), encoding='utf-8', newline='')
            return f
        return page.file('base.html')

    def _run(self, exe, lane, page, cls, args, timeout):
        # A fresh TEMP/cwd per process: removing and re-creating one fixed directory raced with Windows' delayed
        # delete (the next bench started in a directory that then vanished: "os error 2", a false failure).
        old = getattr(self, '_tmp', {}).get(lane.id)
        if old:
            shutil.rmtree(old, ignore_errors=True)
        self._seq = getattr(self, '_seq', 0) + 1
        tmp = lane.work / f'ours-{lane.id}-{__import__("os").getpid()}-{self._seq}'; tmp.mkdir(parents=True, exist_ok=False)
        self._tmp = getattr(self, '_tmp', {}); self._tmp[lane.id] = tmp
        env = dict(__import__('os').environ, TEMP=str(tmp), TMP=str(tmp))
        gpu = 'default' if CLASSES[cls][1] == 'gpu' else 'software'
        err = tmp / 'stderr.txt'
        p = W.Proc([exe, str(args[0]), '--viewport', '800x600', '--gpu', gpu, '--json'] + args[1:], mask=lane.mask(cls),
                   desk=self.cfg.desktop, env=env, cwd=tmp, mem_cap=int(self.cfg.mem_cap_gb * (1 << 30)), stderr_path=err)
        out = []; w0 = time.perf_counter()
        import threading
        t = threading.Thread(target=lambda: out.append(p.stdout.read()), daemon=True); t.start()
        code = p.wait(timeout)
        if code is None:
            p.kill(); p.close(); t.join(5)
            return None, f'hangs: no result within {timeout} s', round(time.perf_counter() - w0, 2)
        t.join(10); p.close(); wall = round(time.perf_counter() - w0, 2)
        text = (out[0] if out else b'').decode('utf-8', 'replace')
        try:
            j = M.bench_json(text)
        except ValueError:
            j = None
        if code != 0 or not j:
            msg = err.read_text(encoding='utf-8', errors='replace') if err.exists() else ''
            return None, (f'crashed: {last_error(msg)}' if code != 0 else 'no report in the output'), wall
        return j, None, wall

    def _runs(self, page, est_ms):
        """Samples per warm process: cheap pages get more (the process start dominates), heavy pages fewer."""
        if est_ms is None or est_ms < 100:
            return 8
        return 5 if est_ms < 400 else 3

    def _plan_measure(self, lane, page, cls, policy, phases, part):
        """One process per page: every phase in one plan, cold first (document-cold samples), then the warm phases;
        steady memory once after the first phase on the base input, peak working set over the whole visit (as the
        browsers), CPU per frame from process cycle time."""
        rec = REC.new_record(self.name, self.version(), part, page, cls, self.definition(part) + '-' + policy.definition(), BOUNDS_PLAN)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        to = self.e.get('timeout_s', {}).get(page.kind, self.e.get('timeout_s', {}).get('default', 120))
        exe = self.e['exe']; series = self.e.get('series_ms', 500)
        order = [ph for ph in ('cold', 'R', 'S', 'C', 'D', 'Cvis', 'Dvis') if ph in phases]
        steps = M.scroll_steps(page.meta)
        if 'S' in order and steps == 0:  # the same scenario as the browsers: nothing to scroll
            REC.row(rec, 'S')['empty'] = M.NO_SCROLL; order.remove('S')
        plan = {'series_ms': series, 'min_runs': 3, 'stable_mad': self.e.get('stable_mad', STABLE_MAD), 'phases': [],
                'memory_settle_ms': 2000 if order and order[0] in ('cold', 'R') else 0}
        for ph in order:
            p = {'name': ph, 'input': str(self._page(page, ph)), 'runs': 30}
            if ph == 'cold':
                p['mode'] = 'cold'; p['runs'] = 5
            elif ph == 'R':
                p['mode'] = 'relayout'
            elif ph == 'S':
                # as many 120 px steps as the browser probe makes on this page (down to one); the bench stops
                # earlier only where its own document ends (whole step no longer fits)
                p.update(mode='scroll', runs=steps or M.SCROLL_MAX_STEPS, min_runs=1, series_ms=600000, stable_mad=0)
            else:
                p['mode'] = 'edit'; p['edit'] = EDITS[ph][1]
            plan['phases'].append(p)
        if order:
            pf = lane.work / f'plan-{lane.id}.json'
            import json
            pf.write_text(json.dumps(plan), encoding='utf-8')
            j, why, wall = self._run(exe, lane, page, cls, [self._page(page, order[0]), '--plan', str(pf)], to * 3)
            rec['plan_wall_s'] = wall
            rec['memory'] = {'empty': why} if why else {'peak_bytes': j.get('peak_working_set_bytes'), 'steady_bytes': None}
            for ph in order:
                r = REC.row(rec, ph)
                if why:
                    r['empty'] = why; continue
                rep = (j.get('phases') or {}).get(ph) or {}
                if rep.get('error'):
                    e = rep['error']
                    r['empty'] = (M.NO_SCROLL if 'whole wheel steps fit' in e
                                  else 'crashed: ' + e[:160]); continue
                r['wall_s'] = round((rep.get('wall_ms') or 0) / 1000, 2)
                m = M.OURS_MODE.get(ph, 'edit')
                fr = M.ours_frame(rep, m)
                r.update(time_ms=fr['req'], time_req_ms=fr['req'], time_raf_ms=fr['raf'], repeats=fr['samples'])
                r['cpu_ms'] = M.ours_cpu_cycles(rep, m, j.get('cpu_calibration'))
                if ph in ('Cvis', 'Dvis'):
                    bad = M.ours_vis_check(rep)
                    if bad:
                        r['note'] = bad; r['invalid'] = True
                q = (((rep.get('modes') or {}).get(m) or {}).get('start_quiet') or {})
                if q.get('quiet') is False:
                    r['note'] = (r.get('note', '') + f" page kept painting before the phase "
                                 f"({q.get('busy_windows')} busy quiet windows)").strip()[:200]
                if rep.get('check_error'):
                    r['note'] = (r.get('note', '') + ' check: ' + rep['check_error'])[:200]
            if not why:
                ma, note = M.ours_steady(j.get('memory_audit'))
                rec.setdefault('memory', {})['steady_bytes'] = ma
                if note:
                    rec['memory']['steady_note'] = note
                if ma is None:
                    rec['memory']['steady_empty'] = note or 'no memory audit in the plan report'
        rec['wall_s'] = time.perf_counter() - t0
        return rec

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        if self.e.get('plan'):  # one process per page and class: every phase in one plan
            return self._plan_measure(lane, page, cls, policy, phases, part)
        exe = self.e['exe'] if part == 'time' else self.e['counters_exe']
        rec = REC.new_record(self.name, self.version(), part, page, cls, self.definition(part) + '-' + policy.definition(), BOUNDS)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        to = self.e.get('timeout_s', {}).get(page.kind, self.e.get('timeout_s', {}).get('default', 120))
        quick = policy.min_repeats == 1
        est = None
        for ph in phases:
            r = REC.row(rec, ph)
            if ph == 'cold':
                if part == 'time':
                    args = [self._page(page, ph), '--mode', 'both', '--runs', '5', '--memory-settle-ms', '2000']
                else:
                    args = [self._page(page, ph), '--mode', 'cold', '--runs', '3']
                j, why, r['wall_s'] = self._run(exe, lane, page, cls, args, to)
                if why:
                    r['empty'] = why
                    if part == 'time':
                        rec['memory'] = {'empty': why}
                    continue
                if part == 'time':
                    fr = M.ours_frame(j, 'cold'); r.update(time_ms=fr['req'], time_req_ms=fr['req'], time_raf_ms=fr['raf'], repeats=fr['samples'])
                    est = fr['req']; pk, sd = M.ours_memory(j)
                    rec['memory'] = {'peak_bytes': pk, 'steady_bytes': sd, 'empty': None if pk else 'no memory in the report'}
                else:
                    r['cpu_ms'] = M.ours_cpu(j, 'cold'); r['empty'] = None if r['cpu_ms'] is not None else 'no CPU samples in the report'
                continue
            n = 3 if quick else self._runs(page, est)
            if ph == 'S':
                sh = page.meta.get('scrollHeight')
                if sh is not None:
                    fit = M.scroll_steps(page.meta)
                    if fit < 1:
                        r['empty'] = M.NO_SCROLL; continue
                    n = min(n, fit)
            mode = {'R': ['--mode', 'relayout'], 'S': ['--mode', 'scroll']}.get(ph) or ['--mode', 'edit', '--edit', EDITS[ph][1]]
            j, why, r['wall_s'] = self._run(exe, lane, page, cls, [self._page(page, ph)] + mode + ['--runs', str(n)], to)
            if why:
                r['empty'] = why; continue
            m = M.OURS_MODE.get(ph, 'edit')
            if part == 'time':
                fr = M.ours_frame(j, m)
                r.update(time_ms=fr['req'], time_req_ms=fr['req'], time_raf_ms=fr['raf'], repeats=fr['samples'])
                if ph in ('Cvis', 'Dvis'):
                    bad = M.ours_vis_check(j)
                    if bad:
                        r['note'] = bad; r['invalid'] = True
            else:
                r['cpu_ms'] = M.ours_cpu(j, m); r['empty'] = None if r['cpu_ms'] is not None else 'no CPU samples in the report'
        rec['wall_s'] = time.perf_counter() - t0
        return rec
