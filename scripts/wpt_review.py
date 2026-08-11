"""Стенд ручного разбора пар: наш снимок, наш эталон, снимок Chrome и оценка.

Числа стенда говорят «сошлось/не сошлось», но не говорят, ЧТО тест хочет и не
ошибся ли сам замер. Хуже того, стенд сравнивает нас С НАМИ ЖЕ: и тест, и
эталон рисует наш движок, поэтому пара, сломанная одинаково с обеих сторон,
выглядит зелёной. Поэтому здесь третья колонка — тот же тест, снятый настоящим
Chrome (`scripts/wpt_chrome_shots.py`): вот он и есть честный эталон.

Пары раскладываются по одной: слева наш снимок теста, посередине наш эталон,
справа Chrome, под ними — всё словесное, что в тесте есть (заголовок,
`assert`, видимая инструкция, ссылки на спецификацию), кнопки оценки и поле
для заметки своими словами.

    python scripts/wpt_review.py target/wpt-report-text-v1.txt
    # откроется target/wpt-review.html

Оценки хранятся в самом браузере и выгружаются кнопкой в JSON, чтобы потом
разбирать их по списку. Снимки берутся из `target/wpt-shots` — их пишет
прогон с `WPT_DUMP=1`.
"""

import html
import json
import re
import sys
from pathlib import Path

SHOTS = Path("target/wpt-shots")
CHROME = Path("target/wpt-chrome")
OUT = Path("target/wpt-review.html")
THRESHOLD = 0.5

VERDICTS = [
    ("pass", "прошёл"),
    ("fail", "не прошёл"),
    ("maybe", "возможно не прошёл"),
    ("blank", "пустой экран"),
    ("unclear", "непонятно, что проверять"),
]


def strip_tags(raw: str) -> str:
    """Видимый текст: без служебных блоков и разметки."""
    raw = re.sub(r"(?is)<(script|style|title)[^>]*>.*?</\1>", " ", raw)
    raw = re.sub(r"(?s)<!--.*?-->", " ", raw)
    raw = re.sub(r"(?s)<[^>]+>", " ", raw)
    return re.sub(r"\s+", " ", html.unescape(raw)).strip()


def brief(path: Path) -> dict:
    """Что тест словами говорит о себе."""
    try:
        src = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return {"title": "", "assert": "", "text": "", "help": []}
    title = re.search(r"(?is)<title[^>]*>(.*?)</title>", src)
    assertion = re.search(
        r"""(?is)<meta[^>]+name=['"]?assert['"]?[^>]+content=['"](.*?)['"]""", src
    )
    # Инструкция автора обычно лежит в самом начале тела: «Test passes if…».
    visible = strip_tags(src)
    return {
        "title": html.unescape(title.group(1)).strip() if title else "",
        "assert": html.unescape(assertion.group(1)).strip() if assertion else "",
        "text": visible[:400],
        "help": re.findall(r"""(?i)rel=['"]?help['"]?[^>]*href=['"]([^'"]+)['"]""", src),
    }


def rows(report: Path) -> list[dict]:
    out = []
    for line in report.read_text(encoding="utf-8").splitlines():
        parts = line.split("|")
        if len(parts) < 3:
            continue
        test = Path(parts[0])
        stem = test.stem
        try:
            value = float(parts[-1])
            verdict = f"{value:.2f}%"
            green = value <= THRESHOLD
        except ValueError:
            verdict = parts[-1]
            green = False
        # Своя оценка, проставленная заранее: порог стенда плюс отдельная
        # пометка для нерисующейся страницы. Человеку остаётся спорить с ней,
        # а не выносить вердикт с нуля.
        if verdict.endswith("%"):
            mine = "pass" if green else "fail"
        else:
            mine = "blank"
        out.append(
            {
                "mine": mine,
                "name": test.name,
                "path": str(test),
                "ref": Path(parts[1]).name,
                "verdict": verdict,
                "green": green,
                # Путь АБСОЛЮТНЫЙ: страницу открывают и из другой папки
                # (скачанную копию), а относительный путь там указывает в
                # пустоту — картинки молча не грузятся.
                "shot": SHOTS.resolve().joinpath(f"{stem}.png").as_uri(),
                "shot_ref": SHOTS.resolve().joinpath(f"{stem}--ref.png").as_uri(),
                # Снимок настоящего браузера: он же честный эталон. Может не
                # быть — тогда колонка покажет, чем его снять.
                "shot_chrome": CHROME.resolve().joinpath(f"{stem}.png").as_uri(),
                "has_chrome": (CHROME / f"{stem}.png").is_file(),
                **brief(test),
            }
        )
    return out


PAGE = """<!doctype html>
<html lang="ru"><meta charset="utf-8">
<title>Разбор пар WPT</title>
<style>
 body{margin:0;font:14px/1.5 system-ui;background:#1a1c23;color:#e6e6ee}
 header{position:sticky;top:0;background:#23252f;padding:10px 16px;display:flex;
   gap:16px;align-items:center;border-bottom:1px solid #3d3f51}
 h1{font-size:15px;margin:0;font-weight:600}
 .shots{display:flex;gap:16px;padding:16px}
 .shots figure{margin:0;flex:1}
 .shots img{width:100%;border:1px solid #3d3f51;background:#fff}
 figcaption{font-size:12px;color:#9aa0b4;margin-bottom:6px}
 .body{padding:0 16px 24px}
 dl{display:grid;grid-template-columns:max-content 1fr;gap:4px 12px;margin:0 0 12px}
 dt{color:#9aa0b4}
 dd{margin:0}
 .none{color:#9aa0b4;font-style:italic}
 .marks{display:flex;gap:8px;flex-wrap:wrap;margin:12px 0}
 .marks button{background:#3d3f51;color:#e6e6ee;border:1px solid #4a4a5a;
   border-radius:6px;padding:6px 12px;cursor:pointer;font:inherit}
 .marks button[aria-pressed=true]{background:#8ab4f8;color:#1a1c23;font-weight:600}
 .num{font-variant-numeric:tabular-nums}
 a{color:#8ab4f8}
 code{background:#23252f;padding:1px 5px;border-radius:4px}
 .note{display:block;margin:12px 0 0;color:#9aa0b4}
 .note textarea{display:block;width:100%;margin-top:6px;box-sizing:border-box;
   background:#23252f;color:#e6e6ee;border:1px solid #3d3f51;border-radius:6px;
   padding:8px;font:inherit;resize:vertical}
 .note textarea:focus{outline:2px solid #8ab4f8;outline-offset:1px}
 .noted{color:#ffd479}
</style>
<header>
 <h1>Разбор пар WPT</h1>
 <span class="num" id="pos"></span>
 <span id="score"></span>
 <button id="prev">← назад</button>
 <button id="next">вперёд →</button>
 <button id="save">выгрузить оценки</button>
 <button id="only">только спорные</button>
 <span id="counts"></span>
</header>
<section class="shots">
 <figure><figcaption>наш тест</figcaption><img id="a" alt="снимок теста"></figure>
 <figure><figcaption>наш эталон</figcaption><img id="b" alt="снимок эталона"></figure>
 <figure>
  <figcaption>Chrome — как должно быть</figcaption>
  <img id="c" alt="снимок Chrome">
  <p class="none" id="c-none" hidden>снимка нет:
   <code>python scripts/wpt_chrome_shots.py &lt;список пар&gt;</code></p>
 </figure>
</section>
<div class="body">
 <dl>
  <dt>моя оценка</dt><dd id="mine"></dd>
  <dt>файл</dt><dd id="name"></dd>
  <dt>эталон</dt><dd id="ref"></dd>
  <dt>заголовок</dt><dd id="title"></dd>
  <dt>что проверяет</dt><dd id="assert"></dd>
  <dt>видимый текст</dt><dd id="text"></dd>
  <dt>спецификация</dt><dd id="help"></dd>
 </dl>
 <div class="marks" id="marks"></div>
 <label class="note">что не так — своими словами
  <textarea id="note" rows="4" placeholder="например: у нас вторая коробка на 20 точек ниже, в Chrome они впритык"></textarea>
 </label>
</div>
<script>
const DATA = __DATA__;
const VERDICTS = __VERDICTS__;
const marks = JSON.parse(localStorage.getItem('wpt-marks') || '{}');
// Заметки словами: то, чего кнопками не скажешь. Хранятся рядом с оценками и
// выгружаются вместе с ними.
const notes = JSON.parse(localStorage.getItem('wpt-notes') || '{}');
const NAMES = Object.fromEntries(VERDICTS);
let at = 0;
let onlyDisputed = false;

// Спорная пара — та, где оценка человека уже стоит и не совпала с моей.
const disputed = (row) => marks[row.name] && marks[row.name] !== row.mine;
const visible = () => (onlyDisputed ? DATA.filter(disputed) : DATA);

const set = (id, value, fallback) => {
  const node = document.getElementById(id);
  node.textContent = value || '';
  node.className = value ? '' : 'none';
  if (!value) node.textContent = fallback;
};

function draw() {
  const list = visible();
  if (!list.length) { onlyDisputed = false; }
  const rows = visible();
  at = Math.min(at, rows.length - 1);
  const row = rows[at];
  document.getElementById('pos').textContent = `${at + 1} / ${rows.length}`;
  const mine = document.getElementById('mine');
  mine.textContent = NAMES[row.mine];
  mine.style.color = row.mine === 'pass' ? '#7bd88f' : '#ff7b72';
  document.getElementById('score').textContent = row.verdict;
  document.getElementById('a').src = row.shot;
  document.getElementById('b').src = row.shot_ref;
  const chrome = document.getElementById('c');
  chrome.hidden = !row.has_chrome;
  document.getElementById('c-none').hidden = row.has_chrome;
  if (row.has_chrome) chrome.src = row.shot_chrome;
  const note = document.getElementById('note');
  note.value = notes[row.name] || '';
  set('name', row.name, '');
  set('ref', row.ref, '');
  set('title', row.title, 'заголовка нет');
  set('assert', row.assert, 'словесного утверждения нет — сравниваются снимки');
  set('text', row.text, 'видимого текста нет');
  const help = document.getElementById('help');
  help.innerHTML = row.help.length
    ? row.help.map((h) => `<a href="${h}" target="_blank">${h}</a>`).join('<br>')
    : '<span class="none">ссылок нет</span>';
  const box = document.getElementById('marks');
  box.innerHTML = '';
  for (const [key, label] of VERDICTS) {
    const b = document.createElement('button');
    b.textContent = label;
    b.setAttribute('aria-pressed', marks[row.name] === key);
    b.onclick = () => { marks[row.name] = key; store(); draw(); };
    box.appendChild(b);
  }
  const done = Object.keys(marks).length;
  const spor = DATA.filter(disputed).length;
  const said = Object.values(notes).filter((t) => t.trim()).length;
  document.getElementById('counts').textContent =
    `оценено ${done} из ${DATA.length}, расходимся в ${spor}, заметок ${said}`;
}

function store() {
  localStorage.setItem('wpt-marks', JSON.stringify(marks));
  localStorage.setItem('wpt-notes', JSON.stringify(notes));
}

// Заметка сохраняется на каждый знак: перелистнуть страницу и потерять
// написанное нельзя.
document.getElementById('note').addEventListener('input', (e) => {
  const row = visible()[at];
  if (!row) return;
  notes[row.name] = e.target.value;
  store();
});
function go(step) {
  const n = visible().length;
  at = (at + step + n) % n;
  draw();
}

document.getElementById('only').onclick = () => {
  onlyDisputed = !onlyDisputed;
  at = 0;
  draw();
};
document.getElementById('prev').onclick = () => go(-1);
document.getElementById('next').onclick = () => go(1);
document.getElementById('save').onclick = () => {
  // Выгружается ПАРА «оценка + заметка» на каждый разобранный тест, иначе
  // словесное придётся сводить с числами вручную.
  const out = {};
  for (const row of DATA) {
    if (marks[row.name] || (notes[row.name] || '').trim()) {
      out[row.name] = {
        verdict: marks[row.name] || null,
        note: (notes[row.name] || '').trim() || null,
        mine: row.mine,
        score: row.verdict,
        path: row.path,
      };
    }
  }
  const blob = new Blob([JSON.stringify(out, null, 2)], {type: 'application/json'});
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = 'wpt-marks.json';
  a.click();
};
addEventListener('keydown', (e) => {
  // Пока пишут заметку, клавиши принадлежат полю, а не листалке.
  if (e.target.tagName === 'TEXTAREA') return;
  if (e.key === 'ArrowLeft') go(-1);
  if (e.key === 'ArrowRight') go(1);
  const n = Number(e.key);
  if (n >= 1 && n <= VERDICTS.length) {
    marks[visible()[at].name] = VERDICTS[n - 1][0];
    store();
    draw();
  }
});
draw();
</script>
</html>
"""


def main() -> None:
    if len(sys.argv) < 2:
        print("нужен путь к отчёту", file=sys.stderr)
        raise SystemExit(1)
    data = rows(Path(sys.argv[1]))
    if not data:
        print("в отчёте нет строк", file=sys.stderr)
        raise SystemExit(1)
    missing = sum(
        1 for r in data if not (SHOTS / Path(r["shot"]).name).is_file()
    )
    page = PAGE.replace("__DATA__", json.dumps(data, ensure_ascii=False))
    page = page.replace("__VERDICTS__", json.dumps(VERDICTS, ensure_ascii=False))
    OUT.write_text(page, encoding="utf-8")
    print(f"{OUT}: пар {len(data)}, без снимков {missing}")
    if missing:
        print("снимки пишет прогон с WPT_DUMP=1", file=sys.stderr)


main()
