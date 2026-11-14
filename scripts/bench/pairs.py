"""Canonical paired benchmark runner (harness owner). Raw JSON only; verdict.py judges.

usage: pairs.py OUT_DIR BASE_EXE CAND_EXE PAGES(comma|all) NPAIR MODES(comma) MASK(hex,0=none) AUDIT(0/1) [--seq] [--settle-ms=N]
  --settle-ms=N: audit steady window passed as --memory-settle-ms (default SETTLE_MS).
  --adapter-dir=DIR: directory containing the deployment hb/seqstop/verdict modules.
  OUT_DIR is absolute or relative to agents/harness. Re-running resumes (valid JSON files are kept).
  Layout: {page}-{pair:02}-{base|candidate}-{mode}.json/.err, manifest.json, failures.json.
  Order alternates base/candidate per pair. Bench processes run on hidden desktops (hb.run_hidden).
  A failed process is recorded in failures.json with its stderr tail and retried at most twice,
  never silently: verdict.py reports every failure. After the matrix, every sample still missing gets one more
  round of up to 3 attempts (P17.2: only the failed pairs are rerun, not the matrix).
  --seq (P13.3, harness4): group-sequential stopping, seqstop.py. After pairs 4/6/8 of 10 every page/mode is
  evaluated; a decided unit gets no more pairs (sequential.json). A decisive regression in any unit stops the whole
  matrix at that look (the verdict is NOT READY anyway; every unit decided at that look is listed).
"""
import sys
# The deployment adapter owns scheduling, hidden desktops, corpus and verdict policy.
# Supply its directory explicitly; this runner contains no machine-specific paths.
_adapter = next((a.split('=', 1)[1] for a in sys.argv if a.startswith('--adapter-dir=')), None)
if _adapter:
    sys.path.insert(0, _adapter)
from hb import run_hidden, H, PAGES
import ctypes, hashlib, json
from pathlib import Path

SETTLE_MS = '2000'  # flipped by the lead after fix/bench-settle-window merged and bases were rebuilt
DEFAULT_PAGES = ['text-3200', 'text-800', 'wide-16000', 'p02_many_paragraphs', 'p10_inline_rich', 'text-200',
                 'baseline', 'table-360', 'p03_flex_wrap', 'selectors-800']


def main(argv):
    out, base, cand, pg, npair, md, mask, aud = argv[:8]
    use_seq = '--seq' in argv[8:]
    # harness5 2026-11-08: audit steady window; engine_bench >= aa2c4cef rejects < 2000 (mimalloc ~1 s purge).
    settle = next((a.split('=', 1)[1] for a in argv[8:] if a.startswith('--settle-ms=')), SETTLE_MS)
    R = Path(out) if Path(out).is_absolute() else H / out
    R.mkdir(parents=True, exist_ok=True)
    pages = DEFAULT_PAGES if pg == 'all' else pg.split(',')
    modes = md.split(',')
    audit = aud == '1'
    npair = int(npair)
    if mask != '0':
        k32 = ctypes.WinDLL('kernel32')
        k32.GetCurrentProcess.restype = ctypes.c_void_p
        assert k32.SetProcessAffinityMask(ctypes.c_void_p(k32.GetCurrentProcess()), ctypes.c_size_t(int(mask, 16)))
    exes = {'base': Path(base), 'candidate': Path(cand)}
    manifest = {k: {'path': str(p), 'sha256': hashlib.sha256(p.read_bytes()).hexdigest()} for k, p in exes.items()}
    manifest.update({'pages': pages, 'modes': modes, 'npair': npair, 'affinity_mask': mask, 'audit': audit,
                     'sequential': use_seq, 'memory_settle_ms': int(settle) if audit else None})
    (R / 'manifest.json').write_text(json.dumps(manifest, indent=2))
    failures_file = R / 'failures.json'
    failures = json.loads(failures_file.read_text()) if failures_file.exists() else []
    seq_file = R / 'sequential.json'
    seq = json.loads(seq_file.read_text()) if seq_file.exists() else {'npair': npair, 'units': {}, 'looks': []}

    def run_stem(pair, page, mode, key, attempts):
        stem = f'{page}-{pair:02}-{key}-{mode}'
        f = R / (stem + '.json')
        if f.exists() and f.stat().st_size:
            try:
                json.loads(f.read_text())
                return True
            except ValueError:
                pass
        args = [exes[key], PAGES[page], '--viewport', '800x600', '--runs', '5', '--json', '--mode', mode]
        if mode == 'edit':
            args += ['--edit', 'class']
        if audit and mode == 'both':
            args += ['--memory-settle-ms', settle]
        rc = 1
        attempt_number = sum(item['stem'] == stem for item in failures)
        for _ in range(attempts):
            directory = R / 'attempts' / stem / f'{attempt_number:03}'
            # Never reuse an attempt's pixels, stdout or stderr, even after interruption.
            while directory.exists():
                attempt_number += 1
                directory = directory.parent / f'{attempt_number:03}'
            directory.mkdir(parents=True)
            stdout = directory / 'stdout.json'
            stderr = directory / 'stderr.err'
            capture_args = args + ['--frame-diff-dir', directory / 'frames',
                                   '--fresh-check-dir', directory / 'fresh']
            rc, _ = run_hidden(capture_args, stdout, stderr, stem)
            # The canonical .err always belongs to the first attempt; retries have their own files.
            try:
                with (R / (stem + '.err')).open('xb') as first:
                    first.write(stderr.read_bytes())
            except FileExistsError:
                pass
            if not rc:
                f.write_bytes(stdout.read_bytes())
                break
            tail = stderr.read_text(errors='replace').strip().splitlines()[-1:]
            failures.append({'stem': stem, 'attempt': attempt_number, 'rc': rc, 'stderr': tail,
                             'artifacts': str(directory)})
            failures_file.write_text(json.dumps(failures, indent=1))
            print('FAIL', stem, rc, tail, flush=True)
            attempt_number += 1
        if rc and f.exists():
            f.unlink()
        return not rc

    def active(page, mode):
        return f'{page}|{mode}' not in seq['units']

    def order(pair):
        return ['base', 'candidate'] if pair % 2 else ['candidate', 'base']

    stopped_matrix = False
    for pair in range(1, npair + 1):
        if stopped_matrix:
            break
        for page in pages:
            for mode in modes:
                if active(page, mode):
                    for key in order(pair):
                        run_stem(pair, page, mode, key, 3)
        print('pair', pair, 'completed', flush=True)
        if use_seq:
            stopped_matrix = evaluate(R, manifest, seq, pair, pages, modes, run_stem, order)
            seq_file.write_text(json.dumps(seq, indent=1))
    # P17.2: one more round for samples still missing (only those pairs), units stopped early excluded.
    last = {u: v['stopped_at'] for u, v in seq['units'].items()}
    for pair in range(1, npair + 1):
        for page in pages:
            for mode in modes:
                if pair <= last.get(f'{page}|{mode}', npair):
                    for key in order(pair):
                        run_stem(pair, page, mode, key, 3)
    print('DONE', R, 'failures', len(failures), flush=True)


def evaluate(R, manifest, seq, pair, pages, modes, run_stem, order):
    """Sequential look after `pair` pairs; returns True when the matrix stops."""
    import seqstop, verdict
    if pair not in seqstop.LOOKS or manifest['npair'] != seqstop.NPAIR:
        return False
    # Re-run missing samples of the pairs so far: a decision needs every pair of the unit.
    for p in range(1, pair + 1):
        for page in pages:
            for mode in modes:
                if f'{page}|{mode}' not in seq['units']:
                    for key in order(p):
                        run_stem(p, page, mode, key, 3)
    man, runs = verdict.load_dir(R)
    kind = verdict.kind_of(man)
    bands, sigma = verdict.Bands(verdict.H / 'aa-bands.json'), seqstop.Sigma()
    decided, regression = {}, False
    for page in pages:
        for mode in modes:
            unit = f'{page}|{mode}'
            if unit in seq['units']:
                continue
            pairs = [p for p in range(1, pair + 1) if (page, mode, p, 'base') in runs and (page, mode, p, 'candidate') in runs]
            if len(pairs) < pair:
                continue
            state, reason = seqstop.decide(kind, page, mode, runs, pairs, bands, sigma)
            if state != 'continue':
                decided[unit] = {'stopped_at': pair, 'state': state, 'reason': reason}
                regression |= state == 'stop-regression'
    seq['units'].update(decided)
    seq['looks'].append({'pair': pair, 'decided': len(decided), 'regression': regression})
    for unit, v in decided.items():
        print('SEQ', pair, unit, v['state'], v['reason'], flush=True)
    if regression:
        for page in pages:
            for mode in modes:
                seq['units'].setdefault(f'{page}|{mode}', {'stopped_at': pair, 'state': 'matrix-stopped',
                                                           'reason': 'another unit is a decisive regression'})
        print('SEQ STOP matrix at pair', pair, flush=True)
    return regression


if __name__ == '__main__':
    main(sys.argv[1:])
