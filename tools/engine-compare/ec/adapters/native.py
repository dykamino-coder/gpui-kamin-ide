"""Native engines driven by a plan harness (protocol ``native-plan-v1``: Blitz ``bz_plan``, Kitesurf ``cx_plan``).

One process per page and class runs every phase on the engine's own API, as one browser visit: cold (process creation
to the first CPU RGBA), then R (800 -> 640 -> 800), C/D/Cvis/Dvis (the same edits as ours and the browsers, through the
engine's DOM API) and S (120 px steps, the browsers' step count). Every warm phase is a series of at least 3 frames
and 500 ms; frame = native request -> CPU RGBA (PNG and edit evidence outside the series).
CPU per frame = process CPU (user + kernel, every thread) over the series / frames, read by the harness around the
series (cycle time as an extra field); cold CPU = the process from its creation to the first RGBA.
More cold samples: cold-only processes (no phases, no hold) until the policy's repeats.
Memory: peak = highest working set of the process; steady = median working set while the engine holds the page after
its READY file (``skip_ms`` after it).
A phase the engine cannot run carries the harness's named reason (status unsupported / incomplete), never a zero.
Config: {"adapter": "native", "exe", "label", "env": {...}, "manifest": resource manifest of the page set,
         "hold_ms": 2000, "skip_ms": 500, "timeout_s": 180, "series_ms": 500, "version": optional label}
"""
import json
import os
import re
import shutil
import threading
import time

from .. import metrics as M
from .. import records as REC
from .. import winproc as W
from ..config import file_version
from . import last_error
from .ours import edit_input

DEF = 'native-1'
ORDER = ('R', 'C', 'D', 'Cvis', 'Dvis', 'S')
SECRET = re.compile(r'TOKEN|SECRET|PASSWORD|API_?KEY|CREDENTIAL|AUTH', re.I)


def phase_plan(page, ph, steps=None):
    """Plan entry of one phase: the same selectors, attributes and inputs as ours and the browsers."""
    if ph == 'R':
        return {'name': 'R', 'mode': 'resize', 'widths': [640, 800]}
    inp = str(edit_input(page, ph))
    if ph == 'S':
        p = {'name': 'S', 'mode': 'scroll', 'input': inp, 'scroll_step': M.SCROLL_STEP_PX}
        if steps:
            p['steps'] = steps
        return p
    sel, attr, val = ('html', 'class', 'alt') if ph in ('C', 'Cvis') else ('@middle', 'data-edit', '1')
    p = {'name': ph, 'mode': 'edit', 'input': inp, 'selector': sel, 'attribute': attr, 'value': val,
         'visibility': 'visible' if ph.endswith('vis') else 'offscreen'}
    if ph == 'C':
        p['observe'] = '[data-deep]'
    elif ph.endswith('vis'):
        p['observe'] = 'b4-e'
    return p


def reason(label, text):
    """Named refusal of the harness without local absolute paths."""
    return f'{label}: ' + re.sub(r'[A-Za-z]:[\\/]\S+', '<file>', text or 'no reason given')[:300]


def fill(rec, rep, label, want_steps=None):
    """Rows of one harness report (warm phases); returns the cold sample (time_ms, cpu_ms) or None."""
    cold = None
    for p in rep.get('phases') or []:
        name = p.get('name'); frames = [x for x in (p.get('frames_ms') or []) if x is not None]
        cpu = p.get('cpu') or {}
        if name == 'cold':
            cold = (frames[0] if frames else None, cpu.get('cpu_ms'))
            continue
        r = REC.row(rec, name)
        if p.get('status') != 'ok' or not frames:
            r['empty'] = reason(label, p.get('error')); continue
        r.update(time_ms=M.med(frames), time_req_ms=M.med(frames), repeats=frames,
                 wall_s=round((p.get('series_elapsed_ms') or 0) / 1000, 2))
        if cpu.get('cpu_ms') is not None:
            r['cpu_ms'] = cpu['cpu_ms'] / len(frames); r['cpu_cycles'] = (cpu.get('cycles') or 0) / len(frames)
        else:
            r['cpu_empty'] = f'{label}: the harness build reports no per-phase CPU'
        if name == 'S' and want_steps and len(frames) != want_steps:
            r['note'] = f'{len(frames)} of the scenario\'s {want_steps} steps: the engine\'s document ends earlier'
    return cold


class Native:
    parts = ('all',)
    exclusive = False

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e; self.lock = threading.Lock(); self.seq = 0
        self.label = e.get('label', name.capitalize())

    def version(self):
        return self.e.get('version') or file_version(self.e['exe'])

    def definition(self, part):
        return DEF

    def needs_gpu(self, cls):
        return False

    def session(self, lane, cls):
        return None

    def close(self, lane):
        pass

    def _env(self):
        env = {k: v for k, v in os.environ.items() if not SECRET.search(k)}  # no credentials of the tool's shell
        env.update({k: str(v) for k, v in (self.e.get('env') or {}).items()})
        return env

    def launch(self, lane, cls, page, phases, hold):
        """One harness process; (report | None, reason | None, peak_bytes, steady_bytes)."""
        with self.lock:
            self.seq += 1; n = self.seq
        work = lane.work / f'{self.name}-{lane.id}-{os.getpid()}-{n}'
        shutil.rmtree(work, ignore_errors=True); work.mkdir(parents=True)
        plan = {'input': str(page.file('base.html')), 'page_id': page.id, 'width': 800, 'height': 600,
                'series_ms': int(self.e.get('series_ms', 500)), 'min_runs': 3, 'phases': phases}
        if self.e.get('manifest'):
            plan['manifest'] = self.e['manifest']
        pf = work / 'plan.json'; pf.write_text(json.dumps(plan), encoding='utf-8')
        ready = work / 'READY.json'; err = work / 'stderr.txt'
        to = float(self.e.get('timeout_s', 180))
        p = W.Proc([self.e['exe'], str(pf), str(work / 'out'), str(int(hold)), str(ready)], mask=lane.mask(cls),
                   desk=self.cfg.desktop, env=self._env(), cwd=work, stdout=True,
                   mem_cap=int(self.cfg.mem_cap_gb * (1 << 30)), stderr_path=err)
        tree = W.Tree(p, period=0.05); tree.start(); so = []
        rd = threading.Thread(target=lambda: so.append(p.stdout.read()), daemon=True); rd.start()
        code = p.wait(to); ended = time.perf_counter()
        # the hold starts when READY is written: its mtime on the perf_counter clock (a polling watcher thread was
        # late under load and lost the window)
        seen = [ready.stat().st_mtime - (time.time() - time.perf_counter())] if ready.exists() else []
        if code is None:
            p.kill()
        rd.join(5)
        peaks = tree.finish(); root = W.proc_mem(p.handle)
        peak = max([v for _, v in peaks.values() if v] + ([root.PeakWorkingSetSize] if root else []), default=None)
        steady = None
        if seen and hold:
            a = seen[0] + float(self.e.get('skip_ms', 500)) / 1000
            steady = M.med([s[2] for s in tree.samples if a <= s[0] < ended - 0.05 and s[2]])
        p.close()
        rep, why = None, None
        try:
            rep = json.loads(ready.read_text(encoding='utf-8'))
        except (OSError, ValueError):
            text = (so[0] if so else b'').decode('utf-8', 'replace').strip().splitlines()
            try:
                rep = json.loads(text[-1]) if text else None
            except ValueError:
                rep = None
        if code is None:
            why = f'timeout: no result within {to:.0f} s'
        elif rep is None or rep.get('protocol') != 'native-plan-v1':
            msg = err.read_text(encoding='utf-8', errors='replace') if err.exists() else ''
            why = f'crashed: exit code {code}: {last_error(msg)}'; rep = None
        if not self.e.get('keep_output'):
            shutil.rmtree(work, ignore_errors=True)
        return rep, why, peak, steady

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        bounds = (f'{self.label}: one native harness process per page and class runs every phase on the engine API '
                  '(cold = process creation to the first CPU RGBA; warm series >= 3 frames and 500 ms; frame = native '
                  'request to CPU RGBA, PNG excluded); CPU per frame = process user + kernel time over the series / '
                  'frames; cold CPU = process creation to the first RGBA; extra cold samples from cold-only processes; '
                  'software raster; memory peak = the process working set, steady = median working set while it holds '
                  'the page after READY')
        rec = REC.new_record(self.name, self.version(), part, page, cls, f'{DEF}-{policy.definition()}', bounds)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        order = [ph for ph in ORDER if ph in phases]
        steps = M.scroll_steps(page.meta)
        if 'S' in order and steps == 0:  # the same scenario as every engine: nothing to scroll
            REC.row(rec, 'S')['empty'] = M.NO_SCROLL; order.remove('S')
        rep, why, peak, steady = self.launch(lane, cls, page, [phase_plan(page, ph, steps) for ph in order],
                                             self.e.get('hold_ms', 2000))
        cold = REC.row(rec, 'cold')
        if why:
            cold['empty'] = why; rec['memory'] = {'empty': why}
            for ph in order:
                REC.row(rec, ph)['empty'] = why
            rec['wall_s'] = time.perf_counter() - t0
            return rec
        samples = [fill(rec, rep, self.label, steps)]
        for ph in order:
            if not (rec['rows'].get(ph) or {}).get('time_ms') and not rec['rows'].get(ph, {}).get('empty'):
                REC.row(rec, ph)['empty'] = f'{self.label}: phase missing from the harness report'
        while len(samples) < policy.want([s[0] if s else None for s in samples]):
            r2, why2, _, _ = self.launch(lane, cls, page, [], 0)
            if why2:
                cold['note'] = f'extra cold sample failed: {why2}'[:200]; break
            samples.append(fill(rec, r2, self.label))
        t = [s[0] for s in samples if s and s[0] is not None]
        c = [s[1] for s in samples if s and s[1] is not None]
        if t:
            cold.update(time_ms=M.med(t), time_req_ms=M.med(t), repeats=t)
            cold['cpu_ms'] = M.med(c)
            if cold['cpu_ms'] is None:
                cold['cpu_empty'] = f'{self.label}: the harness build reports no cold CPU'
        else:
            cold['empty'] = f'{self.label}: no cold frame in the harness report'
        rec['memory'] = {'peak_bytes': peak, 'steady_bytes': steady, 'empty': None if peak else 'no working set sampled'}
        if steady is None:
            rec['memory']['steady_empty'] = 'the engine exited before its hold window was sampled'
        if rep.get('complete') is False:
            rec['note'] = f'{self.label}: partial result (named refusals in the phase rows)'
        rec['wall_s'] = time.perf_counter() - t0
        return rec
