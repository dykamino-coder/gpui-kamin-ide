"""Kitesurf in an existing long-lived Linux container (only podman start/exec/cp/update; nothing is built or pulled).

The container holds the page set under ``root`` and a render adapter; one ``podman exec`` per page runs all repeats
(batch) and prints their results, so the container round trip is paid once per page.
cold = median of 3 of the engine's own cold_render clock (parse .. raster + PNG, from cx-ready.json); without it, the process
wall minus the same on an empty page; memory peak = peak RSS of the process tree, steady = the last RSS sample of the hold. The container cannot be
pinned to a lane: CPU quota by class, and the unit runs alone (``exclusive``).
Config: {"adapter": "kitesurf", "podman": exe, "container": name, "root": in-container page root,
         "manifest": in-container manifest, "blank": in-container empty page, "memprof": in-container sampler,
         "render": in-container adapter binary, "hold_ms": 2000, "cpus": {"gpu4": 4, ...}, "version": label}
"""
import json
import statistics
import subprocess
import threading
import time

from .. import metrics as M
from .. import records as REC

DEF = 'kitesurf-2'  # 2: cold = the engine's own cold_render clock
REPEATS = 3


class Kitesurf:
    parts = ('all',)
    exclusive = True

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e; self.lock = threading.Lock(); self.cpus = None

    def version(self):
        return self.e.get('version') or self.e['container']

    def definition(self, part):
        return DEF

    def needs_gpu(self, cls):
        return False

    def session(self, lane, cls):
        return None

    def close(self, lane):
        pass

    def _pod(self, *a, timeout=60):
        return subprocess.run([self.e['podman'], *map(str, a)], capture_output=True, text=True, timeout=timeout,
                              creationflags=0x08000000)

    def ready(self):
        """'' when the container runs without a network, else the reason."""
        try:
            r = self._pod('inspect', self.e['container'])
            info = json.loads(r.stdout)[0]
            if info['State']['Status'] != 'running':
                self._pod('start', self.e['container'])
                info = json.loads(self._pod('inspect', self.e['container']).stdout)[0]
            if info['State']['Status'] != 'running':
                return f'container {info["State"]["Status"]}'
            if info.get('NetworkSettings', {}).get('Networks'):
                return 'container has a network (refused: pages must stay offline)'
            return ''
        except (OSError, ValueError, IndexError, subprocess.TimeoutExpired) as e:
            return f'container unavailable: {e}'[:200]

    def _batch(self, page_id, html, reps):
        """All repeats in one exec; returns [(wall_ms, peak_rss, steady_rss) | None]."""
        rid = f'/tmp/ec-{page_id}-{time.time_ns()}'
        hold = int(self.e.get('hold_ms', 2000))
        run = (f'python3 -I {self.e["memprof"]} linux-process --out {rid}/$i --observe --ready-file {rid}/$i.ready.json '
               f'--duration 120 -- {self.e["render"]} {html} {rid}/$i/frame.png 800 600 {self.e["manifest"]} {page_id} {hold} '
               f'{rid}/$i/cx-ready.json')
        sh = (f'mkdir -p {rid}; for i in $(seq 1 {reps}); do echo "REP $i"; {run} | tail -n 1; '
              f'echo "SAMPLES $i"; tail -c 4000 {rid}/$i/samples.json 2>/dev/null | tr -d "\\n"; echo; '
              f'echo "READY $i"; head -c 20000 {rid}/$i/cx-ready.json 2>/dev/null | tr -d "\\n"; echo; done;rm -rf {rid}')
        try:
            r = self._pod('exec', '--user', '65534:65534', self.e['container'], 'sh', '-c', sh, timeout=60 + reps * 130)
        except subprocess.TimeoutExpired:
            return None, 'hangs: no result within the time limit'
        out, cur = {}, None
        for ln in r.stdout.splitlines():
            if ln.startswith('REP '):
                cur = int(ln[4:]); out[cur] = {}
            elif ln.startswith('SAMPLES ') and cur:
                out[cur]['samples_next'] = True
            elif ln.startswith('READY ') and cur:
                out[cur]['ready_next'] = True
            elif cur and out[cur].pop('samples_next', False):
                out[cur]['samples'] = ln
            elif cur and out[cur].pop('ready_next', False):
                try:
                    out[cur]['own'] = ((json.loads(ln) or {}).get('phases_ms') or {}).get('cold_render')
                except ValueError:
                    pass
            elif cur and ln.startswith('{'):
                try:
                    out[cur]['res'] = json.loads(ln)
                except ValueError:
                    pass
        res = []
        for i in range(1, reps + 1):
            x = out.get(i, {}).get('res')
            if not x or x.get('wall_ms') is None:
                res.append(None); continue
            steady = None
            s = out[i].get('samples') or ''
            k = s.rfind('"rss_bytes"')
            if k >= 0:
                try:
                    steady = int(s[k:].split(':', 1)[1].split(',')[0].split('}')[0])
                except ValueError:
                    steady = None
            res.append((x['wall_ms'], x.get('peak_rss_bytes'), steady, out[i].get('own')))
        err = (r.stderr.strip().splitlines() or [''])[-1][:160]
        return res, None if any(res) else ('crashed: ' + (err or f'exit {r.returncode}'))

    def _quota(self, cls):
        n = self.e.get('cpus', {}).get(cls)
        if n and self.cpus != n:
            self._pod('update', '--cpus', n, self.e['container']); self.cpus = n

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        bounds = ('Kitesurf: cold = the engine\'s own clock from parse to raster and PNG (cold_render) inside a Linux '
                  'container (CPU quota by class); median of 3; software raster; memory = Linux RSS of the process tree')
        rec = REC.new_record(self.name, self.version(), part, page, cls, f'{DEF}-{policy.definition()}', bounds)
        rec['lane'] = 'container'; t0 = time.perf_counter()
        reps = 1 if policy.min_repeats == 1 else REPEATS
        with self.lock:
            why = self.ready()
            if not why:
                self._quota(cls)
                blank = self._blank(cls)
                res, why = self._batch(page.id, f'{self.e["root"]}/{page.kind}/{page.id}/base.html', reps)
        cold = REC.row(rec, 'cold')
        if why:
            cold['empty'] = why; rec['memory'] = {'empty': why}
        elif not blank:
            cold['empty'] = 'fails on an empty page: no baseline'; rec['memory'] = {'empty': cold['empty']}
        else:
            ok = [x for x in res if x]
            cold['walls_ms'] = [x[0] for x in ok]; cold['blank_walls_ms'] = blank
            own = [x[3] for x in ok if x[3] is not None]
            if len(own) == len(ok):  # the engine's own clock: parse .. raster + PNG
                cold['repeats'] = own
            else:  # wall minus blank: the 2 s hold is in both, so the difference is often noise
                cold['repeats'] = [x[0] - statistics.median(blank) for x in ok]
                cold['note'] = 'no engine clock in the result: process wall minus an empty page'
            v = M.med(cold['repeats']) if len(ok) == reps else None
            if v is not None and v <= 0:
                cold['empty'] = 'within the noise of the empty-page baseline'; v = None
            cold['time_ms'] = v
            if v is None and not cold.get('empty'):
                cold['empty'] = 'failed on one of the repeats'
            rec['memory'] = {'peak_bytes': M.med([x[1] for x in ok]), 'steady_bytes': M.med([x[2] for x in ok])}
        for ph in phases:
            if ph != 'cold':
                REC.row(rec, ph)['empty'] = 'Kitesurf: the container adapter renders the first frame only'
        rec['wall_s'] = time.perf_counter() - t0
        return rec

    def _blank(self, cls):
        f = self.cfg.store / '_blank' / f'{self.name}-{self.version()}-{cls}.json'
        if f.exists():
            return json.loads(f.read_text(encoding='utf-8'))
        res, _ = self._batch('_blank', self.e['blank'], REPEATS)
        w = [x[0] for x in (res or []) if x]
        f.parent.mkdir(parents=True, exist_ok=True); f.write_text(json.dumps(w), encoding='utf-8')
        return w
