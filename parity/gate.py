# -*- coding: utf-8 -*-
"""Гейт визуальной сверки: «готово» = ВСЕ 159 элементов в MATCH.

Запуск:  python parity/gate.py            — сводка + код возврата
         python parity/gate.py --list     — плюс список расхождений
         python parity/gate.py --json     — машиночитаемый REPORT.json

Код возврата 0 только когда каждый элемент в последнем цикле получил MATCH.
Пока есть хоть один DIVERGES — 1, то есть парити НЕ закрыт, и это нельзя
объявить завершённым ни в отчёте, ни в релизе.
"""
import io
import json
import os
import re
import sys

ZONES = [
    (1, 19, "Титлбар"),
    (20, 37, "Сайдбар"),
    (38, 51, "Activity, рейлы, стрипы"),
    (52, 91, "Панели и экраны"),
    (92, 107, "Дерево файлов"),
    (108, 129, "Редактор, оверлеи, статус"),
    (130, 159, "Токены, семплы, глобальные стили"),
]

# Пометка в скобках («(правка)», «(дополнение)», «(баг от пользователя)») —
# часть заголовка, а не брак: без неё разбор терял ПОСЛЕДНИЙ блок и в зачёт
# шёл предыдущий цикл. Так 57 досье жили со стухшим вердиктом (ц.34)
HEAD = re.compile(r"^## Цикл (\d+)(?: \([^)]*\))?: (MATCH|DIVERGES).*$", re.M)
# Заголовок, который ВЫГЛЯДИТ вердиктом, но не разобрался: раньше такой блок
# молча игнорировался и в зачёт шёл ПРЕДЫДУЩИЙ цикл (поймано на 117, где
# «DIVERGES (осознанное отступление)» держал элемент в MATCH целый прогон)
HEAD_LOOSE = re.compile(r"^## Цикл .*$", re.M)


def last_verdict(path: str) -> tuple[str, str]:
    """(вердикт, цикл) последнего блока в verdict.md."""
    if not os.path.exists(path):
        return "—", "—"
    text = io.open(path, encoding="utf-8").read()
    heads = HEAD.findall(text)
    bad = [h for h in HEAD_LOOSE.findall(text) if not HEAD.match(h)]
    if bad:
        print(f"[!] {path}: неразобранные заголовки вердикта: {bad}")
    if not heads:
        return "—", "—"
    cyc, verd = heads[-1]
    return verd.strip(), cyc


# «Осталось: …» последнего блока — то, что цикл НЕ закрыл. Пустой список у
# DIVERGES означает, что вердикт не назвал причину, и это само по себе брак.
OPEN_RE = re.compile("^Осталось[^:]*:\\s*(.+?)(?=\\n\\n|\\Z)", re.M | re.S)


def open_items(path: str) -> list[str]:
    if not os.path.exists(path):
        return []
    text = io.open(path, encoding="utf-8").read()
    heads = list(HEAD.finditer(text))
    if not heads:
        return []
    tail = text[heads[-1].end():]
    hit = OPEN_RE.search(tail)
    if not hit:
        return []
    body = " ".join(hit.group(1).split())
    return [x.strip(" .") for x in body.split(";") if x.strip(" .")]


def main() -> int:
    root = os.path.dirname(os.path.abspath(__file__))
    dirs = sorted(
        (d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d)) and d[0].isdigit()),
        key=lambda d: int(d.split("-")[0]),
    )
    rows = []
    same_shots = []
    for d in dirs:
        verd, cyc = last_verdict(os.path.join(root, d, "verdict.md"))
        num = int(d.split("-")[0])
        paths = [os.path.join(root, d, f) for f in ("original.png", "ours.png")]
        # `NO_SHOT.md` — элемент НЕВИЗУАЛЬНЫЙ (классы на body, глобальный CSS):
        # кадр ему нечего показывать, и вечный пробел в метрике только маскирует
        # реальные пропуски. Файл обязан объяснять причину (ревью ц.24).
        no_shot = os.path.join(root, d, "NO_SHOT.md")
        if os.path.isfile(no_shot):
            rows.append((num, d, verd, cyc, True))
            continue
        has_shots = all(os.path.exists(p) for p in paths)
        # Пара из ДВУХ ОДИНАКОВЫХ файлов — не пара: так в зоне сайдбара семь
        # элементов держали один и тот же кадр sessions-列表 вместо поповера,
        # rename-инпута и свотчей, а гейт считал их проверенными (ревью ц.9).
        if has_shots:
            blobs = [io.open(p, "rb").read() for p in paths]
            if blobs[0] == blobs[1]:
                has_shots = False
                same_shots.append(num)
        rows.append((num, d, verd, cyc, has_shots))

    # Один и тот же кадр в РАЗНЫХ досье — не поэлементный скрин, а общий кадр
    # окна: досье формально «с парой», а сверять по нему нечего. Раньше гейт
    # сравнивал только original↔ours ВНУТРИ досье и такие группы не видел
    # (ревью ц.25: 14 групп, целые зоны на одном кадре).
    import hashlib

    by_hash: dict[str, list[int]] = {}
    for d in dirs:
        pth = os.path.join(root, d, "ours.png")
        if os.path.isfile(pth):
            h = hashlib.md5(io.open(pth, "rb").read()).hexdigest()
            by_hash.setdefault(h, []).append(int(d.split("-")[0]))
    shared_shots = sorted(
        (v for v in by_hash.values() if len(v) > 1), key=lambda v: -len(v)
    )

    # Досье с ОБЩИМ кадром парой кадров не считается: сверять по кропу чужой
    # зоны нечего. Иначе метрика «без пары кадров» врёт (ревью ц.26).
    shared_ids = {i for g in shared_shots for i in g}
    rows = [(n, d, v, c, has and n not in shared_ids) for n, d, v, c, has in rows]

    match = [r for r in rows if r[2].startswith("MATCH")]
    diverges = [r for r in rows if r[2].startswith("DIVERGES")]
    unjudged = [r for r in rows if r[2] == "—"]
    no_shots = [r for r in rows if not r[4]]

    if "--json" in sys.argv:
        report = {
            "total": len(rows),
            "match": len(match),
            "diverges": len(diverges),
            "unjudged": len(unjudged),
            "no_shots": [r[0] for r in no_shots],
            "shared_shots": shared_shots,
            "closed": not diverges and not unjudged,
            "elements": [
                {
                    "id": num,
                    "slug": d.split("-", 1)[1],
                    "verdict": verd,
                    "cycle": cyc,
                    "shots": has,
                    "open": open_items(os.path.join(root, d, "verdict.md")),
                }
                for num, d, verd, cyc, has in rows
            ],
        }
        out = os.path.join(root, "REPORT.json")
        io.open(out, "w", encoding="utf-8").write(
            json.dumps(report, ensure_ascii=False, indent=2)
        )
        print(f"REPORT.json: {report['match']}/{report['total']} MATCH, "
              f"закрыт={report['closed']}")
        return 0 if report["closed"] else 1

    print(f"элементов: {len(rows)}")
    print(f"  MATCH:     {len(match)}")
    print(f"  DIVERGES:  {len(diverges)}")
    print(f"  без вердикта: {len(unjudged)}")
    print(f"  без пары кадров: {len(no_shots)}")
    if shared_shots:
        n = sum(len(g) for g in shared_shots)
        print(f"  общий кадр вместо поэлементного: {n} досье в {len(shared_shots)} группах")
    print()
    for lo, hi, title in ZONES:
        z = [r for r in rows if lo <= r[0] <= hi]
        if not z:
            continue
        zm = sum(1 for r in z if r[2].startswith("MATCH"))
        cyc = max((int(r[3]) for r in z if r[3].isdigit()), default=0)
        print(f"  {lo:>3}-{hi:<3} {title:<34} {zm:>3}/{len(z):<3} MATCH   последний цикл: {cyc}")

    if "--list" in sys.argv and diverges:
        print("\nрасхождения:")
        for num, d, verd, cyc, _ in diverges:
            print(f"  {num:>3} {d}  (ц.{cyc})")

    if diverges or unjudged:
        print(
            f"\nГЕЙТ ЗАКРЫТ: {len(diverges)} расхождений, {len(unjudged)} без вердикта. "
            "Парити объявлять завершённым нельзя."
        )
        return 1
    if no_shots:
        print(
            f"\nГЕЙТ ЗАКРЫТ: у {len(no_shots)} элементов нет пары кадров — "
            "вердикт без кадров не считается подтверждённым."
        )
        return 1
    print("\nГЕЙТ ОТКРЫТ: все элементы в MATCH и с парой кадров.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
