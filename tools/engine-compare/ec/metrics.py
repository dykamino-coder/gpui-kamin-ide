"""Metric definitions, shared by every adapter. Pure functions over raw engine output (unit-tested).

Frame time of a phase, ms (median over samples):
  ours     req = first_frame_total minus the bench's own capture copy (not raster; browsers do not pay it).
  browser  per visit min(req, raf):
           req = edit in a task -> rAF -> message after the rendering update (probe ``frameReq``),
           raf = edit inside rAF -> message after the rendering update (probe ``frame``);
           Chrome/Edge add (RasterTask + Display::DrawAndSwap) per frame of the same probe window from the trace;
           Firefox raf = MozAfterPaint, req = frameReq + (afterPaint - frame).
  cold     ours first_frame_total of a cold document; browsers time from navigation start to two rAFs after load.
  S        a sample counts only when the root scroller moved by the whole 120 px step; fewer than 3 such steps -> no number.
CPU per frame, ms: ours = cycle time of every thread of the process over the frame window, minus the stand's capture window
  (copy + wake after readback); the old counters-build phase sum (15.6 ms quanta) is no longer used;
  browsers = CPU of the whole browser tree (helpers excluded) over a rAF loop with the edit / frames; cold = navigation -> load.
Memory: peak = highest working-set sum of the engine's processes while the page is open; steady = median of that sum
  0.5 s after load until the end of the steady window (ours: the bench's memory audit after the settle time).
"""
import statistics

PHASES = ['cold', 'R', 'C', 'Cvis', 'D', 'Dvis', 'S']
OURS_MODE = {'cold': 'cold', 'R': 'relayout', 'S': 'scroll'}  # every other phase is an edit


def med(xs):
    xs = [x for x in xs if x is not None]
    return statistics.median(xs) if xs else None


def spread(xs):
    """Relative spread of repeats: (max - min) / median; None with fewer than two values."""
    xs = [x for x in xs if x is not None]
    if len(xs) < 2:
        return None
    m = statistics.median(xs)
    return (max(xs) - min(xs)) / m if m else None


def bench_json(text):
    """The engine_bench JSON document from its stdout (log lines may precede it)."""
    import json
    i = text.find('{')
    return json.loads(text[i:]) if i >= 0 else None


def ours_frame(j, mode):
    """req/raf of one engine_bench mode, ms."""
    m = j['modes'][mode]; s = m['samples_ms']
    tot = [x['first_frame_total'] for x in s]; oth = [x['pipeline_other'] for x in s]
    cc = [(d or {}).get('capture_copy') or 0 for d in ((m.get('diagnostic_scopes') or {}).get('samples_ms') or [None] * len(s))]
    if len(cc) == len(s):
        tot = [a - c for a, c in zip(tot, cc)]
    host = [x.get('host_resources') or 0 for x in s]
    if mode == 'cold':
        raf = [a - b - h for a, b, h in zip(tot, oth, host)]
    else:
        raf = [a - b for a, b in zip(tot, oth)]
    return {'req': med(tot), 'raf': med(raf), 'samples': tot}


def ours_cpu(j, mode):
    """CPU ms per frame from the counters build: median over samples of the per-phase process CPU, pipeline_other excluded."""
    m = (j.get('modes') or {}).get(mode) or {}
    smp = (m.get('process_resources') or {}).get('samples') or []
    v = [sum((ph or {}).get('process_cpu_100ns') or 0 for n, ph in s['phases'].items() if n != 'pipeline_other') / 1e4
         for s in smp if s]
    return med(v)


def ours_cpu_cycles(rep, mode, calibration):
    """CPU ms per frame from a single-process plan: median over samples of the process cycle time inside the frame
    (all threads), converted with the run's cycles per CPU millisecond."""
    k = (calibration or {}).get('cycles_per_cpu_ms')
    m = (rep.get('modes') or {}).get(mode) or {}
    d = (m.get('diagnostic_scopes') or {}).get('samples_ms') or []
    # minus the stand's capture window (copy to memory + wake after readback, the capture's own clock; one thread,
    # so its duration bounds its CPU): browsers have no such step
    v = [max(0.0, x['process_cpu_mcycles'] * 1e6 / k - (x.get('capture_copy') or 0) - (x.get('readback_to_resume') or 0))
         for x in d if x and x.get('process_cpu_mcycles') is not None] if k else []
    return med(v)


def ours_memory(j):
    """(peak, steady) bytes from a ``--mode both --memory-settle-ms`` run."""
    ma = j.get('memory_audit') or {}
    return j.get('peak_working_set_bytes'), (((ma.get('page') or {}).get('end') or {}).get('rss_bytes'))


def ours_vis_check(j):
    """'' when every visible-edit frame matches a fresh render and the edit changes pixels, else the reason."""
    for m in (j.get('modes') or {}).values():
        fc = m.get('fresh_checks') or []
        if not m.get('matches_fresh') or any(c.get('different_rgb_pixels') for c in fc):
            return 'incremental frame differs from a fresh render (engine defect); number shown, not ranked'
        if len(fc) < 2 or len({c.get('fresh_bgr_fnv1a64') for c in fc}) < 2:
            return 'edit not visible: both variants render the same frame (stand defect)'
    return ''


def browser_visit(engine, res):
    """{phase: {'req', 'raf', 'value'}} of one browser visit (driver RESULT)."""
    tw = (res.get('trace') or {}).get('windows') or {}
    probe = res.get('probe') or {}; frames = probe.get('frames') or 14; out = {}

    def ras(win):
        w = tw.get(win)
        return None if not w else ((w.get('RasterTask') or {}).get('sumMs', 0) + (w.get('Display::DrawAndSwap') or {}).get('sumMs', 0)) / frames
    for k, c in (probe.get('cases') or {}).items():
        fr = (c.get('frame') or {}).get('median'); rq = (c.get('frameReq') or {}).get('median')
        ap = (c.get('afterPaint') or {}).get('median') if c.get('afterPaint') else None
        if engine == 'firefox':
            raf = ap; req = rq + max(0.0, ap - fr) if None not in (ap, rq, fr) else None
        else:
            rf, rr = ras('frame:' + k), ras('req:' + k)
            raf = fr + rf if None not in (rf, fr) else None
            req = rq + rr if None not in (rr, rq) else None
        if k == 'S' and ((c.get('frame') or {}).get('moved') or 0) < 3:
            req = raf = None
        vals = [v for v in (req, raf) if v]
        out[k] = {'req': req, 'raf': raf, 'value': min(vals) if vals else None}
    if res.get('twoRafMs') is not None:
        cold = {'req': res.get('twoRafMs'), 'raf': None}; w = tw.get('cold')
        if w:
            main = sum((w.get(n) or {}).get('sumMs', 0) for n in ('ParseHTML', 'UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint'))
            cold['raf'] = main + (w.get('RasterTask') or {}).get('sumMs', 0) + (w.get('Display::DrawAndSwap') or {}).get('sumMs', 0)
        vals = [v for v in (cold['req'], cold['raf']) if v]
        cold['value'] = min(vals) if vals else None
        out['cold'] = cold
    return out


def scroll_empty_reason(res):
    c = (((res.get('probe') or {}).get('cases') or {}).get('S') or {}).get('frame') or {}
    if (c.get('moved') or 0) < 3:
        return 'page does not scroll by three whole 120 px steps'
    return None


def cpu_windows(marks, frames, per_ms=None):
    """CPU ms per frame for every cpu_<case>_0/1 mark pair; cold = nav -> loaded (ms, not per frame).

    marks: {name: (t, cpu_s, ws, private[, cycles])}; frames: {case: frames of the loop}; per_ms: cycles per CPU ms."""
    out = {}
    for k, a in marks.items():
        if k.startswith('cpu_') and k.endswith('_0'):
            c = k[4:-2]; b = marks.get(f'cpu_{c}_1')
            if b and frames.get(c):
                # exact cycle time when calibrated (process times move in ~15.6 ms quanta)
                out[c] = ((b[4] - a[4]) / per_ms if per_ms and len(a) > 4 else (b[1] - a[1]) * 1000) / frames[c]
    if 'nav' in marks and 'loaded' in marks:
        a, b = marks['nav'], marks['loaded']
        out['cold'] = (b[4] - a[4]) / per_ms if per_ms and len(a) > 4 else (b[1] - a[1]) * 1000
    return out


def tree_memory(samples, marks):
    """(peak, steady) working-set sums from tree samples [(t, cpu, ws, pv)] and marks."""
    nav = marks.get('nav'); end = marks.get('steady_end'); loaded = marks.get('loaded')
    after = [s[2] for s in samples if nav and s[0] >= nav[0]]
    peak = max(after) if after else None
    steady = None
    if loaded and end:
        w = [s[2] for s in samples if loaded[0] + 0.5 <= s[0] <= end[0]]
        steady = med(w)
    return peak, steady
