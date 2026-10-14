"""Список пар «тест|эталон» для стенда — по ссылке `rel="match"` в самом тесте.

Раздел css-text именует эталон по тесту (`foo.html` → `reference/foo-ref.html`),
и там пары выводились из имени. В CSS2 так нельзя: десятки тестов ссылаются на
один общий эталон в `css/CSS2/reference/`, а имя ни о чём не говорит. Читаем
ссылку.

Читаются `.html`, `.xht`, `.xhtml` и отдельные `.svg` (ссылка `<h:link rel="match">`).

Отсеиваются:
  * тесты со скриптом — стенд их не исполняет;
  * `rel="mismatch"` — сравнение с АНТИ-эталоном, у стенда своя пометка;
  * кодировки и заголовки ответа (`at-charset`, `character-encoding`,
    `content-type`) — офлайн из файла они бессмысленны.

Запуск: python scripts/wpt_pairs.py <каталог> <файл-списка>
"""

import os
import re
import sys

LINK = re.compile(r'<link[^>]*\brel\s*=\s*["\']?match["\']?[^>]*>', re.I)
HREF = re.compile(r'\bhref\s*=\s*["\']([^"\']+)["\']', re.I)
SCRIPT = re.compile(r'<script\b', re.I)
# Отдельные `.svg`-тесты ссылаются на эталон XHTML-элементом с префиксом
# (`<h:link rel="match">`, `<html:link>`) или `<link xmlns="…xhtml">`;
# скрипт там тоже бывает с префиксом (`<h:script>`, `<svg:script>`).
SVG_LINK = re.compile(r'<(?:[\w-]+:)?link\b[^>]*\brel\s*=\s*["\']?match["\']?[^>]*>', re.I)
SVG_SCRIPT = re.compile(r'<(?:[\w-]+:)?script\b', re.I)
SKIP = ('at-charset', 'character-encoding', 'content-type')


def pairs(root):
    out = []
    for dirpath, _, names in os.walk(root):
        for name in sorted(names):
            svg = name.endswith('.svg')
            if not svg and not name.endswith(('.html', '.xht', '.xhtml')):
                continue
            if name.startswith(SKIP) or '-ref' in name or name.startswith('ref-'):
                continue
            path = os.path.join(dirpath, name)
            try:
                text = open(path, encoding='utf-8', errors='replace').read()
            except OSError:
                continue
            if (SVG_SCRIPT if svg else SCRIPT).search(text):
                continue
            tag = (SVG_LINK if svg else LINK).search(text)
            if not tag:
                continue
            href = HREF.search(tag.group(0))
            if not href:
                continue
            h = href.group(1)
            # Абсолютная ссылка (`/css/reference/...`) — от корня набора WPT.
            if h.startswith('/'):
                wpt_root = root
                while os.path.basename(wpt_root) != 'wpt-parsing' and os.path.dirname(wpt_root) != wpt_root:
                    wpt_root = os.path.dirname(wpt_root)
                ref = os.path.normpath(os.path.join(wpt_root, h.lstrip('/')))
            else:
                ref = os.path.normpath(os.path.join(dirpath, h))
            if os.path.exists(ref):
                out.append((os.path.abspath(path), os.path.abspath(ref)))
    return out


if __name__ == '__main__':
    got = pairs(sys.argv[1])
    with open(sys.argv[2], 'w', encoding='utf-8') as f:
        for test, ref in got:
            f.write(test + '|' + ref + '\n')
    print(len(got), 'пар в', sys.argv[2])
