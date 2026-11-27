"""Bounded process-memory observations for memprof.py ws, independent of peak breakdowns."""

from collections import deque


class Trend:
    """Keep the first observation and the most recent five minutes at one-second resolution."""

    def __init__(self):
        self.first = None
        self.latest = None
        self.samples = deque(maxlen=300)

    def observe(self, elapsed_s, working_set, private_usage):
        row = {"elapsed_s": elapsed_s, "working_set": working_set, "private_usage": private_usage}
        if self.first is None:
            self.first = row
        self.latest = row
        if not self.samples or elapsed_s - self.samples[-1]["elapsed_s"] >= 1:
            self.samples.append(row)

    def report(self):
        if self.first is None:
            return None
        rows = list(self.samples)
        if rows[-1] is not self.latest:
            rows.append(self.latest)
        return {"first": self.first, "last": self.latest, "recent": rows}
