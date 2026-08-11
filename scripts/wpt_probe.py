r"""Собрать пару «тест|эталон» из двух файлов для точечной пробы.

Зачем отдельный сценарий: пути к парам на Windows содержат обратные косые, и
собирать их из оболочки — верный способ получить `\f` вместо `\fl.html`.
Дважды на этом попались, поэтому список пишется отсюда.

    python scripts/wpt_probe.py target/fl.html target/flref.html
    ./target/debug/examples/wptrun.exe target/pairs-probe-one.txt 400 300
"""

import sys
from pathlib import Path

OUT = Path("target/pairs-probe-one.txt")


def main() -> None:
    if len(sys.argv) != 3:
        print("нужны два пути: тест и эталон", file=sys.stderr)
        raise SystemExit(1)
    test, reference = (Path(a).resolve() for a in sys.argv[1:3])
    for one in (test, reference):
        if not one.is_file():
            print(f"нет файла: {one}", file=sys.stderr)
            raise SystemExit(1)
    OUT.write_text(f"{test}|{reference}\n", encoding="utf-8")
    print(OUT.read_text(encoding="utf-8").strip())


main()
