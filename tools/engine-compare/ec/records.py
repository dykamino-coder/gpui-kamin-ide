"""One JSON schema for every engine, and the result cache.

A *unit* is one engine x page x hardware class x part ("time" or "cpu" for ours, "all" for the others): every phase
of the unit is measured in one go and stored in one file. The cache key covers everything a number depends on:
engine build, page content, class, part and the measurement definition. A new build of one engine changes only its
key, so only its column is measured again.

Record:
  {"schema": 1, "key", "engine", "engine_version", "part", "page", "page_hash", "cls", "definition",
   "lane", "mask", "started", "wall_s",
   "rows": {phase: {"time_ms", "time_req_ms", "time_raf_ms", "repeats": [ms per repeat], "cpu_ms", "empty", "note"}},
   "memory": {"peak_bytes", "steady_bytes", "empty"},
   "bounds": text: what the numbers include and exclude for this engine,
   "empty": reason when the whole unit has no numbers}
A cell without a number always carries ``empty``: the reason in plain words.
"""
import hashlib
import json
import os
import time
from pathlib import Path

SCHEMA = 1


def unit_key(engine, engine_version, page_hash, cls, part, definition):
    s = '|'.join(map(str, (SCHEMA, engine, engine_version, page_hash, cls, part, definition)))
    return hashlib.sha1(s.encode()).hexdigest()[:16]


def unit_path(store, engine, cls, page, key):
    return Path(store) / engine / cls / page / f'{key}.json'


def new_record(engine, engine_version, part, page, cls, definition, bounds):
    return {'schema': SCHEMA, 'key': unit_key(engine, engine_version, page.hash, cls, part, definition), 'engine': engine,
            'engine_version': engine_version, 'part': part, 'page': page.id, 'page_kind': page.kind, 'page_hash': page.hash,
            'cls': cls, 'definition': definition, 'started': time.time(), 'rows': {}, 'memory': {}, 'bounds': bounds}


def row(record, phase):
    return record['rows'].setdefault(phase, {'repeats': [], 'empty': None})


def save(store, rec):
    p = unit_path(store, rec['engine'], rec['cls'], rec['page'], rec['key'])
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_suffix('.tmp')
    tmp.write_text(json.dumps(rec, indent=1, default=str), encoding='utf-8')
    os.replace(tmp, p)
    return p


def load(store, engine, cls, page, key):
    p = unit_path(store, engine, cls, page, key)
    try:
        return json.loads(p.read_text(encoding='utf-8'))
    except (OSError, ValueError):
        return None


def scan(store):
    """Every record of the store (all versions)."""
    for p in Path(store).glob('*/*/*/*.json'):
        try:
            yield json.loads(p.read_text(encoding='utf-8'))
        except (OSError, ValueError):
            continue


def merge(recs):
    """One engine's records of a page and class (its parts) as one: numbers win over reasons, rows and memory fields
    are joined (e.g. ours: time from one build and CPU from another, or cold and warm processes)."""
    rows, memory = {}, {}
    for rec in recs:
        for ph, r in (rec.get('rows') or {}).items():
            y = rows.setdefault(ph, {})
            for k, v in r.items():
                if k == 'empty':
                    continue
                if v not in (None, [], '') and y.get(k) in (None, [], ''):
                    y[k] = v
            if r.get('empty') and not y.get('empty_seen'):
                y['empty_seen'] = r['empty']
        for k, v in (rec.get('memory') or {}).items():
            if v is not None and memory.get(k) is None:
                memory[k] = v
        if rec.get('empty'):
            for ph in rows:
                rows[ph].setdefault('empty_seen', rec['empty'])
    for y in rows.values():
        reason = y.pop('empty_seen', None)
        y['empty'] = None if y.get('time_ms') is not None else (reason or 'no frame time in the result')
    if memory.get('peak_bytes') is not None:
        memory['empty'] = None
    return {'rows': rows, 'memory': memory}


def validate(rec):
    """List of schema problems ('' list = valid): every phase row has a number or a reason."""
    bad = []
    for k in ('schema', 'key', 'engine', 'engine_version', 'page', 'cls', 'rows', 'bounds'):
        if k not in rec:
            bad.append(f'missing {k}')
    for ph, r in (rec.get('rows') or {}).items():
        has = r.get('time_ms') is not None or r.get('cpu_ms') is not None
        if not has and not r.get('empty'):
            bad.append(f'{ph}: neither a number nor a reason')
    m = rec.get('memory') or {}
    if m and m.get('peak_bytes') is None and not m.get('empty'):
        bad.append('memory: neither a number nor a reason')
    return bad
