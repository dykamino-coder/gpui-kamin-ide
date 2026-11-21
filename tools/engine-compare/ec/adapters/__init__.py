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
    """Adaptive repeats: at least ``min_repeats``; rows faster than ``fast_ms`` get ``fast_repeats``; rows whose
    repeats spread more than ``noisy`` (relative) get more, up to ``max_repeats``. ``quick`` = one repeat."""

    def __init__(self, min_repeats=3, fast_ms=5.0, fast_repeats=8, noisy=0.25, max_repeats=8, cpu_frames=30, steady_ms=2000):
        self.min_repeats = min_repeats; self.fast_ms = fast_ms; self.fast_repeats = fast_repeats
        self.noisy = noisy; self.max_repeats = max_repeats; self.cpu_frames = cpu_frames; self.steady_ms = steady_ms

    def want(self, repeats):
        """How many repeats a row with these values should have in total."""
        from ..metrics import med, spread
        v = [x for x in repeats if x is not None]
        if not v:
            return len(repeats)  # nothing to refine: failures are not retried
        n = self.min_repeats
        if med(v) < self.fast_ms:
            n = max(n, self.fast_repeats)
        s = spread(v)
        if s is not None and s > self.noisy:
            n = max(n, min(self.max_repeats, len(v) + 2))
        return min(n, self.max_repeats)

    def definition(self):
        return f'rep{self.min_repeats}-{self.fast_repeats}@{self.fast_ms}-n{self.noisy}-cpu{self.cpu_frames}-st{self.steady_ms}'


QUICK = Policy(min_repeats=1, fast_repeats=1, max_repeats=1)


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
    else:
        raise ValueError(f'unknown adapter {kind!r} for engine {name!r}')
    return A(name, cfg, e)
