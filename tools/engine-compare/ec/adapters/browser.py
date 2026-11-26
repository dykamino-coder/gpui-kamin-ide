"""Chrome, Edge, Firefox through driver/browser-driver.cjs: one long-lived browser per lane and class, a fresh
context (Chromium) or tab (Firefox) per page, every warm phase in the same page visit.

Visit 1: load -> steady window (memory) -> probe of all phases -> CPU loops of fixed frame count (CPU per frame).
Visits 2..N: load -> probe only, for timing repeats; later visits probe only the rows that still need repeats
(noisy, or faster than the fast-row threshold).
Config: {"adapter": "browser", "browser": "chrome"|"edge"|"firefox"|"servo", "exe": ..., "geckodriver": ... (Firefox),
         "gpu": false when the engine rasters on the CPU in every class (Servo headless)}
"""
import json
import shutil
import threading
import time
from pathlib import Path

from .. import metrics as M
from .. import records as REC
from .. import winproc as W
from ..config import CLASSES, file_version

DEF = 'browser-4'  # 4: classify absent S before startup; 3: adaptive repeat visits (adapters.Policy); 2: probe-ec.html (stuck scroll steps skip the MozAfterPaint guard), Servo screenshots
# The probe's MozAfterPaint guard waits 250 ms for a paint; a scroll step that did not move the scroller paints nothing,
# so its miss is known at once (wrap() recorded it in c.ok). Only that wait changes; every measured frame is the same.
GUARD = 'setTimeout(function(){if(a===my){a=0;miss++;c.ok.pop();next();}},250);'
GUARD_EC = ('setTimeout(function(){if(a===my){a=0;miss++;c.ok.pop();next();}},'
            '(c.ok&&c.ok.length&&!c.ok[c.ok.length-1])?0:250);')


def probe_file(page):
    """The page's probe with the stuck-step guard (derived next to it, like C.html); the original when it differs."""
    src = page.file('probe.html'); text = src.read_text(encoding='utf-8')
    if GUARD not in text:
        return src
    out = page.file('probe-ec.html')
    new = text.replace(GUARD, GUARD_EC)
    if not out.exists() or out.read_text(encoding='utf-8') != new:
        out.write_text(new, encoding='utf-8', newline='')
    return out
BOUNDS = {'chromium': ('headless browser, page in a fresh context of a warm browser; frame = better of two probe clocks plus '
                       'raster and swap from the trace of the same window; memory = working-set sum of all browser processes'),
          'firefox': ('headless browser, page in a fresh tab of a warm browser; frame = better of MozAfterPaint and the request '
                      'clock; memory = working-set sum of all browser processes'),
          'servo': ('headless servoshell driven through its own WebDriver server, the page in the one window of a warm '
                    'browser; frame = request -> pixels: the edit (one probe frame) then a WebDriver screenshot, which '
                    'Servo answers only when the rendering is up to date with no pending frame, minus the same without an '
                    'edit (transport and PNG encode); its probe clocks end before its asynchronous layout and paint, so they are '
                    'not used; memory = working set of the '
                    'servoshell process; headless servoshell rasters on the CPU (ANGLE on D3D11 WARP, no GPU driver loaded), so '
                    'gpu2 is measured once under soft2')}
DRIVER = Path(__file__).resolve().parents[2] / 'driver' / 'browser-driver.cjs'


class Session:
    def __init__(self, ad, lane, cls):
        self.lane = lane; self.cls = cls; b = ad.e.get('browser', ad.name)
        wd = lane.work / f'{ad.name}-{cls}-L{lane.id}'
        shutil.rmtree(wd, ignore_errors=True); wd.mkdir(parents=True, exist_ok=True); self.wd = wd
        dc = {'exe': ad.e['exe'], 'cls': CLASSES[cls][1], 'workDir': str(wd), 'playwright': ad.cfg.playwright,
              'selenium': ad.cfg.selenium, 'geckodriver': ad.e.get('geckodriver')}
        (wd / 'driver.json').write_text(json.dumps(dc), encoding='utf-8')
        self.p = W.Proc([ad.cfg.node, str(DRIVER), b, str(wd / 'driver.json')], mask=lane.mask(cls), desk=ad.cfg.desktop,
                        stdin=True, mem_cap=int(ad.cfg.mem_cap_gb * (1 << 30)), stderr_path=wd / 'driver.err', cwd=wd)
        self.tree = W.Tree(self.p); self.tree.start()
        kind, data = self._read(120)
        if kind != 'READY' or data.get('error'):
            raise RuntimeError(f'browser did not start: {(data or {}).get("error", kind)}'[:300])
        self.info = data

    def _read(self, timeout):
        """Next READY/RESULT line; MARK lines are answered on the way (snapshot, then ACK)."""
        res = []
        def rd():
            while True:
                ln = self.p.stdout.readline()
                if not ln:
                    res.append(('EOF', {})); return
                s = ln.decode('utf-8', 'replace').rstrip('\r\n')
                if s.startswith('MARK '):
                    self.marks[s[5:].split()[0]] = self.tree.mark(s[5:]); self.p.stdin.write(b'ACK\n'); continue
                k, _, rest = s.partition(' ')
                if k in ('READY', 'RESULT'):
                    try:
                        res.append((k, json.loads(rest) if rest else {}))
                    except ValueError:
                        res.append((k, {'status': 'error', 'error': 'bad driver output'}))
                    return
        self.marks = getattr(self, 'marks', {})
        t = threading.Thread(target=rd, daemon=True); t.start(); t.join(timeout)
        return res[0] if res else ('TIMEOUT', {})

    def visit(self, op, timeout):
        self.marks = {}; t0 = time.perf_counter()
        self.p.stdin.write((json.dumps(dict(op, op='page')) + '\n').encode())
        kind, res = self._read(timeout)
        t1 = time.perf_counter()
        if kind == 'TIMEOUT':
            return None, f'hangs: page did not finish within {timeout} s', None
        if kind != 'RESULT':
            return None, 'browser exited during the page', None
        samples = [s for s in self.tree.samples if t0 <= s[0] <= t1]
        if isinstance(res, dict):
            res['visitWallMs'] = (t1 - t0) * 1000  # controller side: the driver's page() plus marks and transport
        return res, None, (dict(self.marks), samples)

    def close(self):
        try:
            self.p.stdin.write(b'{"op":"quit"}\n'); self.p.wait(10)
        except OSError:
            pass
        self.tree.finish(); self.p.close()
        shutil.rmtree(self.wd, ignore_errors=True)


class Browser:
    parts = ('all',)
    exclusive = False

    def __init__(self, name, cfg, e):
        self.name = name; self.cfg = cfg; self.e = e; self.sessions = {}; self._ver = None

    def version(self):
        return self.e.get('version') or file_version(self.e['exe'])

    def definition(self, part):
        return DEF

    def needs_gpu(self, cls):
        return self.e.get('gpu', True) and CLASSES[cls][1] == 'gpu'

    def session(self, lane, cls):
        return (self.name, cls)

    def close(self, lane):
        s = self.sessions.pop(lane.id, None)
        if s:
            s.close()

    def _session(self, lane, cls):
        s = self.sessions.get(lane.id)
        if s and (s.cls != cls or s.p.poll() is not None):
            self.close(lane); s = None
        if not s:
            s = self.sessions[lane.id] = Session(self, lane, cls)
        return s

    def measure(self, lane, page, cls, part, policy, phases=None):
        phases = phases or M.PHASES
        b = self.e.get('browser', self.name)
        fam = b if b in ('firefox', 'servo') else 'chromium'
        rec = REC.new_record(self.name, self.version(), part, page, cls, f'{DEF}-{policy.definition()}', BOUNDS[fam])
        rec['lane'] = lane.id; rec['mask'] = format(lane.mask(cls), 'X'); rec['family'] = fam; t0 = time.perf_counter()
        if 'S' in phases and M.scroll_steps(page.meta) == 0:
            REC.row(rec, 'S')['empty'] = M.NO_SCROLL
            phases = [p for p in phases if p != 'S']
            if not phases:
                rec['wall_s'] = time.perf_counter() - t0
                return rec
        to = 420 if page.kind != 'syn' else 300
        try:
            s = self._session(lane, cls)
        except (RuntimeError, OSError) as e:
            rec['empty'] = str(e)
            for ph in phases:
                REC.row(rec, ph)['empty'] = str(e)
            rec['memory'] = {'empty': str(e)}
            return rec
        rec['browser_version'] = s.info.get('version')
        warm = [p for p in phases if p != 'cold']
        probe = str(probe_file(page))
        op = {'probe': probe, 'cases': ','.join(warm), 'steadyMs': policy.steady_ms,
              'cpuCases': warm, 'cpuFrames': policy.cpu_frames, 'trace': fam == 'chromium'}
        res, why, extra = s.visit(op, to)
        if res is None or res.get('status') != 'ok':
            why = why or 'did not open: ' + ' '.join(str((res or {}).get('error', '?')).split())[:160]
            for ph in phases:
                REC.row(rec, ph)['empty'] = why
            rec['memory'] = {'empty': why}; self.close(lane)
            rec['wall_s'] = time.perf_counter() - t0
            return rec
        marks, samples = extra
        pk, sd = M.tree_memory(samples, marks)
        rec['memory'] = {'peak_bytes': pk, 'steady_bytes': sd, 'empty': None if pk else 'no memory samples'}
        first = res
        frames = {c: (v or {}).get('frames') for c, v in (res.get('cpu') or {}).items()}
        last = s.tree.snap()  # calibration: the warm browser's cycles per CPU millisecond so far
        per_ms = last[4] / (last[1] * 1000) if last[1] > 1 else None
        cpu = M.cpu_windows(marks, frames, per_ms)
        self._add(rec, res, phases)
        for ph in phases:
            if ph in cpu:
                REC.row(rec, ph)['cpu_ms'] = cpu[ph]
        visits = 1
        while visits < policy.max_repeats:
            need = [ph for ph in phases if len(REC.row(rec, ph)['repeats']) < policy.want(REC.row(rec, ph)['repeats'])]
            if not need:
                break
            w = [p for p in need if p != 'cold']
            op = {'probe': probe, 'cases': ','.join(w), 'steadyMs': 0, 'cpuCases': [],
                  'cpuFrames': policy.cpu_frames, 'trace': fam == 'chromium'}
            res, why, _ = s.visit(op, to)
            visits += 1
            if res is None or res.get('status') != 'ok':
                rec.setdefault('notes', []).append(f'repeat {visits} failed: {why or res.get("error", "?")}'[:200])
                if res is None:
                    self.close(lane)
                break
            self._add(rec, res, ['cold'] + w if 'cold' in need else w)
        for ph in phases:
            r = REC.row(rec, ph); r['time_ms'] = M.med(r['repeats'])
            if r['time_ms'] is None and not r.get('empty'):
                r['empty'] = (M.scroll_empty_reason(first) if ph == 'S' else None) or 'no frames of this phase in the probe result'
        rec['visits'] = visits; rec['wall_s'] = time.perf_counter() - t0
        return rec

    @staticmethod
    def _add(rec, res, phases):
        rec.setdefault('visit_walls_ms', []).append({k: round(res[k], 1) for k in ('visitWallMs', 'pageWallMs', 'navWallMs', 'probeWallMs', 'shotWallMs', 'traceWallMs', 'cpuWallMs')
                                                     if isinstance(res.get(k), (int, float))})
        cases = (res.get('probe') or {}).get('cases') or {}
        missed = {k: (c.get('afterPaint') or {}).get('missed') for k, c in cases.items() if c.get('afterPaint')}
        if missed:  # Firefox: frames whose MozAfterPaint did not come within the probe's guard
            rec.setdefault('paint_missed', []).append(missed)
        v = M.browser_visit(rec['family'], res)
        for ph in phases:
            x = v.get(ph)
            if x and x['value'] is not None:
                REC.row(rec, ph)['repeats'].append(x['value'])
