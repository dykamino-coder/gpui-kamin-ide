"""Убрать из списка пар тесты, которым нужен JavaScript.

`target/L-all.txt` собирался без фильтра скрипта, и в базе сидят 2986 тестов со
`<script>` (34 из них — с `<meta name="variant">`: вариант применяется JS-ом,
без него тест и эталон рисуют дефолт и «сходятся», не проверив ничего).
`scripts/wpt_pairs.py` такие тесты отсеивает — этот скрипт доводит старый
список до той же политики.

Запуск: python scripts/wpt_prune_js.py target/L-all-v27.txt target/L-all-v28.txt
Печатает: сколько осталось, сколько убрано по каждой причине.
"""

import re
import sys
from pathlib import Path

VARIANT = re.compile(r'name=["\']variant["\']', re.I)
SCRIPT = re.compile(r'<script\b', re.I)


def main() -> None:
    src, dst = Path(sys.argv[1]), Path(sys.argv[2])
    kept: list[str] = []
    dropped = {'вариант': 0, 'скрипт': 0, 'нет файла': 0}
    for line in src.read_text(encoding='utf-8', errors='replace').splitlines():
        parts = line.split('|')
        if len(parts) < 2 or not parts[0].strip():
            continue
        test = Path(parts[0].strip())
        try:
            text = test.read_text(encoding='utf-8', errors='replace')
        except OSError:
            # Файла нет на этом коммите вендора — пара и так даст «снимок не
            # получен»; из списка не выкидываем, чтобы не менять знаменатель молча.
            dropped['нет файла'] += 1
            kept.append(line)
            continue
        if VARIANT.search(text):
            dropped['вариант'] += 1
            continue
        if SCRIPT.search(text):
            dropped['скрипт'] += 1
            continue
        kept.append(line)
    dst.write_text('\n'.join(kept) + '\n', encoding='utf-8')
    print(f'{src.name}: осталось {len(kept)}, убрано вариантных {dropped["вариант"]}, '
          f'скриптовых {dropped["скрипт"]}, без файла (оставлены) {dropped["нет файла"]}')


if __name__ == '__main__':
    main()
