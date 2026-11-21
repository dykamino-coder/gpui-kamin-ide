"""engine-compare: fast comparison of our HTML engine with other browser engines on prebuilt binaries.

  python -I engine_compare.py --config cfg.json quick <page> [--cls gpu4] [--engines a,b]
  python -I engine_compare.py --config cfg.json run [--pages a,b|--kinds real,syn] [--engines ...] [--classes gpu4,gpu2,soft2,soft1]
                                                [--phases cold,R,...] [--force] [--dry]
  python -I engine_compare.py --config cfg.json doctor [--aa]
  python -I engine_compare.py --config cfg.json report <out.html> [--classes ...]
  python -I engine_compare.py diff <rows-A.json> <rows-B.json>
Nothing is built: the config names the executables. See README.md.
"""
import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from ec import adapters as A  # noqa: E402
from ec import metrics as M  # noqa: E402
from ec import records as REC  # noqa: E402
from ec.config import CLASSES, Config  # noqa: E402
from ec.scheduler import Lane, Runner, plan  # noqa: E402


def csv(s):
    return [x for x in (s or '').split(',') if x]


def lanes_of(cfg, ids=None):
    return [Lane(i, b, cfg.work / f'lane{i}') for i, b in enumerate(cfg.lanes) if ids is None or i in ids]


def fmt(x):
    return '-' if x is None else (f'{x:.0f}' if x >= 100 else f'{x:.1f}' if x >= 10 else f'{x:.2f}')


def gate(cfg, a):
    """Refuse to measure while builds or other engines run on the measuring lanes (they would be in every number)."""
    if getattr(a, 'ignore_load', False):
        return
    import os
    from ec.doctor import intruders, load_and_hogs
    bad = intruders(cfg.lanes)
    _, hogs = load_and_hogs(cfg.lanes, own=(os.getpid(),))
    if bad or hogs:
        sys.exit('refused: processes on the measuring lanes ' + ', '.join(
            [f'{n} pid {p} mask 0x{m:X}' for p, n, m in bad] + [f'{n} pid {p} {s:.0%} of a core' for p, n, s, _ in hogs])
            + ' (move them off the lanes, or --ignore-load for a functional check)')


def quick(cfg, a):
    gate(cfg, a)
    page = cfg.pages.get(a.page)
    if not page:
        sys.exit(f'unknown page {a.page!r}; the page set has {len(cfg.pages)} pages')
    names = csv(a.engines) or list(cfg.engines)
    ads = {n: A.load(n, cfg) for n in names}
    todo, _ = plan(ads, [page], [a.cls], A.QUICK, cfg.store / 'quick', force=True)
    r = Runner(cfg, ads, A.QUICK, lanes_of(cfg), cfg.gpu_parallel, log=lambda m: print(m, flush=True))
    cfg.store = cfg.store / 'quick'
    wall = r.run(todo)
    recs = {n: REC.merge([rec for t, rec in r.done if t.engine == n]) for n in names}
    print(f'\n{page.id} ({page.kind}), {CLASSES[a.cls][2]}; quick repeats; wall {wall:.1f} s\n')
    w = max(10, max(len(n) for n in names) + 2)
    for title, key, scale in (('frame ms', 'time_ms', 1), ('cpu ms/frame', 'cpu_ms', 1)):
        print(title.ljust(12) + ''.join(n.rjust(w) for n in names))
        for ph in M.PHASES:
            cells = [(recs[n]['rows'].get(ph) or {}).get(key) for n in names]
            print(ph.ljust(12) + ''.join((fmt(c) if c is not None else 'x').rjust(w) for c in cells))
        print()
    print('memory MB'.ljust(12) + ''.join(n.rjust(w) for n in names))
    for k in ('peak_bytes', 'steady_bytes'):
        cells = [recs[n]['memory'].get(k) for n in names]
        print(k.split('_')[0].ljust(12) + ''.join((fmt(c / 2 ** 20) if c else '-').rjust(w) for c in cells))
    why = sorted({f'  {n} {ph}: {x["empty"]}' for n in names for ph, x in recs[n]['rows'].items() if x.get('empty')}
                 | {f'  {n} {ph}: {x["note"]}' for n in names for ph, x in recs[n]['rows'].items() if x.get('note')})
    if why:
        print('\nx = no number:\n' + '\n'.join(why))
    walls = ', '.join(f'{t.engine}/{t.part} {rec.get("wall_s", 0):.1f}s' for t, rec in r.done)
    print(f'\nunit walls: {walls}')

def run(cfg, a):
    gate(cfg, a)
    pages = [cfg.pages[i] for i in csv(a.pages)] if a.pages else [p for p in cfg.pages.values() if not a.kinds or p.kind in csv(a.kinds)]
    names = csv(a.engines) or list(cfg.engines)
    ads = {n: A.load(n, cfg) for n in names}
    classes = csv(a.classes) or list(CLASSES)
    n = int(a.repeats or 3)
    pol = A.Policy(min_repeats=n, fast_repeats=max(n, 8) if n > 1 else 1, max_repeats=max(n, 8) if n > 1 else 1,
                   cpu_frames=int(a.cpu_frames or 10))
    todo, cached = plan(ads, pages, classes, pol, cfg.store, force=a.force)
    print(f'{len(todo)} units to measure, {len(cached)} cached ({len(pages)} pages x {len(classes)} classes x {len(names)} engines)', flush=True)
    if a.dry or not todo:
        return
    log = open(cfg.store / 'run.log', 'a', encoding='utf-8', buffering=1) if cfg.store.mkdir(parents=True, exist_ok=True) is None else None
    def say(m):
        line = time.strftime('%H:%M:%S ') + m; print(line, flush=True); log.write(line + '\n')
    wall = Runner(cfg, ads, pol, lanes_of(cfg), cfg.gpu_parallel, log=say).run(todo)
    say(f'done: {len(todo)} units in {wall:.0f} s')


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--config', default=None)
    ap.add_argument('--store', default=None, help='result store (default: the config''s)')
    sub = ap.add_subparsers(dest='cmd', required=True)
    q = sub.add_parser('quick'); q.add_argument('page'); q.add_argument('--cls', default='gpu4', choices=list(CLASSES)); q.add_argument('--engines'); q.add_argument('--ignore-load', action='store_true')
    r = sub.add_parser('run')
    for k in ('--pages', '--kinds', '--engines', '--classes', '--phases', '--repeats', '--cpu-frames'):
        r.add_argument(k)
    r.add_argument('--force', action='store_true'); r.add_argument('--dry', action='store_true'); r.add_argument('--ignore-load', action='store_true')
    d = sub.add_parser('doctor'); d.add_argument('--aa', action='store_true')
    rp = sub.add_parser('report'); rp.add_argument('out'); rp.add_argument('--classes'); rp.add_argument('--title', default=''); rp.add_argument('--lang', default='en', choices=['en', 'ru'])
    df = sub.add_parser('diff'); df.add_argument('a'); df.add_argument('b'); df.add_argument('--engine', default='ours')
    a = ap.parse_args()
    if a.cmd == 'diff':
        from ec import report
        return report.diff_cli(a)
    if not a.config:
        sys.exit('--config is required')
    cfg = Config(a.config)
    if a.store:
        cfg.store = Path(a.store).resolve()
    if a.cmd == 'quick':
        quick(cfg, a)
    elif a.cmd == 'run':
        run(cfg, a)
    elif a.cmd == 'doctor':
        from ec import doctor
        doctor.main(cfg, a)
    elif a.cmd == 'report':
        from ec import report
        report.main(cfg, a)


if __name__ == '__main__':
    main()
