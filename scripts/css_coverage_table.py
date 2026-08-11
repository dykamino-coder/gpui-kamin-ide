"""Таблица покрытия CSS в документацию — из реестра в коде.

Единственный источник правды о покрытии — `crates/html/src/coverage.rs`:
там же его проверяет тест. Документация от него отстаёт молча, поэтому
таблица в ней не пишется руками, а собирается этим скриптом.

    python scripts/css_coverage_table.py

Секция `id="tablica"` в `docs/html-css-mapping.html` перезаписывается целиком.
"""

import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = ROOT / "crates" / "html" / "src" / "coverage.rs"
DOC = ROOT / "docs" / "html-css-mapping.html"

MARK_START = '<h2 class="sec" id="tablica">'
MARK_END = '<h2 class="sec" id="fon">'


def parse() -> list[tuple[str, str, str, str]]:
    """Строки реестра: (раздел, свойство, вердикт, примечание)."""
    text = REGISTRY.read_text(encoding="utf-8")
    body = text[text.index("pub const PROPERTIES"): text.index("\n];")]
    section = ""
    out = []
    # Записи ищутся по всему тексту раздела, а не построчно: длинные rustfmt
    # переносит на несколько строк, а в образце значения бывают экранированные
    # кавычки (`"tnum" 1`) — построчный разбор терял ровно такие свойства.
    # У `part(...)` три поля: имя, образец и причина неполноты — в таблице
    # показывается именно причина, поэтому третье поле разбирается отдельно.
    entry = re.compile(
        r'\b(m|no|imp|part)\(\s*"([^"]+)",\s*"((?:[^"\\]|\\.)*)"'
        r'(?:\s*,\s*"((?:[^"\\]|\\.)*)")?'
    )
    for chunk in re.split(r"(// --- .+? -+)", body):
        head = re.match(r"// --- (.+?) -+$", chunk.strip())
        if head:
            section = head.group(1).strip()
            continue
        flat = " ".join(chunk.split())
        for kind, name, arg, why in entry.findall(flat):
            note = why if kind == "part" and why else arg
            out.append((section, name, kind, note.replace('\\"', '"').replace("\\ ", " ")))
    return out


def escape(text: str) -> str:
    """Значение из кода — в текст страницы."""
    return text.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


TAG = {
    "m": ('<span class="tag t-ok">перенесено</span>', "образец: <code>{}</code>"),
    "part": ('<span class="tag t-part">частично</span>', "{}"),
    "no": ('<span class="tag t-part">пустышка</span>', "{}"),
    "imp": ('<span class="tag t-no">нечем</span>', "{}"),
}


def render(rows: list[tuple[str, str, str, str]]) -> str:
    total = len(rows)
    mapped = sum(1 for r in rows if r[2] == "m")
    partial = sum(1 for r in rows if r[2] == "part")
    noop = sum(1 for r in rows if r[2] == "no")
    imp = total - mapped - noop - partial
    # Та же формула, что в тесте: частичное — половина.
    pct = (mapped + noop + partial * 0.5) * 100.0 / total

    html = [
        MARK_START + "Полная таблица свойств</h2>",
        "<p><b>Что это.</b> Каждое свойство CSS, встречающееся в разметке "
        "интерфейсов, с вердиктом. Таблица собирается из реестра в коде "
        "(<code>crates/html/src/coverage.rs</code>) скриптом "
        "<code>scripts/css_coverage_table.py</code> — руками её не правят.</p>",
        "<p><b>Зачем именно так.</b> Список поддержанного в документации "
        "устаревает молча. Здесь же за каждой строкой стоит тест: пометка "
        "«перенесено» означает, что образец значения проверенно меняет "
        "разрешённый стиль, иначе сборка падает.</p>",
        '<div class="metric">',
        f"  <div><b>{pct:.0f}%</b><span>покрытие CSS (частичное — за половину)</span></div>",
        f"  <div><b>{mapped}</b><span>совпадает с браузером</span></div>",
        f"  <div><b>{partial}</b><span>узнаваемо, но с оговоркой</span></div>",
        f"  <div><b>{noop + imp}</b><span>пустышек и невыразимого</span></div>",
        "</div>",
    ]

    section = None
    for sec, name, kind, arg in rows:
        if sec != section:
            if section is not None:
                html.append("</tbody></table>")
            section = sec
            html.append(f"<h3>{sec}</h3>")
            html.append("<table><thead><tr><th>Свойство</th><th>Статус</th>"
                        "<th>Примечание</th></tr></thead><tbody>")
        tag, note = TAG[kind]
        html.append(
            f"<tr><td><code>{escape(name)}</code></td><td>{tag}</td>"
            f"<td>{note.format(escape(arg)) if arg else ''}</td></tr>"
        )
    html.append("</tbody></table>")
    return "\n".join(html) + "\n\n"


def main() -> int:
    rows = parse()
    if not rows:
        print("реестр не разобрался")
        return 1
    doc = DOC.read_text(encoding="utf-8")
    start = doc.find(MARK_START)
    end = doc.index(MARK_END)
    if start == -1:
        start = end
    doc = doc[:start] + render(rows) + doc[end:]
    DOC.write_text(doc, encoding="utf-8")
    print(f"таблица обновлена: {len(rows)} свойств")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
