"""Compatibility alias of scripts/memprof.py report (docs/memprof.md).

  alloc_stacks.py EXE STACKS [BASE_EXE BASE_STACKS] [--live] [--depth N] [--frames A-B]

Without --live it folds every allocation of the probe window (memprof.py report --allocated);
with --live, only blocks still live at the end of the window.
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import memprof  # noqa: E402


def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    if "--live" in argv:
        argv.remove("--live")
    else:
        argv.append("--allocated")
    memprof.main(["report", *argv])


if __name__ == "__main__":
    main()
