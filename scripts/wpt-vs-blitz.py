# Сводка «мы vs Blitz» по семействам CSS.
# Вход: target/wpt-report.txt (наш свод: тест|эталон|вердикт, зелёный <= 0.5)
#   + вшитая таблица Blitz reftest-процентов (срез 2026-08-19, plan/handbook-drafts/blitz-families.md).
# Выход: таблица в stdout, сортировка по нашему отставанию.
import re, sys
from collections import defaultdict

REPORT = sys.argv[1] if len(sys.argv) > 1 else "target/wpt-report.txt"

# Blitz reftest pass/total (из сырого wptreport + WPT MANIFEST, см. blitz-families.md)
BLITZ = {
    "css-grid": (588, 1568), "css-flexbox": (729, 996), "css-text": (682, 1466),
    "css-writing-modes": (140, 1139), "css-backgrounds": (461, 712),
    "css-borders": (19, 82), "css-color": (278, 307), "css-overflow": (124, 517),
    "css-tables": (57, 164), "css-position": (50, 248), "css-multicol": (63, 463),
    "css-transforms": (393, 789), "css-images": (130, 460), "css-fonts": (182, 371),
    "css-ui": (897, 1002), "css-shadow": (7, 112), "css-content": (5, 63),
    "css-gaps": (12, 367), "css-counter-styles": (8, 240), "css-lists": (52, 168),
    "css-display": (68, 213), "css-align": (31, 62), "css-sizing": (291, 559),
    "css-values": (106, 209), "selectors": (37, 226), "css-pseudo": (81, 260),
    "css-masking": (162, 370), "css-shapes": (27, 231), "css-variables": (178, 182),
    "css-cascade": (30, 43), "CSS2": (3809, 6170), "css-inline": (15, 150),
    "css-break": (84, 958), "css-text-decor": (86, 302), "css-logical": (2, 6),
    "css-box": (4, 67), "css-contain": (233, 428), "filter-effects": (147, 328),
}

fam_re = re.compile(r"[\\/]css[\\/]([^\\/]+)[\\/]")
ours = defaultdict(lambda: [0, 0])  # family -> [green, total]
for line in open(REPORT, encoding="utf-8", errors="replace"):
    parts = line.strip().split("|")
    if len(parts) < 3:
        continue
    m = fam_re.search(parts[0])
    fam = m.group(1) if m else "other"
    try:
        verdict = float(parts[2])
    except ValueError:
        continue
    ours[fam][1] += 1
    if verdict <= 0.5:
        ours[fam][0] += 1

rows = []
for fam, (g, t) in sorted(ours.items()):
    our_pct = 100.0 * g / t if t else 0.0
    b = BLITZ.get(fam)
    if b:
        b_pct = 100.0 * b[0] / b[1]
        delta = our_pct - b_pct
        rows.append((delta, fam, g, t, our_pct, b[0], b[1], b_pct))
    else:
        rows.append((None, fam, g, t, our_pct, None, None, None))

print(f"{'семейство':28} {'мы':>12} {'мы%':>6}  {'Blitz':>12} {'Blitz%':>6} {'Δ':>7}")
for delta, fam, g, t, op, bg, bt, bp in sorted(rows, key=lambda r: (r[0] is None, r[0] if r[0] is not None else 0)):
    b_s = f"{bg}/{bt}" if bg is not None else "—"
    bp_s = f"{bp:.1f}" if bp is not None else "—"
    d_s = f"{delta:+.1f}" if delta is not None else "—"
    print(f"{fam:28} {g:>5}/{t:<6} {op:>5.1f}  {b_s:>12} {bp_s:>6} {d_s:>7}")

tg = sum(v[0] for v in ours.values()); tt = sum(v[1] for v in ours.values())
print(f"\nИТОГО (наш срез): {tg}/{tt} = {100.0*tg/tt:.1f}%")
print("Blitz reftest-итог по css/: 11191/24092 = 46.4% (полный WPT-знаменатель)")
print("ОГОВОРКА: знаменатели разные — наш срез это НАША выборка reftest-пар,")
print("Blitz гоняет все reftests ревизии a95401e4e. Δ корректна как порядок, не как точность.")
