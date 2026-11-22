"""report: one self-contained HTML page (+ rows.json for diff); diff: what got better or worse outside the noise.

Every cell has a number or the reason it has none. Ours is compared with the best other engine of the row.
"""
import html
import json
import re
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
           'worst_m': 'every page by peak memory, best for us first', 'worst_c': 'every row by CPU per frame, best for us first', 'h_cls': 'Class', 'h_engine': 'Engine', 'h_kind': 'Set', 'all': 'All comparisons', 'pass': 'Time to get through the whole page set', 'pass_note': 'Sum of unit walls per engine and class (lanes run in parallel, so this is machine time, not the run time). Goal: ours is the fastest. Ours is split into the stand (process launches, memory windows, fresh-frame checks, separate CPU build) and the engine (document load and phase frames). CPU-raster engines measure gpu2 once under soft2.', 'pass_total': 'Total', 'pass_page': 'Per page', 'pass_rank': 'Place', 'pass_stand': 'Stand', 'pass_engine': 'Engine itself', 'u_ms': 'ms', 'u_mb': 'MB', 'pass_pages': 'Pages', 'pass_missing': 'units not measured', 'count': 'behind (over 5%) in {n}, first in {m}, within 5% in {k}, of {t} comparable', 'causes': 'Causes and how to win',
           'page': 'Page', 'ph': 'Phase', 'ours': 'Ours', 'best': 'Best', 'ratio': 'Ours / best', 'note': 'Note',
           'rows': 'Rows', 'faster': 'We are faster', 'median': 'Median ours / best', 'worst': 'Worst',
           'mem': 'Memory, MB (peak / steady)', 'cpu': 'CPU per frame, ms', 'time': 'Frame time, ms',
           'no_scroll': 'page does not scroll', 'not_measured': 'not measured yet', 'none': 'no other engine has a number', 'fast': 'row under 5 ms: verdict needs 8 repeats',
           'invalid': 'not ranked: ', 'gap': 'Peak gap, MB', 'bounds': 'Measurement bounds', 'samples': 'samples: {n}', 'series_def': 'Repeats: every row has at least 3 values (ours: frames of one series; browsers: visits), then one more while it is not stable, up to the cap (ours: 500 ms of frames; browsers: 8 visits); stable = MAD / median <= 3 % (MAD = median absolute deviation from the median). The cell title gives the number of samples. Scroll keeps the browsers'' step count.', 'ours_fail': 'did not open / crashed', 'ours_failsum': 'We did not get through: {n} cells (time {t}, CPU {c}, memory {m}); the reason is on each red cell (hover).', 'cpu_none': 'the engine gives no CPU for this phase', 'm_time': 'time', 'm_cpu': 'CPU', 'm_peak': 'peak memory', 'm_steady': 'steady memory', 'cover': 'Coverage: engine x metric x phase', 'cover_note': 'Measured cells / all cells (every page and class). Green: all, yellow: part, red: none; the reasons of every empty cell are in the next table.', 'gaps': 'What is not measured and why', 'gaps_note': 'Every empty cell of every engine, grouped by engine, metric, phase and reason, most cells first.', 'metric': 'Metric', 'cells': 'Cells', 'reason': 'Reason', 'scroll_def': 'Scroll (S), every engine: from the top of the page, as many 120 px steps as the browser probe makes on that page: min(14, (scrollHeight - 600) // 120), down to one step; a step counts only when the root scroller moved by the whole 120 px; a page without one whole step has no S row.', 'loads': 'Foreign load on the P lanes at the start of every block (10 s; share of one core; the owner''s programs are left alone)', 'versions': 'Versions',
           'legend': ['we are faster than the best', 'within 5%', 'slower, up to x2', 'x2-x10', 'x10 and more'],
           'v_style': 'style edit', 'v_content': 'content edit', 'v_vis': 'visible edit', 'source': 'source',
           'pages_title': 'Pages under test', 'about': 'What it is', 'size': 'HTML+CSS, KB', 'local': 'local file',
           'intro': 'Frame time is from the frame request to finished pixels, ms (median of repeats); browsers take the '
                    'better of their two clocks. Viewport 800x600. Lanes run on equal performance cores in parallel.'},
    'ru': {'title': 'Наш движок против всех движков', 'phase': {'cold': 'холодная загрузка', 'R': 'смена ширины окна',
           'C': 'правка стиля, вне экрана', 'Cvis': 'правка стиля, видимая', 'D': 'правка содержимого, вне экрана',
           'Dvis': 'правка содержимого, видимая', 'S': 'шаг прокрутки (120 px)'},
           'summary': 'Сводка: где мы первые и где нет', 'worst_t': 'все строки по времени, от лучших для нас к худшим',
           'worst_m': 'все страницы по пику памяти, от лучших к худшим', 'worst_c': 'все строки по процессору на кадр, от лучших к худшим', 'cls': {'gpu4': 'GPU, 4 потока', 'gpu2': 'GPU, 2 ядра', 'soft2': 'софт-растр, 2 ядра', 'soft1': 'софт-растр, 1 ядро'}, 'h_cls': 'Класс', 'h_engine': 'Движок', 'h_kind': 'Набор', 'all': 'Все сравнения', 'pass': 'Время прохождения всего набора', 'pass_note': 'Сумма времени единиц по движку и классу (дорожки идут параллельно, это машинное время, а не длительность захода). Цель: наш движок проходит набор быстрее любого. Наш столбец разложен на стенд (запуски процессов, окна памяти, сверка со свежим кадром, отдельная сборка для CPU) и движок (загрузка документа и кадры фаз). Движки с растром на процессоре меряют gpu2 один раз в soft2.', 'pass_total': 'Всего', 'pass_page': 'На страницу', 'pass_rank': 'Место', 'pass_stand': 'Стенд', 'pass_engine': 'Сам движок', 'u_ms': 'мс', 'u_mb': 'МБ', 'pass_pages': 'Страниц', 'pass_missing': 'единиц не измерено', 'count': 'отстаём (больше 5 %) в {n}, первые в {m}, в пределах 5 % в {k}, из {t} сравнимых',
           'causes': 'Причины и способы обгона', 'page': 'Страница', 'ph': 'Фаза', 'ours': 'Мы', 'best': 'Лучший',
           'ratio': 'Мы / лучший', 'note': 'Пометка', 'rows': 'Строк', 'faster': 'Мы быстрее', 'median': 'Медиана мы / лучший',
           'worst': 'Худшая', 'mem': 'Память, МБ (пик / уст.)', 'cpu': 'Процессор на кадр, мс', 'time': 'Время кадра, мс',
           'no_scroll': 'страница не прокручивается', 'not_measured': 'ещё не измерено', 'none': 'ни один другой движок не дал числа',
           'fast': 'строка <5 мс: вердикт только при 8 повторах', 'invalid': 'не в счёт: ', 'gap': 'Разрыв пика, МБ',
           'bounds': 'Границы замера', 'samples': 'образцов: {n}', 'series_def': 'Повторы: у каждой строки не меньше 3 значений (у нас — кадры одной серии, у браузеров — визиты), затем ещё по одному, пока строка не устоялась, до потолка (у нас 500 мс кадров, у браузеров 8 визитов); устоялась = MAD / медиана <= 3 % (MAD — медиана модулей отклонений от медианы). Число образцов — в подсказке ячейки. Прокрутка сохраняет число шагов браузеров.', 'ours_fail': 'не открылась / упала', 'ours_failsum': 'Мы не прошли: {n} ячеек (время {t}, процессор {c}, память {m}); причина — во всплывающей подсказке каждой красной ячейки.', 'cpu_none': 'движок не даёт процессорного времени этой фазы', 'm_time': 'время', 'm_cpu': 'процессор', 'm_peak': 'пик памяти', 'm_steady': 'уст. память', 'cover': 'Покрытие: движок × метрика × фаза', 'cover_note': 'Измерено ячеек / всего ячеек (все страницы и классы). Зелёный — всё, жёлтый — часть, красный — ничего; причины каждой пустой ячейки — в следующей таблице.', 'gaps': 'Что не измерено и почему', 'gaps_note': 'Каждая пустая ячейка любого движка, по движку, метрике, фазе и причине, от больших групп к меньшим.', 'metric': 'Метрика', 'cells': 'Ячеек', 'reason': 'Причина', 'scroll_def': 'Прокрутка (S), одинаково у всех движков: от верха страницы столько шагов по 120 px, сколько делает браузерный пробник на этой странице: min(14, (scrollHeight − 600) // 120), вплоть до одного шага; шаг в счёт, только если корневой прокрутчик сдвинулся на целые 120 px; страница без одного целого шага строки S не имеет.', 'loads': 'Посторонняя нагрузка на P-дорожках в начале каждого блока (10 с; доля одного ядра; программы владельца не трогаются)', 'versions': 'Версии',
           'legend': ['мы быстрее лучшего', '±5 %', 'медленнее до ×2', '×2–10', '×10 и больше'],
           'v_style': 'правка стиля', 'v_content': 'правка содержимого', 'v_vis': 'видимая правка', 'source': 'источник',
           'pages_title': 'Проверяемые страницы', 'about': 'Что это', 'size': 'HTML+CSS, КБ', 'local': 'локальный файл',
           'intro': 'Время — кадр от запроса до готовых пикселей, мс (медиана повторов); у браузера берётся лучший из двух '
                    'способов отсчёта. Окно 800×600. Дорожки идут параллельно на равных производительных ядрах.'},
}


def cname(cls, T):
    return (T.get('cls') or {}).get(cls) or CLASSES[cls][2]


def f(x):
    return '—' if x is None else (f'{x:.0f}' if x >= 100 else f'{x:.1f}' if x >= 10 else f'{x:.2f}')


def col(r):
    if r is None:
        return 'nd'
    return 'g' if r < 0.95 else 'n' if r <= 1.05 else 'y' if r < 2 else 'o' if r < 10 else 'r'


def latest(store, t, version, part=None):
    """Newest record of the same build, page content, class and part under another definition (e.g. a quick run)."""
    best = None
    for p in (Path(store) / t.engine / t.cls / t.page.id).glob('*.json'):
        try:
            r = json.loads(p.read_text(encoding='utf-8'))
        except (OSError, ValueError):
            continue
        if r.get('engine_version') == version and r.get('page_hash') == t.page.hash and r.get('part') == (part or t.part):
            if best is None or r.get('started', 0) > best.get('started', 0):
                best = r
    return best


def collect(cfg, classes, engines=None):
    """rows: [{cls, page, kind, engine, phase, time_ms, repeats, cpu_ms, empty, note, invalid}], mem: {(cls, page, engine): {...}}"""
    names = engines or list(cfg.engines)
    ads = {n: A.load(n, cfg) for n in names}
    todo, cached = plan(ads, list(cfg.pages.values()), classes, A.Policy(), cfg.store)
    units = {}
    dirty = contamination(cfg)
    passes = {}  # (cls, engine) -> {'wall', 'engine', 'pages', 'units', 'missing'}
    for t in todo + cached:
        rec = REC.load(cfg.store, t.engine, t.cls, t.page.id, t.key) or latest(cfg.store, t, ads[t.engine].version())
        found = [rec]
        if not rec and t.part == 'all':  # stores of the two-unit ours plan (parts cold + warm) of the same build
            found = [r for r in (latest(cfg.store, t, ads[t.engine].version(), p) for p in ('cold', 'warm')) if r] or [None]
        units.setdefault((t.cls, t.page, t.engine), []).extend(found)
        ps = passes.setdefault((t.cls, t.engine), {'wall': 0.0, 'engine': 0.0, 'pages': set(), 'units': 0, 'missing': 0})
        for rec in found:
            add_pass(ps, rec, t, ads, dirty)
    rows, mem, bounds, vers = units_rows(classes, ads, units)
    return rows, mem, bounds, vers, passes


def add_pass(ps, rec, t, ads, dirty):
    """One unit's wall into the page-set pass of its engine and class; marks records of a contaminated window."""
    if not rec:
        ps['missing'] += 1; return
    ps['wall'] += rec.get('wall_s') or 0; ps['units'] += 1; ps['pages'].add(t.page.id)
    if ads[t.engine].e.get('adapter') == 'ours':  # frames the engine itself spent; the rest is the stand
        ps['engine'] += sum(sum(x for x in (r.get('repeats') or []) if x) for r in (rec.get('rows') or {}).values()) / 1000
    note = next((d['note'] for d in dirty if d['from'] <= (rec.get('started') or 0) < d['to']), None)
    if note and not rec.get('_dirty'):
        rec['_dirty'] = True
        for r in (rec.get('rows') or {}).values():
            r['note'] = '; '.join(x for x in (r.get('note'), note) if x)


def units_rows(classes, ads, units):
    """Report rows and memory of the collected unit records."""
    rows, mem, bounds, vers = [], {}, {}, {}
    shared = {}  # (cls, engine) -> measured class, for CPU-raster engines whose classes coincide
    for cls in classes:
        for n, ad in ads.items():
            c = canon(ad, cls)
            if c != cls:
                shared[(cls, n)] = c
                for (c2, p, e), recs in list(units.items()):
                    if c2 == c and e == n:
                        units[(cls, p, e)] = recs
    # resources the page snapshot lacks (reported by our stand): the same page for every engine, so every row says it
    unav = {p.id: r['unavailable_resources'] for (_, p, _), recs in units.items() for r in recs
            if r and r.get('unavailable_resources')}
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
            if unav.get(page.id):
                note = '; '.join(x for x in (note, f'{unav[page.id]} resources unavailable (failed fetches, the same page '
                                             'for every engine)') if x)
            if (cls, eng) in shared or any(k[1] == eng and v == cls for k, v in shared.items()):
                note = '; '.join(x for x in (note, 'engine rasters on the CPU: this class and ' + (shared.get((cls, eng)) or 'gpu2') + ' coincide, measured once') if x)
            rows.append(dict(base, phase=ph, time_ms=r.get('time_ms'), repeats=r.get('repeats') or [], cpu_ms=r.get('cpu_ms'),
                             cpu_empty=r.get('cpu_empty'), empty=r.get('empty'), note=note, invalid=r.get('invalid'), within_noise=r.get('within_noise')))
        if m['memory']:
            mem[(cls, page.id, eng)] = m['memory']
    return rows, mem, bounds, vers


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
    if e == M.NO_SCROLL:
        return T['no_scroll']
    return T['not_measured'] if e == 'not_measured' else (e or 'no number')


def pass_table(passes, classes, T, ours):
    """Time to get through the whole page set, per engine and class: goal row (ours must be the fastest)."""
    h = [f'<h3>{T["pass"]}</h3><p class="note">{html.escape(T["pass_note"])}</p><div class="tw"><table><thead><tr><th>{T["h_cls"]}</th>'
         f'<th>{T["h_engine"]}</th><th>{T["pass_total"]}</th><th>{T["pass_page"]}</th><th>{T["pass_rank"]}</th><th>{T["pass_stand"]}</th>'
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
            h.append(f'<tr class="{cl if e == ours else ""}"><td>{html.escape(cname(cls, T))}</td><td>{"<b>" + html.escape(e) + "</b>" if e == ours else html.escape(e)}</td>'
                     f'<td class="num">{v["wall"]:.0f} s</td><td class="num">{per:.1f} s</td><td class="num {cl}">{rank} / {len(es)}</td>'
                     f'<td class="num">{st}</td><td class="num">{en}</td><td class="num">{len(v["pages"])}{(" (+" + str(v["missing"]) + " " + T["pass_missing"] + ")") if v["missing"] else ""}</td></tr>')
    h.append('</tbody></table></div>')
    return ''.join(h)


def ours_failed(x):
    """Our cell that should hold a number: neither 'not measured yet' nor a page without the phase (nothing to scroll)."""
    return x.get('time_ms') is None and x.get('empty') not in ('not_measured', M.NO_SCROLL)


def cpu_why(x, T):
    """Reason of an empty CPU cell: the adapter's own, else the row's, else that the engine gives none for the phase."""
    if x.get('cpu_ms') is not None:
        return ''
    return x.get('cpu_empty') or (why(x, T) if x.get('time_ms') is None else T['cpu_none'])


def mem_why(m, key, T):
    if not m:
        return T['not_measured']
    if key == 'peak_bytes':
        return m.get('empty') or 'no number'
    return m.get('steady_empty') or m.get('empty') or 'no number'


def gap_list(rows, mem, T):
    """{(engine, metric, phase, reason): cells} for every empty cell of every engine."""
    out = {}

    def add(k):  # thread/process ids in panic lines would split one reason into many groups
        k = k[:3] + (re.sub(r'\((\d+)\)', '(…)', k[3]),)
        out[k] = out.get(k, 0) + 1
    seen = set()
    for x in rows:
        seen.add((x['cls'], x['page'], x['engine']))
        if x.get('time_ms') is None:
            add((x['engine'], T['m_time'], x['phase'], why(x, T)))
        if x.get('cpu_ms') is None:
            add((x['engine'], T['m_cpu'], x['phase'], cpu_why(x, T)))
    for k in seen:
        m = mem.get(k)
        for key, name in (('peak_bytes', T['m_peak']), ('steady_bytes', T['m_steady'])):
            if not (m or {}).get(key):
                add((k[2], name, '—', mem_why(m, key, T)))
    return out


def coverage(rows, mem, engines, T):
    """Engine x metric x phase: measured cells / all cells (every page and class)."""
    cols = [(T['m_time'], ph) for ph in M.PHASES] + [(T['m_cpu'], ph) for ph in M.PHASES] + [(T['m_peak'], '—'), (T['m_steady'], '—')]
    have, total = {}, {}
    seen = set()
    for x in rows:
        seen.add((x['cls'], x['page'], x['engine']))
        for metric, v in ((T['m_time'], x.get('time_ms')), (T['m_cpu'], x.get('cpu_ms'))):
            k = (x['engine'], metric, x['phase']); total[k] = total.get(k, 0) + 1; have[k] = have.get(k, 0) + (v is not None)
    for k in seen:
        m = mem.get(k) or {}
        for key, name in (('peak_bytes', T['m_peak']), ('steady_bytes', T['m_steady'])):
            kk = (k[2], name, '—'); total[kk] = total.get(kk, 0) + 1; have[kk] = have.get(kk, 0) + bool(m.get(key))
    h = [f'<h2 id="cover">{T["cover"]}</h2><p class="note">{html.escape(T["cover_note"])}</p><div class="tw"><table><thead><tr><th>{T["h_engine"]}</th>'
         + ''.join(f'<th>{html.escape(mt)}{(" " + ph) if ph != "—" else ""}</th>' for mt, ph in cols) + '</tr></thead><tbody>']
    for e in engines:
        cells = []
        for mt, ph in cols:
            n, a = have.get((e, mt, ph), 0), total.get((e, mt, ph), 0)
            c = 'nd' if not a else 'g' if n == a else 'r' if n == 0 else 'y'
            cells.append(f'<td class="num {c}">{n}/{a}</td>')
        h.append(f'<tr><td>{html.escape(e)}</td>{"".join(cells)}</tr>')
    h.append('</tbody></table></div>')
    return ''.join(h)


def gap_table(gaps, T):
    h = [f'<h2 id="gaps">{T["gaps"]}</h2><p class="note">{html.escape(T["gaps_note"])}</p><div class="tw"><table><thead><tr><th>{T["h_engine"]}</th>'
         f'<th>{T["metric"]}</th><th>{T["ph"]}</th><th>{T["cells"]}</th><th>{T["reason"]}</th></tr></thead><tbody>']
    for (e, mt, ph, why_), n in sorted(gaps.items(), key=lambda kv: (kv[0][0], -kv[1], kv[0][1], kv[0][2])):
        h.append(f'<tr><td>{html.escape(e)}</td><td>{html.escape(mt)}</td><td>{html.escape(ph)}</td><td class="num">{n}</td>'
                 f'<td>{html.escape(why_)}</td></tr>')
    h.append('</tbody></table></div>')
    return ''.join(h)


def render(cfg, rows, mem, bounds, vers, classes, T, title, passes=None):
    idx = {(x['cls'], x['page'], x['engine'], x['phase']): x for x in rows}
    others = [e for e in cfg.engines if cfg.engines[e].get('adapter') != 'ours']
    ours = next((e for e in cfg.engines if cfg.engines[e].get('adapter') == 'ours'), None)
    kinds = sorted({p.kind for p in cfg.pages.values()})
    H, SUM, WT, WM, WC = [], {}, [], [], []
    FAIL = {'time': 0, 'cpu': 0, 'mem': 0}

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
        H.append(f'<h2 id="{cls}">{html.escape(cname(cls, T))}</h2>')
        for kind in kinds:
            ids = [p.id for p in cfg.pages.values() if p.kind == kind]
            H.append(f'<h3>{html.escape(cname(cls, T))} — {html.escape(kind)}</h3><div class="tw"><table><thead><tr><th>{T["page"]}</th>'
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
                        cells.append(f'<td class="num" title="{T["samples"].format(n=len(x.get("repeats") or []))}">{f(v)}</td>' if v is not None else
                                     f'<td class="why" title="{html.escape(why(x, T))}">{html.escape(why(x, T))}</td>')
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
                    if ov is not None:
                        oc = f'<td class="num" title="{T["samples"].format(n=len(o.get("repeats") or []))}">{f(ov)}</td>'
                    elif ours_failed(o):  # we did not get through: red, the reason on hover
                        FAIL['time'] += 1
                        oc = f'<td class="r fail" title="{html.escape(why(o, T))}">{T["ours_fail"]}</td>'
                    else:
                        oc = f'<td class="why" title="{html.escape(why(o, T))}">{html.escape(why(o, T))}</td>'
                    H.append(f'<tr><td>{plink(p)}</td><td>{T["phase"][ph]}</td>{oc}{"".join(cells)}<td>{html.escape(best[1]) if best else "—"}</td>'
                             f'<td class="num {col(ratio)}">{("×" + f(ratio)) if ratio else "—"}</td><td class="note">{html.escape("; ".join(note))}</td></tr>')
            H.append('</tbody></table></div>')
            H.append(f'<h4>{T["mem"]}</h4><div class="tw"><table><thead><tr><th>{T["page"]}</th><th>{T["ours"]}</th>'
                     + ''.join(f'<th>{html.escape(e)}</th>' for e in others) + f'<th>{T["best"]}</th><th>{T["gap"]}</th></tr></thead><tbody>')
            for p in ids:
                def mcell(e):
                    m = mem.get((cls, p, e))
                    if not m or not m.get('peak_bytes'):
                        w = mem_why(m, 'peak_bytes', T)
                        if e == ours and m:
                            FAIL['mem'] += 1 + (not m.get('steady_bytes'))
                            return None, f'<td class="r fail" title="{html.escape(w)}">{T["ours_fail"]}</td>'
                        return None, f'<td class="why" title="{html.escape(w)}">{html.escape(w)}</td>'
                    if m.get('steady_bytes'):
                        sd = f(m['steady_bytes'] / MB)
                    else:
                        FAIL['mem'] += e == ours
                        sd = f'<span class="{"fail" if e == ours else "nd"}" title="{html.escape(mem_why(m, "steady_bytes", T))}">—</span>'
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
                    ox = idx.get((cls, p, ours, ph)) or {'empty': 'not_measured'}; oc = ox.get('cpu_ms'); vs = []
                    for e in others:
                        x = idx.get((cls, p, e, ph)) or {'empty': 'not_measured'}
                        vs.append((x.get('cpu_ms'), e, x))
                    bb = min([v for v in vs if v[0]], default=None); rr = oc / bb[0] if oc and bb else None
                    if rr:
                        WC.append((rr, cls, kind, p, ph, oc, bb))
                    if oc is not None:
                        ocell = f'<td class="num">{f(oc)}</td>'
                    elif ox.get('empty') not in ('not_measured', M.NO_SCROLL):
                        FAIL['cpu'] += 1
                        ocell = f'<td class="r fail" title="{html.escape(cpu_why(ox, T))}">{T["ours_fail"]}</td>'
                    else:
                        ocell = f'<td class="num nd" title="{html.escape(cpu_why(ox, T))}">—</td>'
                    H.append(f'<tr><td>{plink(p)}</td><td>{T["phase"][ph]}</td>{ocell}'
                             + ''.join(f'<td class="num">{f(v)}</td>' if v is not None else
                                       f'<td class="num nd" title="{html.escape(cpu_why(x, T))}">—</td>' for v, _, x in vs)
                             + f'<td class="num {col(rr)}">{("×" + f(rr)) if rr else "—"}</td></tr>')
            H.append('</tbody></table></div>')
    sm = [f'<h2 id="sum">{T["summary"]}</h2><div class="tw"><table><thead><tr><th>{T["h_cls"]}</th><th>{T["h_kind"]}</th><th>{T["ph"]}</th><th>{T["rows"]}</th>'
          f'<th>{T["faster"]}</th><th>{T["median"]}</th><th>{T["worst"]}</th></tr></thead><tbody>']
    for (cls, kind, ph), r in SUM.items():
        sm.append(f'<tr><td>{html.escape(cname(cls, T))}</td><td>{html.escape(kind)}</td><td>{T["phase"][ph]}</td><td class="num">{len(r)}</td>'
                  f'<td class="num">{sum(1 for x in r if x < 1)}</td><td class="num {col(M.med(r))}">×{f(M.med(r))}</td><td class="num {col(max(r))}">×{f(max(r))}</td></tr>')
    sm.append('</tbody></table></div>')
    total = sum(FAIL.values())
    line = T['ours_failsum'].format(n=total, t=FAIL['time'], c=FAIL['cpu'], m=FAIL['mem'])
    sm[0] = sm[0].replace('</h2>', f'</h2><p class="failsum {"r" if total else "g"}">{html.escape(line)}</p>', 1)
    sm.append(coverage(rows, mem, ([ours] if ours else []) + others, T))
    sm.append(gap_table(gap_list(rows, mem, T), T))
    if passes:
        sm.append(pass_table(passes, classes, T, ours))
    # Full lists of every comparable row, best for us first; no top-N cut.
    sm.append(f'<h2 id="all">{T["all"]}</h2>')
    for cls in classes:
        lists = (
            (T['worst_t'], T['u_ms'], [(x[0], x[3], x[2], T['phase'][x[4]], x[5], x[6]) for x in WT if x[1] == cls]),
            (T['worst_m'], T['u_mb'], [(x[4] / x[5][0], x[3], x[2], '', x[4] / MB, (x[5][0] / MB, x[5][1])) for x in WM if x[1] == cls]),
            (T['worst_c'], T['u_ms'], [(x[0], x[3], x[2], T['phase'][x[4]], x[5], x[6]) for x in WC if x[1] == cls]),
        )
        for name, unit, items in lists:
            items.sort(key=lambda x: x[0])
            behind = sum(1 for x in items if x[0] > 1.05); first = sum(1 for x in items if x[0] < 0.95)
            cnt = T['count'].format(n=behind, m=first, k=len(items) - behind - first, t=len(items))
            sm.append(f'<h3>{html.escape(cname(cls, T))}: {name}</h3><p class="note">{cnt}</p><div class="tw"><table><thead><tr>'
                      f'<th>{T["page"]}</th><th>{T["h_kind"]}</th><th>{T["ph"]}</th><th>{T["ours"]}, {unit}</th><th>{T["best"]}, {unit}</th>'
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
    bnd = f'<li>{html.escape(T["series_def"])}</li><li>{html.escape(T["scroll_def"])}</li>' + ''.join(f'<li><b>{html.escape(e)}</b>: {html.escape(str(b))}</li>' for e, b in bounds.items())
    try:  # foreign load seen at the start of every block (the gate's line in run.log)
        loads = [x.strip() for x in (Path(cfg.store) / 'run.log').read_text(encoding='utf-8').splitlines() if 'load check' in x]
    except OSError:
        loads = []
    if loads:
        bnd += f'<li><b>{T["loads"]}</b><ul>' + ''.join(f'<li>{html.escape(x)}</li>' for x in loads) + '</ul></li>'
    ver = '; '.join(f'{html.escape(e)} {html.escape(str(v))}' for e, v in vers.items())
    leg = ''.join(f'<span class="{c}">{html.escape(t)}</span>' for c, t in zip('gnyor', T['legend']))
    return f"""<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Engine comparison</title><style>{CSS}</style></head><body>
<h1>{html.escape(title or T['title'])}</h1><p class="note">{html.escape(T['intro'])} {T['versions']}: {ver}.</p>
<p class="leg">{leg}</p><nav><a href="#sum">{T['summary']}</a><a href="#all">{T['all']}</a><a href="#cover">{T['cover']}</a><a href="#gaps">{T['gaps']}</a><a href="#pages">{T['pages_title']}</a><a href="#causes">{T['causes']}</a>{''.join(f'<a href="#{c}">{html.escape(cname(c, T))}</a>' for c in classes)}</nav>
{''.join(sm)}{''.join(pg)}<h2 id="causes">{T['causes']}</h2>{causes or '<p class="note">—</p>'}
<h2>{T['bounds']}</h2><ul class="note">{bnd}</ul>{''.join(H)}</body></html>"""


CSS = """:root{--link:#0b57b8;--visited:#6a3fb0;--bg:#fff;--fg:#1c2024;--mut:#667085;--line:#e3e6ea;--head:#f4f6f8;--g:#d7f2dd;--n:#eef0f2;--y:#fff3c4;--o:#ffd9b0;--r:#ffc2c2}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--link:#8ab4f8;--visited:#c4a7f5;--bg:#15181c;--fg:#e6e8eb;--mut:#98a2b3;--line:#2a3038;--head:#1e232a;--g:#17452a;--n:#252a31;--y:#4d4410;--o:#5c3a12;--r:#641f1f}}
:root[data-theme="dark"]{--link:#8ab4f8;--visited:#c4a7f5;--bg:#15181c;--fg:#e6e8eb;--mut:#98a2b3;--line:#2a3038;--head:#1e232a;--g:#17452a;--n:#252a31;--y:#4d4410;--o:#5c3a12;--r:#641f1f}
body{background:var(--bg);color:var(--fg);font:14px/1.45 system-ui,Segoe UI,sans-serif;margin:0;padding:16px;max-width:1700px}
h1{font-size:22px;margin:0 0 4px}h2{font-size:19px;margin:30px 0 8px}h3{font-size:16px;margin:22px 0 6px}h4{font-size:14px;margin:16px 0 4px}
.tw{overflow-x:auto;border:1px solid var(--line);border-radius:6px}table{border-collapse:collapse;width:100%;font-size:12.5px}
th,td{padding:3px 7px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}th{background:var(--head);position:sticky;top:0;white-space:nowrap}
.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}.note{color:var(--mut);font-size:12px}.why{color:var(--mut);font-size:11px;max-width:220px}
.g{background:var(--g)}.n{background:var(--n)}.y{background:var(--y)}.o{background:var(--o)}.r{background:var(--r)}.nd{color:var(--mut)}
a{color:var(--link)}a:visited{color:var(--visited)}.fail{font-weight:600;cursor:help}.failsum{padding:6px 10px;border-radius:6px;font-weight:600}nav a{margin-right:12px}.leg span{padding:1px 8px;border-radius:3px;margin-right:6px}td .vl{font-size:10.5px;color:var(--mut)}td .vl a{color:var(--mut)}pre.cause{white-space:pre-wrap;font:12px/1.4 ui-monospace,Consolas,monospace;background:var(--head);padding:8px;border-radius:6px}"""


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
