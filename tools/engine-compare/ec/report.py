"""report: one self-contained HTML page (+ rows.json for diff); diff: what got better or worse outside the noise.

Every cell has a number or the reason it has none. Ours is compared with the best other engine of the row.
"""
import html
import json
import sys
from pathlib import Path

from . import adapters as A
from . import metrics as M
from . import records as REC
from .config import CLASSES
from .scheduler import canon, plan

MB = 2 ** 20
L = {
    'en': {'title': 'Our engine against every engine', 'phase': {'cold': 'cold load', 'R': 'viewport width change',
           'C': 'style edit, off screen', 'Cvis': 'style edit, visible', 'D': 'content edit, off screen',
           'Dvis': 'content edit, visible', 'S': 'scroll step (120 px)'},
           'summary': 'Summary: where we lead and where we do not', 'worst_t': 'every row by time, best for us first',
           'worst_m': 'every page by peak memory, best for us first', 'worst_c': 'every row by CPU per frame, best for us first', 'all': 'All comparisons', 'pass': 'Time to get through the whole page set', 'pass_note': 'Sum of unit walls per engine and class (lanes run in parallel, so this is machine time, not the run time). Goal: ours is the fastest. Ours is split into the stand (process launches, memory windows, fresh-frame checks, separate CPU build) and the engine (document load and phase frames). CPU-raster engines measure gpu2 once under soft2.', 'pass_total': 'Total', 'pass_page': 'Per page', 'pass_rank': 'Place', 'pass_stand': 'Stand', 'pass_engine': 'Engine', 'pass_pages': 'Pages', 'pass_missing': 'units not measured', 'count': 'behind (over 5%) in {n}, first in {m}, within 5% in {k}, of {t} comparable', 'causes': 'Causes and how to win',
           'page': 'Page', 'ph': 'Phase', 'ours': 'Ours', 'best': 'Best', 'ratio': 'Ours / best', 'note': 'Note',
           'rows': 'Rows', 'faster': 'We are faster', 'median': 'Median ours / best', 'worst': 'Worst',
           'mem': 'Memory, MB (peak / steady)', 'cpu': 'CPU per frame, ms', 'time': 'Frame time, ms',
           'not_measured': 'not measured yet', 'none': 'no other engine has a number', 'fast': 'row under 5 ms: verdict needs 8 repeats',
           'invalid': 'not ranked: ', 'gap': 'Peak gap, MB', 'bounds': 'Measurement bounds', 'versions': 'Versions',
           'legend': ['we are faster than the best', 'within 5%', 'slower, up to x2', 'x2-x10', 'x10 and more'],
           'v_style': 'style edit', 'v_content': 'content edit', 'v_vis': 'visible edit', 'source': 'source',
           'pages_title': 'Pages under test', 'about': 'What it is', 'size': 'HTML+CSS, KB', 'local': 'local file',
           'intro': 'Frame time is from the frame request to finished pixels, ms (median of repeats); browsers take the '
                    'better of their two clocks. Viewport 800x600. Lanes run on equal performance cores in parallel.'},
    'ru': {'title': 'Наш движок против всех движков', 'phase': {'cold': 'холодная загрузка', 'R': 'смена ширины окна',
           'C': 'правка стиля, вне экрана', 'Cvis': 'правка стиля, видимая', 'D': 'правка содержимого, вне экрана',
           'Dvis': 'правка содержимого, видимая', 'S': 'шаг прокрутки (120 px)'},
           'summary': 'Сводка: где мы первые и где нет', 'worst_t': 'все строки по времени, от лучших для нас к худшим',
           'worst_m': 'все страницы по пику памяти, от лучших к худшим', 'worst_c': 'все строки по процессору на кадр, от лучших к худшим', 'all': 'Все сравнения', 'pass': 'Время прохождения всего набора', 'pass_note': 'Сумма времени единиц по движку и классу (дорожки идут параллельно, это машинное время, а не длительность захода). Цель: наш движок проходит набор быстрее любого. Наш столбец разложен на стенд (запуски процессов, окна памяти, сверка со свежим кадром, отдельная сборка для CPU) и движок (загрузка документа и кадры фаз). Движки с растром на процессоре меряют gpu2 один раз в soft2.', 'pass_total': 'Всего', 'pass_page': 'На страницу', 'pass_rank': 'Место', 'pass_stand': 'Стенд', 'pass_engine': 'Движок', 'pass_pages': 'Страниц', 'pass_missing': 'единиц не измерено', 'count': 'отстаём (больше 5 %) в {n}, первые в {m}, в пределах 5 % в {k}, из {t} сравнимых',
           'causes': 'Причины и способы обгона', 'page': 'Страница', 'ph': 'Фаза', 'ours': 'Мы', 'best': 'Лучший',
           'ratio': 'Мы / лучший', 'note': 'Пометка', 'rows': 'Строк', 'faster': 'Мы быстрее', 'median': 'Медиана мы / лучший',
           'worst': 'Худшая', 'mem': 'Память, МБ (пик / уст.)', 'cpu': 'Процессор на кадр, мс', 'time': 'Время кадра, мс',
           'not_measured': 'ещё не измерено', 'none': 'ни один другой движок не дал числа',
           'fast': 'строка <5 мс: вердикт только при 8 повторах', 'invalid': 'не в счёт: ', 'gap': 'Разрыв пика, МБ',
           'bounds': 'Границы замера', 'versions': 'Версии',
           'legend': ['мы быстрее лучшего', '±5 %', 'медленнее до ×2', '×2–10', '×10 и больше'],
           'v_style': 'правка стиля', 'v_content': 'правка содержимого', 'v_vis': 'видимая правка', 'source': 'источник',
           'pages_title': 'Проверяемые страницы', 'about': 'Что это', 'size': 'HTML+CSS, КБ', 'local': 'локальный файл',
           'intro': 'Время — кадр от запроса до готовых пикселей, мс (медиана повторов); у браузера берётся лучший из двух '
                    'способов отсчёта. Окно 800×600. Дорожки идут параллельно на равных производительных ядрах.'},
}


def f(x):
    return '—' if x is None else (f'{x:.0f}' if x >= 100 else f'{x:.1f}' if x >= 10 else f'{x:.2f}')


def col(r):
    if r is None:
        return 'nd'
    return 'g' if r < 0.95 else 'n' if r <= 1.05 else 'y' if r < 2 else 'o' if r < 10 else 'r'


def latest(store, t, version):
    """Newest record of the same build, page content, class and part under another definition (e.g. a quick run)."""
    best = None
    for p in (Path(store) / t.engine / t.cls / t.page.id).glob('*.json'):
        try:
            r = json.loads(p.read_text(encoding='utf-8'))
        except (OSError, ValueError):
            continue
        if r.get('engine_version') == version and r.get('page_hash') == t.page.hash and r.get('part') == t.part:
            if best is None or r.get('started', 0) > best.get('started', 0):
                best = r
    return best


def collect(cfg, classes, engines=None):
    """rows: [{cls, page, kind, engine, phase, time_ms, repeats, cpu_ms, empty, note, invalid}], mem: {(cls, page, engine): {...}}"""
    names = engines or list(cfg.engines)
    ads = {n: A.load(n, cfg) for n in names}
    todo, cached = plan(ads, list(cfg.pages.values()), classes, A.Policy(), cfg.store)
    rows, mem, bounds, vers, units = [], {}, {}, {}, {}
    dirty = contamination(cfg)
    passes = {}  # (cls, engine) -> {'wall', 'engine', 'pages', 'units', 'missing'}
    for t in todo + cached:
        rec = REC.load(cfg.store, t.engine, t.cls, t.page.id, t.key) or latest(cfg.store, t, ads[t.engine].version())
        units.setdefault((t.cls, t.page, t.engine), []).append(rec)
        ps = passes.setdefault((t.cls, t.engine), {'wall': 0.0, 'engine': 0.0, 'pages': set(), 'units': 0, 'missing': 0})
        if not rec:
            ps['missing'] += 1; continue
        ps['wall'] += rec.get('wall_s') or 0; ps['units'] += 1; ps['pages'].add(t.page.id)
        if ads[t.engine].e.get('adapter') == 'ours':  # frames the engine itself spent; the rest is the stand
            ps['engine'] += sum(sum(x for x in (r.get('repeats') or []) if x) for r in (rec.get('rows') or {}).values()) / 1000
        note = next((d['note'] for d in dirty if d['from'] <= (rec.get('started') or 0) < d['to']), None)
        if note and not rec.get('_dirty'):
            rec['_dirty'] = True
            for r in (rec.get('rows') or {}).values():
                r['note'] = '; '.join(x for x in (r.get('note'), note) if x)
    shared = {}  # (cls, engine) -> measured class, for CPU-raster engines whose classes coincide
    for cls in classes:
        for n, ad in ads.items():
            c = canon(ad, cls)
            if c != cls:
                shared[(cls, n)] = c
                for (c2, p, e), recs in list(units.items()):
                    if c2 == c and e == n:
                        units[(cls, p, e)] = recs
    for (cls, page, eng), recs in units.items():
        base = {'cls': cls, 'page': page.id, 'kind': page.kind, 'engine': eng}
        got = [r for r in recs if r]
        if not got:
            rows += [dict(base, phase=ph, empty='not_measured') for ph in M.PHASES]
            continue
        bounds[eng] = got[0].get('bounds'); vers[eng] = got[0].get('browser_version') or got[0].get('engine_version')
        m = REC.merge(got)
        for ph in M.PHASES:
            r = m['rows'].get(ph) or {'empty': 'not_measured' if len(got) < len(recs) else 'no number'}
            note = r.get('note')
            if (cls, eng) in shared or any(k[1] == eng and v == cls for k, v in shared.items()):
                note = '; '.join(x for x in (note, 'engine rasters on the CPU: this class and ' + (shared.get((cls, eng)) or 'gpu2') + ' coincide, measured once') if x)
            rows.append(dict(base, phase=ph, time_ms=r.get('time_ms'), repeats=r.get('repeats') or [], cpu_ms=r.get('cpu_ms'),
                             empty=r.get('empty'), note=note, invalid=r.get('invalid'), within_noise=r.get('within_noise')))
        if m['memory']:
            mem[(cls, page.id, eng)] = m['memory']
    return rows, mem, bounds, vers, passes


def contamination(cfg):
    """Windows of known foreign load from the config: [{"from": ISO, "to": ISO, "note"}] -> epoch seconds."""
    import datetime as dt
    out = []
    for d in cfg.raw.get('contamination') or []:
        out.append({'from': dt.datetime.fromisoformat(d['from']).timestamp(), 'to': dt.datetime.fromisoformat(d['to']).timestamp(),
                    'note': d['note']})
    return out


def main(cfg, a):
    lang = getattr(a, 'lang', 'en') or 'en'; T = L[lang]
    classes = [c for c in (a.classes or ','.join(CLASSES)).split(',') if c]
    rows, mem, bounds, vers, passes = collect(cfg, classes)
    out = Path(a.out)
    (out.with_suffix('.rows.json')).write_text(json.dumps({'rows': rows, 'memory': [dict(cls=k[0], page=k[1], engine=k[2], **v)
                                                           for k, v in mem.items()]}, default=str), encoding='utf-8')
    out.write_text(render(cfg, rows, mem, bounds, vers, classes, T, a.title, passes), encoding='utf-8')
    print(f'report: {out} ({len(rows)} cells); rows for diff: {out.with_suffix(".rows.json")}')


def why(x, T):
    e = x.get('empty')
    return T['not_measured'] if e == 'not_measured' else (e or 'no number')


def pass_table(passes, classes, T, ours):
    """Time to get through the whole page set, per engine and class: goal row (ours must be the fastest)."""
    h = [f'<h3>{T["pass"]}</h3><p class="note">{html.escape(T["pass_note"])}</p><div class="tw"><table><thead><tr><th>class</th>'
         f'<th>engine</th><th>{T["pass_total"]}</th><th>{T["pass_page"]}</th><th>{T["pass_rank"]}</th><th>{T["pass_stand"]}</th>'
         f'<th>{T["pass_engine"]}</th><th>{T["pass_pages"]}</th></tr></thead><tbody>']
    for cls in classes:
        es = [(e, v) for (c, e), v in passes.items() if c == cls and v['pages']]
        es.sort(key=lambda x: x[1]['wall'] / len(x[1]['pages']))
        for rank, (e, v) in enumerate(es, 1):
            per = v['wall'] / len(v['pages'])
            st = en = '—'
            if e == ours:
                en = f'{v["engine"]:.0f} s'; st = f'{v["wall"] - v["engine"]:.0f} s ({100 * (1 - v["engine"] / v["wall"]):.0f} %)' if v['wall'] else '—'
            cl = 'g' if rank == 1 else 'r' if e == ours else ''
            h.append(f'<tr class="{cl if e == ours else ""}"><td>{html.escape(CLASSES[cls][2])}</td><td>{"<b>" + html.escape(e) + "</b>" if e == ours else html.escape(e)}</td>'
                     f'<td class="num">{v["wall"]:.0f} s</td><td class="num">{per:.1f} s</td><td class="num {cl}">{rank} / {len(es)}</td>'
                     f'<td class="num">{st}</td><td class="num">{en}</td><td class="num">{len(v["pages"])}{(" (+" + str(v["missing"]) + " " + T["pass_missing"] + ")") if v["missing"] else ""}</td></tr>')
    h.append('</tbody></table></div>')
    return ''.join(h)


def render(cfg, rows, mem, bounds, vers, classes, T, title, passes=None):
    idx = {(x['cls'], x['page'], x['engine'], x['phase']): x for x in rows}
    others = [e for e in cfg.engines if cfg.engines[e].get('adapter') != 'ours']
    ours = next((e for e in cfg.engines if cfg.engines[e].get('adapter') == 'ours'), None)
    kinds = sorted({p.kind for p in cfg.pages.values()})
    H, SUM, WT, WM, WC = [], {}, [], [], []

    def plink(pid):
        """Page name linked to the file the engines opened, its edit variants and its source."""
        p = cfg.pages.get(pid)
        if not p:
            return html.escape(pid)
        out = f'<a href="{p.file("base.html").as_uri()}">{html.escape(pid)}</a>'
        vs = [f'<a href="{p.file(n).as_uri()}">{T[k]}</a>' for k, n in (('v_style', 'C.html'), ('v_content', 'D.html'), ('v_vis', 'vis.html')) if p.file(n).exists()]
        if p.meta.get('source_url'):
            vs.append(f'<a href="{html.escape(p.meta["source_url"])}">{T["source"]}</a>')
        return out + (f' <span class="vl">{" · ".join(vs)}</span>' if vs else '')
    for cls in classes:
        H.append(f'<h2 id="{cls}">{html.escape(CLASSES[cls][2])}</h2>')
        for kind in kinds:
            ids = [p.id for p in cfg.pages.values() if p.kind == kind]
            H.append(f'<h3>{html.escape(CLASSES[cls][2])} — {html.escape(kind)}</h3><div class="tw"><table><thead><tr><th>{T["page"]}</th>'
                     f'<th>{T["ph"]}</th><th>{T["ours"]}</th>' + ''.join(f'<th>{html.escape(e)}</th>' for e in others)
                     + f'<th>{T["best"]}</th><th>{T["ratio"]}</th><th>{T["note"]}</th></tr></thead><tbody>')
            for p in ids:
                for ph in M.PHASES:
                    o = idx.get((cls, p, ours, ph)) or {'empty': 'not_measured'}
                    ov = o.get('time_ms'); cells = []; comp = []
                    for e in others:
                        x = idx.get((cls, p, e, ph)) or {'empty': 'not_measured'}
                        v = x.get('time_ms')
                        if v is not None and x.get('within_noise'):  # shown, never the best: inside the bounds' noise
                            cells.append(f'<td class="num nd" title="{html.escape(x.get("note") or "")}">≈{f(v)}</td>'); continue
                        cells.append(f'<td class="num">{f(v)}</td>' if v is not None else f'<td class="why">{html.escape(why(x, T))}</td>')
                        if v and v > 0:
                            comp.append((v, e))
                    best = min(comp) if comp else None
                    ratio = ov / best[0] if best and ov and not o.get('invalid') else None
                    note = []
                    if o.get('note'):
                        note.append((T['invalid'] if o.get('invalid') else '') + o['note'])
                    if ov is not None and ov < 5 and len(o.get('repeats') or []) < 8:
                        note.append(T['fast'])
                    if ratio is not None:
                        SUM.setdefault((cls, kind, ph), []).append(ratio); WT.append((ratio, cls, kind, p, ph, ov, best))
                    elif ov is not None and not best:
                        note.append(T['none'])
                    oc = f'<td class="num">{f(ov)}</td>' if ov is not None else f'<td class="why">{html.escape(why(o, T))}</td>'
                    H.append(f'<tr><td>{plink(p)}</td><td>{T["phase"][ph]}</td>{oc}{"".join(cells)}<td>{html.escape(best[1]) if best else "—"}</td>'
                             f'<td class="num {col(ratio)}">{("×" + f(ratio)) if ratio else "—"}</td><td class="note">{html.escape("; ".join(note))}</td></tr>')
            H.append('</tbody></table></div>')
            H.append(f'<h4>{T["mem"]}</h4><div class="tw"><table><thead><tr><th>{T["page"]}</th><th>{T["ours"]}</th>'
                     + ''.join(f'<th>{html.escape(e)}</th>' for e in others) + f'<th>{T["best"]}</th><th>{T["gap"]}</th></tr></thead><tbody>')
            for p in ids:
                def mcell(e):
                    m = mem.get((cls, p, e))
                    if not m:
                        return None, f'<td class="why">{T["not_measured"]}</td>'
                    if not m.get('peak_bytes'):
                        return None, f'<td class="why">{html.escape(m.get("empty") or "no number")}</td>'
                    sd = f(m['steady_bytes'] / MB) if m.get('steady_bytes') else '—'
                    return m['peak_bytes'], f'<td class="num">{f(m["peak_bytes"] / MB)} / {sd}</td>'
                opk, oc = mcell(ours); cs = []; bp = []
                for e in others:
                    v, c = mcell(e); cs.append(c)
                    if v:
                        bp.append((v, e))
                b = min(bp) if bp else None; gap = (opk - b[0]) / MB if opk and b else None
                if gap is not None:
                    WM.append((gap, cls, kind, p, opk, b))
                H.append(f'<tr><td>{plink(p)}</td>{oc}{"".join(cs)}<td>{(html.escape(b[1]) + " " + f(b[0] / MB)) if b else "—"}</td>'
                         f'<td class="num {col(opk / b[0]) if gap is not None else "nd"}">{("%+.0f" % gap) if gap is not None else "—"}</td></tr>')
            H.append('</tbody></table></div>')
            H.append(f'<h4>{T["cpu"]}</h4><div class="tw"><table><thead><tr><th>{T["page"]}</th><th>{T["ph"]}</th><th>{T["ours"]}</th>'
                     + ''.join(f'<th>{html.escape(e)}</th>' for e in others) + f'<th>{T["ratio"]}</th></tr></thead><tbody>')
            for p in ids:
                for ph in M.PHASES:
                    oc = (idx.get((cls, p, ours, ph)) or {}).get('cpu_ms'); vs = []
                    for e in others:
                        vs.append(((idx.get((cls, p, e, ph)) or {}).get('cpu_ms'), e))
                    bb = min([v for v in vs if v[0]], default=None); rr = oc / bb[0] if oc and bb else None
                    if rr:
                        WC.append((rr, cls, kind, p, ph, oc, bb))
                    H.append(f'<tr><td>{plink(p)}</td><td>{T["phase"][ph]}</td><td class="num">{f(oc)}</td>'
                             + ''.join(f'<td class="num">{f(v)}</td>' for v, _ in vs) + f'<td class="num {col(rr)}">{("×" + f(rr)) if rr else "—"}</td></tr>')
            H.append('</tbody></table></div>')
    sm = [f'<h2 id="sum">{T["summary"]}</h2><div class="tw"><table><thead><tr><th>class</th><th>pages</th><th>{T["ph"]}</th><th>{T["rows"]}</th>'
          f'<th>{T["faster"]}</th><th>{T["median"]}</th><th>{T["worst"]}</th></tr></thead><tbody>']
    for (cls, kind, ph), r in SUM.items():
        sm.append(f'<tr><td>{html.escape(CLASSES[cls][2])}</td><td>{html.escape(kind)}</td><td>{T["phase"][ph]}</td><td class="num">{len(r)}</td>'
                  f'<td class="num">{sum(1 for x in r if x < 1)}</td><td class="num {col(M.med(r))}">×{f(M.med(r))}</td><td class="num {col(max(r))}">×{f(max(r))}</td></tr>')
    sm.append('</tbody></table></div>')
    if passes:
        sm.append(pass_table(passes, classes, T, ours))
    # Full lists of every comparable row, best for us first; no top-N cut.
    sm.append(f'<h2 id="all">{T["all"]}</h2>')
    for cls in classes:
        lists = (
            (T['worst_t'], 'ms', [(x[0], x[3], x[2], T['phase'][x[4]], x[5], x[6]) for x in WT if x[1] == cls]),
            (T['worst_m'], 'MB', [(x[4] / x[5][0], x[3], x[2], '', x[4] / MB, (x[5][0] / MB, x[5][1])) for x in WM if x[1] == cls]),
            (T['worst_c'], 'ms', [(x[0], x[3], x[2], T['phase'][x[4]], x[5], x[6]) for x in WC if x[1] == cls]),
        )
        for name, unit, items in lists:
            items.sort(key=lambda x: x[0])
            behind = sum(1 for x in items if x[0] > 1.05); first = sum(1 for x in items if x[0] < 0.95)
            cnt = T['count'].format(n=behind, m=first, k=len(items) - behind - first, t=len(items))
            sm.append(f'<h3>{html.escape(CLASSES[cls][2])}: {name}</h3><p class="note">{cnt}</p><div class="tw"><table><thead><tr>'
                      f'<th>{T["page"]}</th><th>pages</th><th>{T["ph"]}</th><th>{T["ours"]}, {unit}</th><th>{T["best"]}, {unit}</th>'
                      f'<th>{T["ratio"]}</th></tr></thead><tbody>' + ''.join(
                          f'<tr class="{col(x[0])}"><td>{plink(x[1])}</td><td>{html.escape(x[2])}</td><td>{x[3]}</td><td class="num">{f(x[4])}</td>'
                          f'<td class="num">{html.escape(x[5][1])} {f(x[5][0])}</td><td class="num {col(x[0])}">×{f(x[0])}</td></tr>' for x in items)
                      + '</tbody></table></div>')
    pg = [f'<h2 id="pages">{T["pages_title"]}</h2>']
    for kind in kinds:
        pg.append(f'<h3>{html.escape(kind)}</h3><div class="tw"><table><thead><tr><th>{T["page"]}</th><th>{T["about"]}</th><th>{T["size"]}</th></tr></thead><tbody>')
        for p in (x for x in cfg.pages.values() if x.kind == kind):
            pg.append(f'<tr><td>{plink(p.id)}</td><td>{html.escape(p.meta.get("about") or "")}</td><td class="num">{p.size() / 1024:.0f}</td></tr>')
        pg.append('</tbody></table></div>')
    causes = ''
    if cfg.causes and Path(cfg.causes).exists():
        causes = ''.join(f'<h3>{html.escape(c.stem)}</h3><pre class="cause">{html.escape(c.read_text(encoding="utf-8", errors="replace"))}</pre>'
                         for c in sorted(Path(cfg.causes).glob('*.md')))
    bnd = ''.join(f'<li><b>{html.escape(e)}</b>: {html.escape(str(b))}</li>' for e, b in bounds.items())
    ver = '; '.join(f'{html.escape(e)} {html.escape(str(v))}' for e, v in vers.items())
    leg = ''.join(f'<span class="{c}">{html.escape(t)}</span>' for c, t in zip('gnyor', T['legend']))
    return f"""<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Engine comparison</title><style>{CSS}</style></head><body>
<h1>{html.escape(title or T['title'])}</h1><p class="note">{html.escape(T['intro'])} {T['versions']}: {ver}.</p>
<p class="leg">{leg}</p><nav><a href="#sum">{T['summary']}</a><a href="#all">{T['all']}</a><a href="#pages">{T['pages_title']}</a><a href="#causes">{T['causes']}</a>{''.join(f'<a href="#{c}">{html.escape(CLASSES[c][2])}</a>' for c in classes)}</nav>
{''.join(sm)}{''.join(pg)}<h2 id="causes">{T['causes']}</h2>{causes or '<p class="note">—</p>'}
<h2>{T['bounds']}</h2><ul class="note">{bnd}</ul>{''.join(H)}</body></html>"""


CSS = """:root{--bg:#fff;--fg:#1c2024;--mut:#667085;--line:#e3e6ea;--head:#f4f6f8;--g:#d7f2dd;--n:#eef0f2;--y:#fff3c4;--o:#ffd9b0;--r:#ffc2c2}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#15181c;--fg:#e6e8eb;--mut:#98a2b3;--line:#2a3038;--head:#1e232a;--g:#17452a;--n:#252a31;--y:#4d4410;--o:#5c3a12;--r:#641f1f}}
:root[data-theme="dark"]{--bg:#15181c;--fg:#e6e8eb;--mut:#98a2b3;--line:#2a3038;--head:#1e232a;--g:#17452a;--n:#252a31;--y:#4d4410;--o:#5c3a12;--r:#641f1f}
body{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,Segoe UI,sans-serif;margin:0;padding:16px;max-width:1700px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:19px;margin:30px 0 8px}h3{font-size:16px;margin:22px 0 6px}h4{font-size:14px;margin:16px 0 4px}
.tw{overflow-x:auto;border:1px solid var(--line);border-radius:6px}table{border-collapse:collapse;width:100%;font-size:12.5px}
th,td{padding:3px 7px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}th{background:var(--head);position:sticky;top:0;white-space:nowrap}
.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}.note{color:var(--mut);font-size:12px}.why{color:var(--mut);font-size:11px;max-width:220px}
.g{background:var(--g)}.n{background:var(--n)}.y{background:var(--y)}.o{background:var(--o)}.r{background:var(--r)}.nd{color:var(--mut)}
nav a{margin-right:12px}.leg span{padding:1px 8px;border-radius:3px;margin-right:6px}td .vl{font-size:10.5px;color:var(--mut)}td .vl a{color:var(--mut)}pre.cause{white-space:pre-wrap;font:12px/1.4 ui-monospace,Consolas,monospace;background:var(--head);padding:8px;border-radius:6px}"""


def noise_verdict(a, b, floor=0.05):
    """('better'|'worse'|'noise', ratio) for repeats a (before) and b (after), lower is better.
    Outside the noise only when the repeat ranges do not overlap and the medians differ by more than the floor."""
    ma, mb = M.med(a), M.med(b)
    if not ma or not mb:
        return None, None
    r = mb / ma
    sep = max(b) < min(a) or min(b) > max(a)
    if len(a) < 2 or len(b) < 2:
        sep = True  # single values: only the floor decides
        floor = max(floor, 0.10)
    if not sep or abs(r - 1) <= floor:
        return 'noise', r
    return ('better' if r < 1 else 'worse'), r


def diff_cli(a):
    A_, B_ = (json.loads(Path(p).read_text(encoding='utf-8')) for p in (a.a, a.b))
    ix = lambda d: {(x['cls'], x['page'], x['phase']): x for x in d['rows'] if x['engine'] == a.engine}
    ra, rb = ix(A_), ix(B_); out = {'better': [], 'worse': []}; same = 0
    for k in sorted(set(ra) & set(rb)):
        x, y = ra[k], rb[k]
        va = x.get('repeats') or ([x['time_ms']] if x.get('time_ms') else []); vb = y.get('repeats') or ([y['time_ms']] if y.get('time_ms') else [])
        v, r = noise_verdict(va, vb)
        if v in out:
            out[v].append((r, k, M.med(va), M.med(vb)))
        elif v == 'noise':
            same += 1
    for v in ('worse', 'better'):
        lst = sorted(out[v], key=lambda t: -t[0] if v == 'worse' else t[0])
        print(f'{v}: {len(lst)}')
        for r, k, x, y in lst[:40]:
            print(f'  {k[0]} {k[1]} {k[2]}: {f(x)} -> {f(y)} ms (x{r:.2f})')
    print(f'within noise: {same}; only in A: {len(set(ra) - set(rb))}; only in B: {len(set(rb) - set(ra))}')
    return 0 if not out['worse'] else 1


if __name__ == '__main__':
    sys.exit('use engine_compare.py report|diff')
