"""Lanes and the unit scheduler.

A lane is a set of 4 logical CPUs (2 physical P-cores with HT) of equal performance; a hardware class is a CPU mask
inside the lane plus a raster choice. Lanes run units in parallel; at most ``gpu_parallel`` GPU units at once;
engines that cannot be pinned (``exclusive``) run alone after everything else. A lane keeps its long-lived browser
while it takes further pages of the same engine and class. Finished units are cached, so a run resumes where it
stopped and only units whose key changed are measured again.
"""
import threading
import time
from pathlib import Path

from . import records as REC
from .config import lane_mask


class Lane:
    def __init__(self, n, base, work):
        self.id = n; self.base = base; self.work = Path(work)
        self.work.mkdir(parents=True, exist_ok=True)

    def mask(self, cls):
        return lane_mask(cls, self.base)


class Task:
    __slots__ = ('engine', 'page', 'cls', 'part', 'key', 'gpu', 'session', 'exclusive')

    def __init__(self, engine, page, cls, part, key, gpu, session, exclusive):
        self.engine = engine; self.page = page; self.cls = cls; self.part = part; self.key = key
        self.gpu = gpu; self.session = session; self.exclusive = exclusive

    def __repr__(self):
        return f'{self.engine}/{self.part} {self.page.id} {self.cls}'


def canon(ad, cls):
    """Class the engine actually measures: without GPU raster, gpu2 and soft2 are the same setup (mask A, CPU raster)."""
    return 'soft2' if cls == 'gpu2' and not ad.needs_gpu('gpu2') else cls


def started_in(store, t, window):
    """True when the cached record of ``t`` was started inside ``window`` (epoch from, to): measured under known load."""
    rec = REC.load(store, t.engine, t.cls, t.page.id, t.key)
    return bool(rec) and window[0] <= (rec.get('started') or 0) < window[1]


def plan(adapters, pages, classes, policy, store, force=False, redo=None):
    """(todo, cached) task lists; ``adapters`` = {name: adapter}. Equivalent classes are measured once (``canon``).
    ``redo`` = (from, to) epoch: cached units started in that window are measured again."""
    todo, cached, seen = [], [], set()
    for cls0 in classes:
        for name, ad in adapters.items():
            ver = ad.version(); cls = canon(ad, cls0)
            if (name, cls) in seen:
                continue
            seen.add((name, cls))
            for part in ad.parts:
                d = ad.definition(part) + '-' + policy.definition()
                for p in pages:
                    key = REC.unit_key(name, ver, p.hash, cls, part, d)
                    t = Task(name, p, cls, part, key, ad.needs_gpu(cls), (name, cls) if ad.session(None, cls) else None, ad.exclusive)
                    have = not force and REC.unit_path(store, name, cls, p.id, key).exists()
                    if have and redo and started_in(store, t, redo):
                        have = False
                    (cached if have else todo).append(t)
    return todo, cached


def pick(queue, lane_session, gpu_busy, gpu_parallel, others_busy):
    """Index of the next task for a lane, or None. Same session first; GPU units only while a GPU slot is free;
    exclusive units only when nothing else is queued or running."""
    best = None
    for i, t in enumerate(queue):
        if t.exclusive:
            continue
        if t.gpu and gpu_busy >= gpu_parallel:
            continue
        if lane_session is not None and t.session == lane_session:
            return i
        if best is None:
            best = i
    if best is not None:
        return best
    if not others_busy:
        for i, t in enumerate(queue):
            if t.exclusive and not (t.gpu and gpu_busy >= gpu_parallel):
                return i
    return None


class Runner:
    def __init__(self, cfg, adapters, policy, lanes, gpu_parallel, log=print):
        self.cfg = cfg; self.ad = adapters; self.policy = policy; self.lanes = lanes
        self.gpu_parallel = gpu_parallel; self.log = log; self.cv = threading.Condition(); self.gpu = 0
        self.busy = {}; self.done = []; self.stop = False; self.keep_memory = False

    def run(self, tasks):
        q = list(tasks)
        q.sort(key=lambda t: (t.exclusive, t.cls, t.engine, t.part, t.page.id))
        t0 = time.perf_counter()

        def worker(lane):
            sess = None
            while True:
                with self.cv:
                    while True:
                        if self.stop or not q:
                            break
                        i = pick(q, sess, self.gpu, self.gpu_parallel, any(v for k, v in self.busy.items() if k != lane.id))
                        if i is not None:
                            break
                        self.cv.wait(1)
                    if self.stop or not q:
                        break
                    t = q.pop(i); self.busy[lane.id] = t
                    if t.gpu:
                        self.gpu += 1
                if t.exclusive:  # wait until every other lane is idle
                    with self.cv:
                        while any(v for k, v in self.busy.items() if k != lane.id):
                            self.cv.wait(1)
                ad = self.ad[t.engine]
                if sess and sess != t.session:
                    for a in self.ad.values():
                        a.close(lane)
                sess = t.session
                s = time.perf_counter()
                try:
                    rec = ad.measure(lane, t.page, t.cls, t.part, self.policy)
                except Exception as e:  # an adapter bug must not stop the other lanes
                    rec = REC.new_record(t.engine, ad.version(), t.part, t.page, t.cls, 'error', 'adapter failed')
                    rec['empty'] = f'tool error: {type(e).__name__}: {e}'[:300]
                    rec['rows'] = {}
                    rec['key'] = None
                if self.keep_memory:  # a redo for time/CPU keeps the memory measured before (memory was not affected)
                    old = REC.load(self.cfg.store, t.engine, t.cls, t.page.id, t.key)
                    if old and (old.get('memory') or {}).get('peak_bytes'):
                        rec['memory'] = old['memory']; rec['memory_started'] = old.get('started')
                rec['key'] = rec.get('key') or t.key
                if rec['key'] == t.key:
                    REC.save(self.cfg.store, rec)
                else:  # adapter computed another key (definition drift): store under the planned key
                    rec['key'] = t.key; REC.save(self.cfg.store, rec)
                self.log(f'L{lane.id} {t} {time.perf_counter() - s:.1f}s')
                with self.cv:
                    self.busy[lane.id] = None
                    if t.gpu:
                        self.gpu -= 1
                    self.done.append((t, rec)); self.cv.notify_all()
            for a in self.ad.values():
                a.close(lane)

        th = [threading.Thread(target=worker, args=(ln,), daemon=True) for ln in self.lanes]
        [x.start() for x in th]
        try:
            while any(x.is_alive() for x in th):
                for x in th:
                    x.join(0.5)
        except KeyboardInterrupt:
            with self.cv:
                self.stop = True; self.cv.notify_all()
            [x.join(30) for x in th]
        return time.perf_counter() - t0
