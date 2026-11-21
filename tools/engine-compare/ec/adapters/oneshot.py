"""One-shot engines (Blitz, Servo): the process loads the page, writes a snapshot and exits.

cold = median of 3 process walls (start -> snapshot written -> exit) minus the median of 3 walls on an empty page
       (process start-up and shutdown cancel out); memory peak = highest working set of the engine's processes;
       no steady memory and no warm phases (the process exits after its first frame).
Config: {"adapter": "oneshot", "exe": ..., "argv": [...], "env": {...}, "label": "Blitz"}
argv/env templates: {exe} {page} {page_uri} {out} {profile}.
"""
import json
import os
import shutil
import threading
import time
from pathlib import Path

from .. import metrics as M
from .. import records as REC
from .. import winproc as W
from ..config import file_version

DEF = 'oneshot-1'
BLANK = '<!doctype html><html><head><title>blank</title></head><body></body></html>'
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

    def _launch(self, lane, cls, page_file, out, timeout):
        out.mkdir(parents=True, exist_ok=True); prof = out / 'profile'; (prof / 'tmp').mkdir(parents=True, exist_ok=True)
        sub = {'exe': self.e['exe'], 'page': str(page_file), 'page_uri': Path(page_file).as_uri(), 'out': str(out), 'profile': str(prof)}
        argv = [a.format(**sub) for a in self.e['argv']]
        env = dict(os.environ, **{k: v.format(**sub) for k, v in self.e.get('env', {}).items()})
        p = W.Proc(argv, mask=lane.mask(cls), desk=self.cfg.desktop, env=env, cwd=out, stdout=False,
                   mem_cap=int(self.cfg.mem_cap_gb * (1 << 30)), stderr_path=out / 'stderr.txt')
        tree = W.Tree(p, period=0.05); tree.start()
        code = p.wait(timeout); wall = time.perf_counter() - p.t0
        if code is None:
            p.kill()
        peaks = tree.finish(); root = W.proc_mem(p.handle)
        pk = max([v for _, v in peaks.values() if v] + ([root.PeakWorkingSetSize] if root else []), default=None)
        p.close()
        err = (out / 'stderr.txt').read_text(encoding='utf-8', errors='replace') if (out / 'stderr.txt').exists() else ''
        shutil.rmtree(out, ignore_errors=True)
        return code, wall * 1000, pk, err

    def _blank(self, lane, cls):
        """Walls on the empty page, once per engine build x class x lane (cached in the store)."""
        f = self.cfg.store / '_blank' / f'{self.name}-{self.version()}-{cls}-L{lane.id}.json'
        with self.lock:
            if f.exists():
                return json.loads(f.read_text(encoding='utf-8'))
            page = lane.work / 'blank.html'; page.write_text(BLANK, encoding='utf-8')
            w = []
            for i in range(REPEATS):
                code, wall, _, _ = self._launch(lane, cls, page, lane.work / f'{self.name}-blank{i}', 90)
                if code == 0:
                    w.append(wall)
            f.parent.mkdir(parents=True, exist_ok=True); f.write_text(json.dumps(w), encoding='utf-8')
            return w

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        bounds = f'{self.label}: process start -> snapshot written and exit, minus the same on an empty page; median of 3'
        rec = REC.new_record(self.name, self.version(), part, page, cls, f'{DEF}-{policy.definition()}', bounds)
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); t0 = time.perf_counter()
        blank = self._blank(lane, cls)
        walls, peaks, why = [], [], None
        n = 1 if policy.min_repeats == 1 else REPEATS
        for i in range(max(n, 1) if blank else 0):
            code, wall, pk, err = self._launch(lane, cls, page.file('base.html'), lane.work / f'{self.name}-{page.id}-{i}', 90)
            if code is None:
                why = 'hangs: no snapshot within 90 s'; break
            if code != 0:
                from . import last_error
                why = f'crashed: exit code {code}: {last_error(err)}'; break
            walls.append(wall); peaks.append(pk)
        if not blank:
            why = 'fails on an empty page: no baseline'
        cold = REC.row(rec, 'cold')
        if walls and not why:
            cold['repeats'] = [w - M.med(blank) for w in walls]; cold['time_ms'] = M.med(cold['repeats'])
            cold['walls_ms'] = walls; cold['blank_walls_ms'] = blank
        else:
            cold['empty'] = why or 'no run'
        rec['memory'] = {'peak_bytes': M.med(peaks), 'steady_bytes': None,
                         'empty': why if not peaks else None, 'steady_empty': 'the process exits right after its first frame'}
        for ph in phases:
            if ph != 'cold':
                REC.row(rec, ph)['empty'] = f'{self.label}: one-shot process, first frame only (no edit or scroll without a script driver)'
        rec['wall_s'] = time.perf_counter() - t0
        return rec
