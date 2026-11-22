"""Engine adapters. Each adapter is one file implementing the ``Adapter`` contract below.

Contract:
  name, parts              -- parts of a unit ("all"; ours: "time" and "cpu", measured by different builds)
  version()                -- identity of the engine build (cache key)
  definition(part)         -- identity of the measurement definition (cache key); bump when a metric changes
  needs_gpu(cls)           -- True when the unit uses the shared GPU (the scheduler limits concurrent GPU units)
  exclusive                -- True when the engine cannot be pinned to a lane (runs alone, machine otherwise idle)
  session(lane, cls)       -- key of a long-lived process kept on the lane between units (None = none)
  measure(lane, page, cls, part, policy) -> record (ec.records schema); never raises for engine failures,
                              a failed cell carries its reason in ``empty``
  close(lane)              -- stop the lane's long-lived processes
"""
import re


class Policy:
    """Adaptive repeats (the same rule as our bench's frame series): at least ``min_repeats`` values, then one more
    while the row is not stable, up to ``max_repeats``. Stable = MAD / median <= ``stable`` (relative median absolute
    deviation). ``quick`` = one repeat."""

    def __init__(self, min_repeats=3, max_repeats=8, stable=0.03, cpu_frames=10, steady_ms=2000):
        self.min_repeats = min_repeats; self.max_repeats = max(max_repeats, min_repeats); self.stable = stable
        self.cpu_frames = cpu_frames; self.steady_ms = steady_ms

    def want(self, repeats):
        """How many repeats a row with these values should have in total."""
        from ..metrics import rel_mad
        v = [x for x in repeats if x is not None]
        if not v:
            return len(repeats)  # nothing to refine: failures are not retried
        if len(v) < self.min_repeats:
            return self.min_repeats
        r = rel_mad(v)
        return len(v) if r is not None and r <= self.stable else min(self.max_repeats, len(v) + 1)

    def settled(self, samples, ours=None, visit_medians=None, margin=0.03, floor_ms=0.1):
        """Why a row may stop: 'ci' when its accuracy (``metrics.accuracy``: the median's 90 % confidence interval
        from order statistics, widened by the spread of the visit medians) is within ``stable`` of the median or within
        the clock step ``floor_ms``; 'place' when that range lies wholly beyond our value of the same row by ``margin``
        (its place against ours is decided); None while neither holds or with fewer than ``min_repeats`` samples."""
        from ..metrics import accuracy, accuracy_range, med
        v = [x for x in samples if x is not None]
        if len(v) < self.min_repeats:
            return None
        h = accuracy(v, visit_medians); m = med(v)
        # browsers' performance.now() moves in 0.1 ms steps: an accuracy below that is not measurable
        if h is not None and (h <= self.stable or h * m <= floor_ms):
            return 'ci'
        c = accuracy_range(v, visit_medians)
        if ours and c and (c[0] > ours * (1 + margin) or c[1] < ours * (1 - margin)):
            return 'place'
        return None if m else 'ci'
    def definition(self):
        return f'rep{self.min_repeats}-{self.max_repeats}-mad{self.stable}-cpu{self.cpu_frames}-st{self.steady_ms}'

QUICK = Policy(min_repeats=1, max_repeats=1)


def last_error(text, limit=160):
    """Last meaningful stderr line with local paths removed (the report is shareable)."""
    lines = [x.strip() for x in (text or '').splitlines() if x.strip()]
    keep = [x for x in lines if not re.search(r'\b(ERROR|WARN)\s+vendor', x)] or lines
    m = re.sub(r'^engine_bench:\s*', '', keep[-1]) if keep else 'no message'
    return re.sub(r'[A-Za-z]:\\\S+', '<file>', m)[:limit]


def load(name, cfg):
    e = cfg.engines[name]; kind = e.get('adapter', name)
    if kind == 'ours':
        from .ours import Ours as A
    elif kind == 'browser':
        from .browser import Browser as A
    elif kind == 'oneshot':
        from .oneshot import OneShot as A
    elif kind == 'kitesurf':
        from .kitesurf import Kitesurf as A
    elif kind == 'native':
        from .native import Native as A
    else:
        raise ValueError(f'unknown adapter {kind!r} for engine {name!r}')
    return A(name, cfg, e)
