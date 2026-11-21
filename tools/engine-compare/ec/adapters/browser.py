"""Chrome, Edge, Firefox through driver/browser-driver.cjs: one long-lived browser per lane and class, a fresh
context (Chromium) or tab (Firefox) per page, every warm phase in the same page visit.

Visit 1: load -> steady window (memory) -> one probe round of every warm row -> CPU loops of fixed frame count (CPU
per frame); the page stays open and only the rows that did not settle get more probe rounds in it (every frame of a
round is a sample of the row). Cold: one fresh load per sample. A row settles when the 90 % confidence interval of its
median (order statistics) is within 3 % of the median, or lies wholly beyond our value of the same row by 3 % (its
place is decided); the cap is 8 rounds (8 loads for cold), a row stopped there is marked ``capped``.
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

DEF = 'browser-4'  # 4: later rounds reuse round 1's raster per frame; warm rows = frames of in-page probe rounds, stop on the median's interval (Policy.settled); 3: adaptive repeat visits (adapters.Policy); 2: probe-ec.html (stuck scroll steps skip the MozAfterPaint guard), Servo screenshots
# The probe's MozAfterPaint guard waits 250 ms for a paint; a scroll step that did not move the scroller paints nothing,
# so its miss is known at once (wrap() recorded it in c.ok). Only that wait changes; every measured frame is the same.
GUARD = 'setTimeout(function(){if(a===my){a=0;miss++;c.ok.pop();next();}},250);'
GUARD_EC = ('setTimeout(function(){if(a===my){a=0;miss++;c.ok.pop();next();}},'
            '(c.ok&&c.ok.length&&!c.ok[c.ok.length-1])?0:250);')


# Later probe rounds need only the frame clocks: the style / layout loops (no metric uses them) are skipped there.
SYNC = 'function sync(c,flush){if(c.nosync)return null;'
SYNC_EC = 'function sync(c,flush){if(c.nosync||window.__b4framesOnly)return null;'


def probe_file(page):
    """The page's probe with the stuck-step guard and the frames-only switch (derived next to it, like C.html); the
    original when neither applies."""
    src = page.file('probe.html'); text = src.read_text(encoding='utf-8')
    if GUARD not in text and SYNC not in text:
        return src
    out = page.file('probe-ec.html')
    new = text.replace(GUARD, GUARD_EC).replace(SYNC, SYNC_EC)
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

    def more(self, op, timeout):
        """One more probe round in the held page: (result | None, reason)."""
        t0 = time.perf_counter()
        self.p.stdin.write((json.dumps(dict(op, op='more')) + '\n').encode())
        kind, res = self._read(timeout)
        if kind == 'TIMEOUT':
            return None, f'hangs: probe round did not finish within {timeout} s'
        if kind != 'RESULT':
            return None, 'browser exited during the probe round'
        res['visitWallMs'] = (time.perf_counter() - t0) * 1000
        return res, None

    def done(self):
        """Close the held page (no answer)."""
        try:
            self.p.stdin.write(b'{"op":"done"}\n')
        except OSError:
            pass

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
        if 'S' in warm and M.scroll_steps(page.meta) == 0:  # nothing to scroll: every step would wait out its guard
            warm.remove('S'); phases = [p for p in phases if p != 'S']; REC.row(rec, 'S')['empty'] = M.NO_SCROLL
        probe = str(probe_file(page)); tr = fam == 'chromium'
        ours = self._ours(page, cls, policy)
        # visit 1: load (cold sample 1) -> steady window -> one probe round of every warm row -> CPU loops; the page
        # stays open for more probe rounds of the rows that did not settle (no reload, the settled ones are not rerun)
        op = {'probe': probe, 'cases': ','.join(warm), 'steadyMs': policy.steady_ms, 'hold': bool(warm),
              'cpuCases': warm, 'cpuFrames': policy.cpu_frames, 'trace': tr, 'traceCategories': self.e.get('trace_categories')}
        res, why, extra = s.visit(op, to)
        if res is None or res.get('status') != 'ok':
            why = why or 'did not open: ' + ' '.join(str((res or {}).get('error', '?')).split())[:160]
            if res is not None and res.get('held'):
                s.done()
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
        rounds = {ph: 1 for ph in phases}
        while res.get('held'):
            need = [ph for ph in warm if REC.row(rec, ph)['repeats'] and rounds[ph] < policy.max_repeats
                    and not policy.settled(REC.row(rec, ph)['repeats'], ours.get(ph))]
            if not need:
                break
            # Chromium: the raster + swap per frame comes from the trace; later rounds reuse round 1's per-frame raster
            # of the same row unless "trace_rounds": "all" (the JS clocks of the round carry the variance)
            again = tr and self.e.get('trace_rounds', 'first') == 'all'
            r2, why = s.more({'cases': ','.join(need), 'trace': again, 'framesOnly': True,
                              'traceCategories': self.e.get('trace_categories')}, to)
            if r2 is not None and tr and not again:
                r2['trace'] = first.get('trace'); r2['trace_reused'] = True
            for ph in need:
                rounds[ph] += 1
            if r2 is None or r2.get('status') != 'ok':
                rec.setdefault('notes', []).append(f'probe round failed: {why or r2.get("error", "?")}'[:200])
                if r2 is None:
                    self.close(lane)
                break
            self._add(rec, r2, need)
        if res.get('held') and lane.id in self.sessions:
            s.done()
        # cold: one fresh load per sample (nothing else in the visit), until it settles or the cap
        while 'cold' in phases and rounds['cold'] < policy.max_repeats and REC.row(rec, 'cold')['repeats'] \
                and not policy.settled(REC.row(rec, 'cold')['repeats'], ours.get('cold')):
            r2, why, _ = s.visit({'probe': probe, 'cases': '', 'steadyMs': 0, 'cpuCases': [],
                                  'trace': tr and self.e.get('cold_clock', 'load') == 'min'}, to)
            rounds['cold'] += 1
            if r2 is None or r2.get('status') != 'ok':
                rec.setdefault('notes', []).append(f'cold visit failed: {why or r2.get("error", "?")}'[:200])
                if r2 is None:
                    self.close(lane)
                break
            self._add(rec, r2, ['cold'])
        for ph in phases:
            r = REC.row(rec, ph); r['time_ms'] = M.med(r['repeats'])
            if r['time_ms'] is None and not r.get('empty'):
                r['empty'] = (M.scroll_empty_reason(first) if ph == 'S' else None) or 'no frames of this phase in the probe result'
            elif r['time_ms'] is not None:
                h = M.ci_half(r['repeats']); r['ci_pct'] = round(100 * h, 1) if h is not None else None
                why_stop = policy.settled(r['repeats'], ours.get(ph))
                r['rounds'] = rounds[ph]; r['settled'] = why_stop
                if not why_stop:
                    r['capped'] = True  # stopped at the cap with the interval still wider than the rule
        rec['visits'] = 1 + max(0, rounds.get('cold', 1) - 1); rec['wall_s'] = time.perf_counter() - t0
        return rec

    def _ours(self, page, cls, policy):
        """Our values of the same page and class ({phase: ms}) when already measured: a row whose interval lies wholly
        beyond ours has its place decided and stops early."""
        name = next((n for n, e in self.cfg.engines.items() if e.get('adapter') == 'ours'), None)
        if not name:
            return {}
        if getattr(self, '_ours_ad', None) is None:
            from . import load
            self._ours_ad = load(name, self.cfg)
        ad = self._ours_ad; part = ad.parts[0]
        key = REC.unit_key(name, ad.version(), page.hash, cls, part, ad.definition(part) + '-' + policy.definition())
        rec = REC.load(self.cfg.store, name, cls, page.id, key) or {}
        return {ph: r.get('time_ms') for ph, r in (rec.get('rows') or {}).items() if r.get('time_ms')}
    def _add(self, rec, res, phases):
        rec.setdefault('visit_walls_ms', []).append({k: round(res[k], 1) for k in ('visitWallMs', 'pageWallMs', 'navWallMs', 'probeWallMs', 'shotWallMs', 'traceWallMs', 'cpuWallMs')
                                                     if isinstance(res.get(k), (int, float))})
        cases = (res.get('probe') or {}).get('cases') or {}
        missed = {k: (c.get('afterPaint') or {}).get('missed') for k, c in cases.items() if c.get('afterPaint')}
        if missed:  # Firefox: frames whose MozAfterPaint did not come within the probe's guard
            rec.setdefault('paint_missed', []).append(missed)
        v = M.browser_visit(rec['family'], res); fr = M.browser_samples(rec['family'], res)
        for ph in phases:
            if ph == 'cold':  # one value per load
                x = v.get(ph) or {}; r = REC.row(rec, ph)
                # "cold_clock": "load" (default) = navigation -> two rAFs after load on every load, no trace needed;
                # "min" = the better of that and the trace's main + raster sum (needs a trace on every cold load)
                val = x.get('value') if self.e.get('cold_clock', 'load') == 'min' else x.get('req')
                if val is not None:
                    r['repeats'].append(val)
                if x.get('raf') is not None:  # kept for the method comparison
                    r.setdefault('alt_trace_ms', []).append(x['raf'])
            elif fr.get(ph):  # warm rows: every frame of the probe round is a sample (as ours: frames of a series)
                r = REC.row(rec, ph); r['repeats'].extend(fr[ph]['samples']); r['clock'] = fr[ph]['clock']
