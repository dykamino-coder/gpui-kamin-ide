"""Memory table of a report: every engine's numbers on a page raw and minus its own empty page, the empty pages
themselves as rows (the engine's constant price of process, window and device), and the best other engine per row.

Rows: {cls, page, engine, metric, raw, blank, net}. Metrics: peak/steady working set, peak/steady private bytes and
private/working set after a phase's frames (``after_<case>_private``). ``best`` compares ``net`` where both sides
have it (the page's own cost), else ``raw``; the empty-page rows compare ``raw``.
"""
import csv
from . import metrics as M

BLANK_PAGE = '_blank'


def rows(mem):
    """Long rows of a report's memory {(cls, page, engine): memory record}."""
    out, blanks = [], {}
    for (cls, page, eng), m in sorted(mem.items()):
        b = m.get('blank') or {}
        net = M.minus_blank(m, b)
        for k in M.MEMORY_KEYS:
            if m.get(k) is not None:
                out.append(dict(cls=cls, page=page, engine=eng, metric=k, raw=m[k], blank=b.get(k), net=net.get(k)))
        for case, v in (m.get('after_phase') or {}).items():
            for k, bk in (('private_bytes', 'steady_private_bytes'), ('ws_bytes', 'steady_bytes')):
                if v.get(k) is not None:
                    out.append(dict(cls=cls, page=page, engine=eng, metric=f'after_{case}_{k[:-6]}', raw=v[k],
                                    blank=b.get(bk), net=(net.get('after_phase') or {}).get(case, {}).get(k)))
        if b:
            blanks[(cls, eng)] = b
    for (cls, eng), b in sorted(blanks.items()):
        for k in M.MEMORY_KEYS:
            if b.get(k) is not None:
                out.append(dict(cls=cls, page=BLANK_PAGE, engine=eng, metric=k, raw=b[k], blank=None, net=None))
    return out


def best(rows_, ours='ours'):
    """Our rows with the lowest other engine of the same class, page and metric, and our value over it."""
    by = {}
    for r in rows_:
        by.setdefault((r['cls'], r['page'], r['metric']), []).append(r)
    out = []
    for key, rs in sorted(by.items()):
        me = next((r for r in rs if r['engine'] == ours), None)
        others = [r for r in rs if r['engine'] != ours]
        if not me or not others:
            continue
        use = 'net' if me['net'] is not None and any(r['net'] is not None for r in others) else 'raw'
        cand = [r for r in others if r[use] is not None]
        if me[use] is None or not cand:
            continue
        b = min(cand, key=lambda r: r[use])
        out.append(dict(cls=key[0], page=key[1], metric=key[2], basis=use, ours=me[use], best=b[use], best_engine=b['engine'],
                        ratio=(me[use] / b[use]) if b[use] > 0 else None, gap_bytes=me[use] - b[use]))
    return out


def write(path, mem, ours='ours'):
    """Write ``<path>`` (every engine) and ``<path stem>.best.csv`` (our rows against the best other engine)."""
    rs = rows(mem)
    with open(path, 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=['cls', 'page', 'engine', 'metric', 'raw', 'blank', 'net']); w.writeheader(); w.writerows(rs)
    bs = best(rs, ours)
    bp = path.with_name(path.stem + '.best.csv')
    with open(bp, 'w', newline='', encoding='utf-8') as f:
        w = csv.DictWriter(f, fieldnames=['cls', 'page', 'metric', 'basis', 'ours', 'best', 'best_engine', 'ratio', 'gap_bytes'])
        w.writeheader(); w.writerows(bs)
    return len(rs), len(bs)
