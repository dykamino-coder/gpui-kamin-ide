"""One-shot engines (Blitz, Servo): the process loads the page, writes a snapshot and exits.

cold = median of 3 process walls (start -> snapshot written -> exit) minus the median of 3 walls on an empty page
       (process start-up and shutdown cancel out), or the engine's own count (self_timing);
cold CPU = CPU of the whole process tree over its life (cycle time) minus the same on the empty page;
memory peak = highest working set of the engine's processes; steady = median working set of the tree from skip_ms after
       the engine's ready file until it exits (config steady: {ready_file, skip_ms}, the engine holds the page); no warm
       phases (the process exits after its first frame).
Config: {"adapter": "oneshot", "exe": ..., "argv": [...], "env": {...}, "label": "Blitz"}
argv/env templates: {exe} {page} {page_uri} {page_id} {out} {profile}; "self_timing": {prefix, field (dotted path)};
"clean_env": true drops credential-like variables from the engine's environment.
"""
import json
import os
import re
import shutil
import threading
import time
from pathlib import Path

from .. import metrics as M
from .. import records as REC
from .. import winproc as W
from ..config import file_version

DEF = 'oneshot-3'  # 2: cold CPU = tree CPU minus the empty page; steady memory in the hold window; 3: private peak
from . import BLANK
REPEATS = 3


class OneShot:
    parts = ('all',)
    exclusive = False

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e; self.lock = threading.Lock()
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

    def _launch(self, lane, cls, page_file, out, timeout, page_id='_blank'):
        out.mkdir(parents=True, exist_ok=True); prof = out / 'profile'; (prof / 'tmp').mkdir(parents=True, exist_ok=True)
        sub = {'exe': self.e['exe'], 'page': str(page_file), 'page_uri': Path(page_file).as_uri(), 'out': str(out), 'profile': str(prof),
               'page_id': page_id}
        argv = [a.format(**sub) for a in self.e['argv']]
        env = dict(os.environ, **{k: v.format(**sub) for k, v in self.e.get('env', {}).items()})
        if self.e.get('clean_env'):  # the engine gets no credentials of the shell that runs the tool
            env = {k: v for k, v in env.items() if not re.search(r'TOKEN|SECRET|PASSWORD|API_?KEY|CREDENTIAL|AUTH', k, re.I)}
        p = W.Proc(argv, mask=lane.mask(cls), desk=self.cfg.desktop, env=env, cwd=out, stdout=True,
                   mem_cap=int(self.cfg.mem_cap_gb * (1 << 30)), stderr_path=out / 'stderr.txt')
        tree = W.Tree(p, period=0.05); tree.start(); so = []
        rd = threading.Thread(target=lambda: so.append(p.stdout.read()), daemon=True); rd.start()
        ready = self._watch_ready(sub, p)
        code = p.wait(timeout); wall = time.perf_counter() - p.t0; ended = time.perf_counter()
        rd.join(5)
        last = tree.snap()  # exited processes keep their final CPU: the whole life of the tree
        cpu = {'cpu_s': last[1], 'cycles': last[4]}
        steady = self._steady(tree.samples, ready, ended)
        self_ms = self._self_timing((so[0] if so else b'').decode('utf-8', 'replace'))
        if code is None:
            p.kill()
        peaks = tree.finish(); root = W.proc_mem(p.handle)
        pk = max([v for _, v in peaks.values() if v] + ([root.PeakWorkingSetSize] if root else []), default=None)
        # private peak: the highest tree sample, or the root's own lifetime peak commit when the process lived shorter
        # than a sampling period (a one-shot engine on a small page exits within tens of milliseconds)
        pv = max([s[3] for s in tree.samples if s[3]] + ([root.PeakPagefileUsage] if root else []), default=None)
        p.close()
        err = (out / 'stderr.txt').read_text(encoding='utf-8', errors='replace') if (out / 'stderr.txt').exists() else ''
        shutil.rmtree(out, ignore_errors=True)
        return code, wall * 1000, (pk, pv), err, self_ms, cpu, steady

    def _watch_ready(self, sub, p):
        """Time the engine's ready file appears (config steady.ready_file), [t] filled by a watcher thread."""
        st = self.e.get('steady'); box = []
        if not st:
            return box
        f = Path(st['ready_file'].format(**sub))

        def watch():
            while p.poll() is None and not box:
                if f.exists():
                    box.append(time.perf_counter()); return
                time.sleep(0.02)
        threading.Thread(target=watch, daemon=True).start()
        return box

    def _steady(self, samples, ready, ended):
        """Median working set of the tree while the engine holds the page after its ready file (skip_ms after it)."""
        st = self.e.get('steady')
        if not st or not ready:
            return None
        a = ready[0] + st.get('skip_ms', 500) / 1000
        return M.med([s[2] for s in samples if a <= s[0] < ended - 0.05 and s[2]])

    def _self_timing(self, text):
        """The engine's own count of the first frame, when it prints one (config `self_timing`: {prefix, field})."""
        st = self.e.get('self_timing')
        if not st:
            return None
        for ln in text.splitlines():
            if ln.startswith(st['prefix']):
                try:
                    v = json.loads(ln if st['prefix'] == '{' else ln[len(st['prefix']):])  # '{': the line is the JSON
                    for k in st['field'].split('.'):  # dotted path into nested results
                        v = v[k]
                    return float(v)
                except (ValueError, KeyError, TypeError):
                    return None
        return None

    def _blank(self, lane, cls):
        """Walls on the empty page, once per engine build x class x lane (cached in the store)."""
        f = self.cfg.store / '_blank' / f'{self.name}-{self.version()}-{cls}-cpu{lane.base}-v4.json'
        with self.lock:
            if f.exists():
                return json.loads(f.read_text(encoding='utf-8'))
            page = lane.work / 'blank.html'; page.write_text(BLANK, encoding='utf-8')
            self._launch(lane, cls, page, lane.work / f'{self.name}-warmup', 90)  # first start pays disk and caches
            w = {'walls': [], 'cpu': [], 'peaks': [], 'private_peaks': [], 'steadies': []}
            for i in range(REPEATS):
                code, wall, (pk, pv), _, _, cpu, sd = self._launch(lane, cls, page, lane.work / f'{self.name}-blank{i}', 90)
                if code == 0:
                    w['walls'].append(wall); w['cpu'].append(cpu)
                    w['peaks'].append(pk); w['private_peaks'].append(pv); w['steadies'].append(sd)
            f.parent.mkdir(parents=True, exist_ok=True); f.write_text(json.dumps(w), encoding='utf-8')
            return w

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        bounds = (f'{self.label}: process start -> snapshot written and exit, minus the same on an empty page; median of 3; '
                  'cold CPU = the process tree over its life minus the empty page'
                  + ('; steady memory = median working set while the engine holds the page after its ready file'
                     if self.e.get('steady') else ''))
        rec = REC.new_record(self.name, self.version(), part, page, cls, f'{DEF}-{policy.definition()}', bounds)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        b = self._blank(lane, cls); blank = b['walls']
        walls, peaks, pvs, own, cpus, steadies, why = [], [], [], [], [], [], None
        n = REPEATS  # cheap; the blank subtraction needs a median on both sides even in quick
        for i in range(max(n, 1) if blank else 0):
            code, wall, (pk, pv), err, sm, cpu, sd = self._launch(lane, cls, page.file('base.html'), lane.work / f'{self.name}-{page.id}-{i}', 90, page.id)
            if code is None:
                why = 'hangs: no snapshot within 90 s'; break
            if code != 0:
                from . import last_error
                why = f'crashed: exit code {code}: {last_error(err)}'; break
            walls.append(wall); peaks.append(pk); pvs.append(pv); own.append(sm); cpus.append(cpu); steadies.append(sd)
        if not blank:
            why = 'fails on an empty page: no baseline'
        cold = REC.row(rec, 'cold')
        if walls and not why and all(x is not None for x in own):
            # the engine's own count from its start to the raster buffer: no process start-up noise in it
            cold['repeats'] = own; cold['time_ms'] = M.med(own); cold['walls_ms'] = walls
            cold['note'] = f'{self.label} own count: ' + self.e['self_timing']['field']
        elif walls and not why:
            cold['repeats'] = [w - M.med(blank) for w in walls]; cold['time_ms'] = M.med(cold['repeats'])
            cold['walls_ms'] = walls; cold['blank_walls_ms'] = blank
            noise = (max(blank) - min(blank)) + (max(walls) - min(walls))
            if cold['time_ms'] is not None and cold['time_ms'] <= noise:
                cold['within_noise'] = True
                cold['note'] = f'within the noise of the bounds (start-up spread {noise:.0f} ms): not ranked'
            if cold['time_ms'] is not None and cold['time_ms'] <= 0:
                cold['time_ms'] = None; cold['empty'] = 'the empty page took as long as the page: start-up noise exceeds the page'
        else:
            cold['empty'] = why or 'no run'
        if not why and cpus:
            cold['cpu_ms'] = M.oneshot_cpu_ms(cpus, b['cpu'])
            if cold['cpu_ms'] is None:
                cold['cpu_empty'] = 'process CPU of the page did not exceed the empty page'
        steady = M.med(steadies) if not why else None
        rec['memory'] = {'peak_bytes': M.med(peaks), 'steady_bytes': steady, 'empty': why if not peaks else None,
                         'peak_private_bytes': M.med([x for x in pvs if x]), 'steady_private_bytes': None,
                         'blank': {'peak_bytes': M.med([x for x in b.get('peaks', []) if x]), 'steady_bytes': M.med([x for x in b.get('steadies', []) if x]),
                                   'peak_private_bytes': M.med([x for x in b.get('private_peaks', []) if x]), 'steady_private_bytes': None}}
        if steady is None:
            rec['memory']['steady_empty'] = why or (
                'the engine did not write its ready file' if self.e.get('steady') else
                'the process exits right after its first frame (no hold mode in this engine)')
        for ph in phases:
            if ph != 'cold':
                REC.row(rec, ph)['empty'] = f'{self.label}: one-shot process, first frame only (no edit or scroll without a script driver)'
        rec['wall_s'] = time.perf_counter() - t0
        return rec
