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

    def definition(self):
        return f'rep{self.min_repeats}-{self.max_repeats}-mad{self.stable}-cpu{self.cpu_frames}-st{self.steady_ms}'


QUICK = Policy(min_repeats=1, max_repeats=1)
# The empty page every engine also opens: its memory is the engine's constant price (process, window, device), which
# the report shows apart and subtracts from each page.
BLANK = '<!doctype html><html><head><title>blank</title></head><body></body></html>'


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
