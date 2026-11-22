"""Metric definitions, shared by every adapter. Pure functions over raw engine output (unit-tested).

Frame time of a phase, ms (median over samples):
  ours     req = first_frame_total minus the bench's own capture copy (not raster; browsers do not pay it).
  browser  per visit min(req, raf):
           req = edit in a task -> rAF -> message after the rendering update (probe ``frameReq``),
           raf = edit inside rAF -> message after the rendering update (probe ``frame``);
           Chrome/Edge add (RasterTask + Display::DrawAndSwap) per frame of the same probe window from the trace;
           Firefox raf = MozAfterPaint, req = frameReq + (afterPaint - frame).
  cold     ours first_frame_total of a cold document; browsers time from navigation start to two rAFs after load.
  S        the same scenario for every engine: from the top, min(14, whole 120 px steps the page allows) steps of 120 px
           (14 = the browser probe's frames; steps the page allows = (scrollHeight - 600) // 120 from the page meta, down
           to 1); a sample counts only when the root scroller moved by the whole step; no whole step -> no number.
CPU per frame, ms: ours = cycle time of every thread of the process over the frame window, minus the stand's capture window
  (copy + wake after readback); the old counters-build phase sum (15.6 ms quanta) is no longer used;
  browsers = CPU of the whole browser tree (helpers excluded) over a rAF loop with the edit / frames; cold = navigation -> load.
Memory: peak = highest working-set sum of the engine's processes while the page is open; steady = median of that sum
  0.5 s after load until the end of the steady window (ours: the bench's memory audit after the settle time).
"""
import statistics

PHASES = ['cold', 'R', 'C', 'Cvis', 'D', 'Dvis', 'S']
SCROLL_STEP_PX = 120
SCROLL_MAX_STEPS = 14  # FRAMES of the browser probe: one scroll pass
TIMEOUT = 'timeout'  # first word of the reason of a cell whose unit was cut at its time limit


def timed_out(rec):
    """True when the unit was cut at its time limit (a cell or the memory says so)."""
    why = [r.get('empty') for r in (rec.get('rows') or {}).values()] + [(rec.get('memory') or {}).get('empty')]
    return any(w and (w.startswith(TIMEOUT) or w.startswith('hangs')) for w in why)


NO_SCROLL = 'page does not scroll by one whole 120 px step'
OURS_MODE = {'cold': 'cold', 'R': 'relayout', 'S': 'scroll'}  # every other phase is an edit


def scroll_steps(meta, viewport_h=600):
    """Steps of the scroll scenario on a page: min(14, whole 120 px steps below the first screen); None unknown."""
    sh = (meta or {}).get('scrollHeight')
    if sh is None:
        return None
    return max(0, min(SCROLL_MAX_STEPS, (int(sh) - viewport_h) // SCROLL_STEP_PX))


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


def rel_mad(xs):
    """Median absolute deviation over the median (the series stop rule); None with fewer than two values."""
    xs = [x for x in xs if x is not None]
    if len(xs) < 2:
        return None
    m = statistics.median(xs)
    return statistics.median(abs(x - m) for x in xs) / m if m else None


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


def ours_steady(audit):
    """(steady bytes, note) of an engine_bench memory audit: the median working set over the idle window (older
    benches: its end); the note says when the page kept painting through every quiet window."""
    page = (audit or {}).get('page') or {}
    if (audit or {}).get('error'):
        return None, 'memory audit failed: ' + str(audit['error'])[:160]
    value = (page.get('median') or {}).get('rss_bytes') or (page.get('end') or {}).get('rss_bytes')
    note = ''
    if page.get('quiet') is False:
        note = (f"page kept painting: {page.get('unexpected_frames')} frames in the measured window after "
                f"{page.get('busy_windows')} busy windows; median of the window")
    return value, note


def oneshot_cpu_ms(page, blank):
    """Cold CPU ms of a one-shot engine: median CPU of the whole process tree over its life on the page minus the same on
    the empty page. page/blank: [{'cpu_s', 'cycles'}]; cycle time is converted with the rate of these launches (process
    times alone move in ~15.6 ms quanta). None when there is no baseline or the page does not exceed it."""
    if not page or not blank:
        return None
    every = page + blank
    sec = sum(x['cpu_s'] for x in every); cyc = sum(x['cycles'] for x in every)
    rate = cyc / (sec * 1000) if sec > 0.2 and cyc else None

    def ms(x):
        return x['cycles'] / rate if rate else x['cpu_s'] * 1000
    v = med([ms(x) for x in page]) - med([ms(x) for x in blank])
    return v if v > 0 else None


def ours_memory(j):
    """(peak, steady) bytes from a ``--mode both --memory-settle-ms`` run."""
    return j.get('peak_working_set_bytes'), ours_steady(j.get('memory_audit'))[0]


def ours_vis_check(j):
    """'' when every visible-edit frame matches a fresh render and the edit changes pixels, else the reason."""
    for m in (j.get('modes') or {}).values():
        fc = m.get('fresh_checks') or []
        if not m.get('matches_fresh') or any(c.get('different_rgb_pixels') for c in fc):
            return 'incremental frame differs from a fresh render (engine defect); number shown, not ranked'
        if len(fc) < 2 or len({c.get('fresh_bgr_fnv1a64') for c in fc}) < 2:
            return 'edit not visible in our frames: both variants render the same frame (Chrome shows the edit: engine defect)'
    return ''


def browser_visit(engine, res):
    """{phase: {'req', 'raf', 'value'}} of one browser visit (driver RESULT)."""
    tw = (res.get('trace') or {}).get('windows') or {}
    probe = res.get('probe') or {}; frames = probe.get('frames') or 14; out = {}

    def ras(win):
        w = tw.get(win)
        return None if not w else ((w.get('RasterTask') or {}).get('sumMs', 0) + (w.get('Display::DrawAndSwap') or {}).get('sumMs', 0)) / frames
    for k, c in cases_of(engine, res).items():
        fr = (c.get('frame') or {}).get('median'); rq = (c.get('frameReq') or {}).get('median')
        ap = (c.get('afterPaint') or {}).get('median') if c.get('afterPaint') else None
        if engine == 'firefox':
            raf = ap; req = rq + max(0.0, ap - fr) if None not in (ap, rq, fr) else None
        elif engine == 'servo':  # Servo's own profiler: Layout (style, layout, display list) + Painting (WebRender)
            raf, req = servo_frame(c), None
        else:
            rf, rr = ras('frame:' + k), ras('req:' + k)
            raf = fr + rf if None not in (rf, fr) else None
            req = rq + rr if None not in (rr, rq) else None
        if k == 'S' and ((c.get('frame') or {}).get('moved') or 0) < 1:
            req = raf = None
        vals = [v for v in (req, raf) if v]
        out[k] = {'req': req, 'raf': raf, 'value': min(vals) if vals else None}
    if res.get('twoRafMs') is not None:
        cold = {'req': res.get('twoRafMs'), 'raf': None}; w = tw.get('cold')
        if engine == 'servo':
            sc = (res.get('servo') or {}).get('cold') or {}
            work = sum((sc.get(n) or {}).get('sumMs', 0) for n in SERVO_COLD)
            cold['raf'] = work or None
        elif w:
            main = sum((w.get(n) or {}).get('sumMs', 0) for n in ('ParseHTML', 'UpdateLayoutTree', 'Layout', 'PrePaint', 'Paint'))
            cold['raf'] = main + (w.get('RasterTask') or {}).get('sumMs', 0) + (w.get('Display::DrawAndSwap') or {}).get('sumMs', 0)
        vals = [v for v in (cold['req'], cold['raf']) if v]
        cold['value'] = min(vals) if vals else None
        out['cold'] = cold
    return out


SERVO_COLD = ('ScriptParseHTML', 'ScriptStylesheetLoad', 'Layout', 'Painting')
SERVO_PARTIAL = ("Servo's profiler has no WebRender scene and frame building (render backend threads): the frame is "
                 'style + layout + display list + WebRender raster and composite only, not ranked as equal')


def cases_of(engine, res):
    """{phase: probe case} of a visit. Servo has no probe: its phases are the profiler windows of its frame loops."""
    if engine == 'servo':
        return {k: dict(v, frame={'moved': len(v.get('frames') or [])}) for k, v in ((res.get('servo') or {}).get('cases') or {}).items()}
    return (res.get('probe') or {}).get('cases') or {}


def servo_samples(c):
    """Per-frame samples of a Servo case: one edit each, its Layout events + the first Painting after it (the
    driver waits for that Painting to finish; Servo paints asynchronously and would coalesce a loop of edits)."""
    return [f['layoutMs'] + (f.get('paintMs') or 0) for f in (c.get('frames') or [])
            if f.get('layoutMs', 0) + (f.get('paintMs') or 0) > 0]


def servo_frame(c):
    """Median Servo frame of a case (ms), None without frame work."""
    return med(servo_samples(c))


def median_ci(xs, level=0.9):
    """Distribution-free confidence interval of the median from order statistics: (lo, hi, coverage). The narrowest
    symmetric pair of ranks whose binomial coverage reaches ``level``; with too few values for that, the full range
    (coverage 1 - 2^(1-n)). None for fewer than two values."""
    s = sorted(x for x in xs if x is not None); n = len(s)
    if n < 2:
        return None
    from math import comb

    def cover(k):  # ranks k .. n-1-k (0-based): 1 - 2 P(B(n, 1/2) <= k)
        return 1 - 2 * sum(comb(n, i) for i in range(k + 1)) / 2 ** n
    k = 0
    while k + 1 < n - 2 - k and cover(k + 1) >= level:
        k += 1
    cov = cover(k)
    return s[k], s[n - 1 - k], cov


def ci_half(xs, level=0.9):
    """Half-width of the median's confidence interval over the median (relative); None without one."""
    c = median_ci(xs, level); m = med(xs)
    return (c[1] - c[0]) / 2 / m if c and m else None


def accuracy(samples, visit_medians=None, level=0.9):
    """Relative accuracy of a row's median: the larger of the confidence-interval half-width of the pooled samples
    and half the spread of the per-visit medians (frames of one visit understate the visit-to-visit spread)."""
    h = ci_half(samples, level); m = med(samples)
    v = [x for x in (visit_medians or []) if x is not None]
    if len(v) >= 2 and m:
        h = max(h or 0.0, (max(v) - min(v)) / 2 / m)
    return h


def accuracy_range(samples, visit_medians=None, level=0.9):
    """(lo, hi) of the median under ``accuracy``: the interval widened to every visit median."""
    c = median_ci(samples, level)
    if not c:
        return None
    v = [x for x in (visit_medians or []) if x is not None]
    return min([c[0]] + v), max([c[1]] + v)

def browser_samples(engine, res):
    """{phase: {'value', 'samples', 'clock'}} of one probe round: per-frame samples of the clock with the lower median
    (the same choice as ``browser_visit``), each with the round's per-frame raster + swap from the trace (Chromium) or
    the MozAfterPaint lag (Firefox); Servo: every edit + screenshot minus the median screenshot alone."""
    tw = (res.get('trace') or {}).get('windows') or {}
    probe = res.get('probe') or {}; frames = probe.get('frames') or 14; out = {}

    def ras(win):
        w = tw.get(win)
        return None if not w else ((w.get('RasterTask') or {}).get('sumMs', 0) + (w.get('Display::DrawAndSwap') or {}).get('sumMs', 0)) / frames

    def smp(x):
        return [v for v in ((x or {}).get('samples') or []) if v is not None]
    for k, c in cases_of(engine, res).items():
        fr, rq, ap = smp(c.get('frame')), smp(c.get('frameReq')), smp(c.get('afterPaint'))
        if k == 'S' and ((c.get('frame') or {}).get('moved') or 0) < 1:
            continue
        cand = {}
        if engine == 'firefox':
            if ap:
                cand['raf'] = ap
            if rq and ap and fr:
                lag = max(0.0, med(ap) - med(fr)); cand['req'] = [v + lag for v in rq]
        elif engine == 'servo':
            cand['profiler'] = servo_samples(c)
        else:
            rf, rr = ras('frame:' + k), ras('req:' + k)
            if fr and rf is not None:
                cand['raf'] = [v + rf for v in fr]
            if rq and rr is not None:
                cand['req'] = [v + rr for v in rq]
        cand = {n: v for n, v in cand.items() if v and med(v) and med(v) > 0}
        if cand:
            clock = min(cand, key=lambda n: med(cand[n]))
            out[k] = {'value': med(cand[clock]), 'samples': cand[clock], 'clock': clock}
    return out

def scroll_empty_reason(res):
    c = (((res.get('probe') or {}).get('cases') or {}).get('S') or {}).get('frame') or {}
    if not c and ((res.get('servo') or {}).get('cases') or {}).get('S'):  # Servo: the steps of its frame loop
        c = {'moved': res['servo']['cases']['S'].get('frames') or 0}
    if (c.get('moved') or 0) < 1:
        return NO_SCROLL
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
