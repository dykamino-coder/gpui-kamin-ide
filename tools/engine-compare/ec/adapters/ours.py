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
BOUNDS_PLAN = ('ours: cold = document-cold samples in one process; warm phases = one process, each phase on its own '
               'input after its load, frame series of fixed duration; frame = from the frame request to pixels read back, '
               'minus the bench capture copy; CPU = cycle time of all threads of the process over the frame window minus the stand capture window (copy, wake after readback), as browsers: whole tree over the frame loop; steady memory once '
               'after the first warm phase')
DEF = 'ours-2'  # 2: CPU = all threads over the frame window minus the capture window


class Ours:
    parts = ('time', 'cpu')
    exclusive = False

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e
        # "plan": a bench with --plan measures time, CPU and memory in two processes (part "all");
        # otherwise the old one-mode-per-process bench and a separate counters build (parts "time" and "cpu").
        self.parts = ('cold', 'warm') if e.get('plan') else ('time', 'cpu')

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
        tmp = lane.work / f'ours-{lane.id}'; tmp.mkdir(parents=True, exist_ok=True)
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
        shutil.rmtree(tmp, ignore_errors=True)
        return j, None, wall

    def _runs(self, page, est_ms):
        """Samples per warm process: cheap pages get more (the process start dominates), heavy pages fewer."""
        if est_ms is None or est_ms < 100:
            return 8
        return 5 if est_ms < 400 else 3

    def _plan_measure(self, lane, page, cls, policy, phases, part):
        """Two processes: cold (document-cold samples, peak memory), then every warm phase in one plan process
        (steady memory once after the first phase, CPU per frame from process cycle time)."""
        rec = REC.new_record(self.name, self.version(), part, page, cls, self.definition(part) + '-' + policy.definition(), BOUNDS_PLAN)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        to = self.e.get('timeout_s', {}).get(page.kind, self.e.get('timeout_s', {}).get('default', 120))
        exe = self.e['exe']; series = self.e.get('series_ms', 500)
        if 'cold' in phases:
            r = REC.row(rec, 'cold')
            j, why, r['wall_s'] = self._run(exe, lane, page, cls, [self._page(page, 'cold'), '--mode', 'cold', '--runs', '5'], to)
            if why:
                r['empty'] = why; rec['memory'] = {'empty': why}
            else:
                fr = M.ours_frame(j, 'cold'); r.update(time_ms=fr['req'], time_req_ms=fr['req'], time_raf_ms=fr['raf'], repeats=fr['samples'])
                r['cpu_ms'] = M.ours_cpu_cycles(j, 'cold', j.get('cpu_calibration'))
                rec['memory'] = {'peak_bytes': j.get('peak_working_set_bytes'), 'steady_bytes': None}
        warm = [ph for ph in ('R', 'S', 'C', 'D', 'Cvis', 'Dvis') if ph in phases]
        plan = {'series_ms': series, 'min_runs': 3, 'memory_settle_ms': 2000 if warm and warm[0] == 'R' else 0, 'phases': []}
        for ph in warm:
            p = {'name': ph, 'input': str(self._page(page, ph)), 'runs': 8 if ph == 'S' else 30}
            if ph == 'R':
                p['mode'] = 'relayout'
            elif ph == 'S':
                p['mode'] = 'scroll'
            else:
                p['mode'] = 'edit'; p['edit'] = EDITS[ph][1]
            plan['phases'].append(p)
        if warm:
            pf = lane.work / f'plan-{lane.id}.json'
            import json
            pf.write_text(json.dumps(plan), encoding='utf-8')
            j, why, wall = self._run(exe, lane, page, cls, [self._page(page, warm[0]), '--plan', str(pf)], to * 2)
            rec['plan_wall_s'] = wall
            for ph in warm:
                r = REC.row(rec, ph)
                if why:
                    r['empty'] = why; continue
                rep = (j.get('phases') or {}).get(ph) or {}
                if rep.get('error'):
                    e = rep['error']
                    r['empty'] = ('page does not scroll by three whole 120 px steps' if 'whole wheel steps fit' in e
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
        if part in ('cold', 'warm'):  # two units the scheduler may put on different lanes
            ph = [p for p in phases if (p == 'cold') == (part == 'cold')]
            return self._plan_measure(lane, page, cls, policy, ph, part)
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
                    fit = (sh - 600) // 120 - 1
                    if fit < 3:
                        r['empty'] = 'page does not scroll by three whole 120 px steps'; continue
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
