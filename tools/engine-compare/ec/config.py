"""External configuration: engines, page set, lanes, store. Nothing machine-specific lives in the repository."""
import hashlib
import json
import os
from pathlib import Path

# Hardware classes: (CPU mask inside a lane of 4 logical CPUs = 2 physical cores with HT, raster).
CLASSES = {
    'gpu4': ('F', 'gpu', 'GPU raster, 4 threads (2 cores with HT)'),
    'gpu2': ('A', 'gpu', 'GPU raster, 2 threads (2 cores, one thread each)'),
    'soft2': ('A', 'sw', 'software raster, 2 threads'),
    'soft1': ('8', 'sw', 'software raster, 1 thread'),
}
LANE_WIDTH = 4


def lane_mask(cls, base):
    """CPU mask of class ``cls`` on the lane whose first logical CPU is ``base``."""
    return int(CLASSES[cls][0], 16) << base


class Config:
    def __init__(self, path):
        self.path = Path(path).resolve()
        self.raw = json.loads(self.path.read_text(encoding='utf-8-sig'))
        base = self.path.parent
        rel = lambda p: (base / p).resolve() if p and not os.path.isabs(p) else (Path(p) if p else None)
        r = self.raw
        self.store = rel(r.get('store', 'store'))
        self.work = rel(r.get('work', 'work'))
        self.desktop = r.get('desktop', 'engine-compare-hidden')
        self.lanes = list(r.get('lanes', [0, 4, 8]))
        self.gpu_parallel = int(r.get('gpu_parallel', 2))
        # soft1 on one lane per physical core (6 instead of 3); on only after the neighbour A/A check (README)
        self.split_soft1 = bool(r.get('split_soft1', False))
        self.node = r.get('node', 'node')
        self.playwright = r.get('playwright'); self.selenium = r.get('selenium')
        self.engines = r.get('engines', {})
        for e in self.engines.values():  # executable paths may be relative to the config file
            for k in ('exe', 'counters_exe', 'geckodriver'):
                if e.get(k):
                    e[k] = str(rel(e[k]))
        self.causes = rel(r.get('causes')) if r.get('causes') else None
        self.mem_cap_gb = float(r.get('mem_cap_gb', 8))
        self.pages = self._pages(r.get('pages', {}), rel)

    def _pages(self, p, rel):
        """Page set: {"list": file of [{id, kind, dir}]} or {"items": [...]}; optional {"meta": {id: {...}} file}."""
        items = p.get('items') or json.loads(rel(p['list']).read_text(encoding='utf-8')) if (p.get('items') or p.get('list')) else []
        meta = json.loads(rel(p['meta']).read_text(encoding='utf-8')) if p.get('meta') else {}
        # optional {id: {url, about}}: where a page comes from and one line on what it is (report only)
        src = json.loads(rel(p['sources']).read_text(encoding='utf-8')) if p.get('sources') else {}
        for k, v in src.items():
            meta.setdefault(k, {}).update({'source_url': v.get('url'), 'about': v.get('about')})
        out = {}
        for it in items:
            out[it['id']] = Page(it['id'], it.get('kind', 'page'), Path(it['dir']), meta.get(it['id'], {}))
        return out


SOURCES = {'base.html', 'vis.html', 'probe.html'}
DERIVED = {'C.html', 'D.html', 'probe-ec.html'}  # written by the adapters from base.html / probe.html: not page identity


class Page:
    """A prepared page directory: base.html (ours: cold/R/S and the C/D edit variants), vis.html (visible edits),
    probe.html (browsers: base + the probe script). Assets sit next to them."""

    def __init__(self, pid, kind, d, meta):
        self.id = pid; self.kind = kind; self.dir = d; self.meta = meta; self._hash = None

    def file(self, name):
        return self.dir / name

    def size(self):
        """Bytes of the page's HTML and CSS (base document and every stylesheet next to it)."""
        n = self.file('base.html').stat().st_size if self.file('base.html').exists() else 0
        return n + sum(p.stat().st_size for p in self.dir.rglob('*.css'))

    @property
    def hash(self):
        if self._hash is None:
            h = hashlib.sha1()
            for root, dirs, files in os.walk(self.dir):
                dirs.sort()
                for f in sorted(files):
                    p = Path(root) / f; rp = p.relative_to(self.dir).as_posix()
                    if rp in DERIVED:
                        continue
                    h.update(rp.encode())
                    if rp in SOURCES:
                        h.update(p.read_bytes())
                    else:
                        h.update(str(p.stat().st_size).encode())
            self._hash = h.hexdigest()[:12]
        return self._hash


def file_version(path):
    """Content hash of an executable (identity of a build), cached by (size, mtime)."""
    p = Path(path); st = p.stat(); key = (str(p), st.st_size, st.st_mtime_ns)
    if key not in _FV:
        h = hashlib.sha1()
        with open(p, 'rb') as f:
            for chunk in iter(lambda: f.read(1 << 20), b''):
                h.update(chunk)
        _FV[key] = h.hexdigest()[:12]
    return _FV[key]


_FV = {}
