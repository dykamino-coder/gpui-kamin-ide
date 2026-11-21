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
