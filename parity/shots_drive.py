# -*- coding: utf-8 -*-
"""Съёмка НАШЕЙ стороны с приведением окна в нужное состояние.

    python parity/shots_drive.py           # снять всё, что описано ниже
    python parity/shots_drive.py --list    # что покрыто, что нет

`shots.py` снимает только то, что случайно оказалось на экране: регион живёт
в реестре, лишь пока элемент отрисован. Поэтому Customize-нав, welcome,
Problems и терминал в прошлом прогоне не снялись — их просто не было видно.
Здесь для каждой ГРУППЫ элементов сначала подаются probe-эмиты, потом
делается свежий кадр и режутся все элементы группы.

Правки: добавить элемент → строка в GROUPS. Регион элемента должен
существовать в коде (`probe_area("id")`), иначе строка честно попадёт в
«без источника».
"""
import io
import json
import os
import socket
import sys
import time

ROOT = os.path.dirname(os.path.abspath(__file__))
PORT = int(os.environ.get("KAMIN_PROBE_PORT", "9333"))
PAD = 6.0

# (эмиты для входа в состояние, {элемент: регион}) — плюс необязательный
# третий элемент `True`, если снимать надо OVERLAY-окно, а не main.
GROUPS: list = [
    # Оверлеи живут в отдельном окне — снимаем с target=overlay
    ([{"cmd": "emit", "kind": "palette"}], {
        "127-command-palette": "ov-palette",
    }, True),
    ([{"cmd": "emit", "kind": "palette"}, {"cmd": "emit", "kind": "quickopen"}], {
        "124-quick-open": "ov-quickopen",
    }, True),
    ([{"cmd": "emit", "kind": "quickopen"}, {"cmd": "emit", "kind": "findInFiles"}], {
        "125-find-in-files": "ov-fif",
    }, True),
    ([{"cmd": "emit", "kind": "findInFiles"}, {"cmd": "emit", "kind": "workspaceSymbols"}], {
        "126-workspace-symbols": "ov-symbols",
    }, True),
    ([{"cmd": "emit", "kind": "workspaceSymbols"},
      {"cmd": "emit", "kind": "toast", "name": "info"}], {
        "128-toasts-stack": "ov-toasts",
    }, True),
    # Customize → Design: корень панели
    ([{"cmd": "emit", "kind": "czOpen"}, {"cmd": "emit", "kind": "czPanel", "name": "design"}], {
        "79-design-panel-shell": "design-panel",
    }),
    # Customize → Extensions: панель и первая строка
    ([{"cmd": "emit", "kind": "czPanel", "name": "extensions"}], {
        "84-extensions-panel": "extensions-panel",
        "85-extension-row": "extension-row",
    }),
    # Базовое состояние: сессии в сайдбаре, чат в главной колонке
    ([], {
        "01-titlebar": "titlebar",
        "03-titlebar-brand-logo": "brand",
        "05-titlebar-command-search-button": "command-search",
        "08-titlebar-quick-actions-row": "quick-actions",
        "09-titlebar-quick-action-button": "toggle-sidebar",
        "10-layout-toggles-trigger": "layout-toggles",
        "13-theme-quick-toggle-trigger": "theme-toggle",
        "04-titlebar-tabs-slot": "tabs-slot",
        "18-session-tabs-strip": "tabs-slot",
        "19-session-tab-chip": "session-chip",
        "20-sidebar-root": "sidebar",
        "38-activity-bar-nav": "activity-bar",
        "39-activity-tile": "activity-tile",
        "52-app-shell": "body",
        "56-right-panel-column": "right-top",
        "58-right-panel-top-card": "right-top",
        "59-right-panel-split-handle": "right-split-handle",
        "60-right-panel-bottom-card": "right-bottom",
        "88-terminal-view": "central-bottom",
        "89-terminal-toolbar": "term-toolbar",
        "108-file-viewer-wrapper": "file-tabs",
        "110-file-viewer-tabs-strip": "file-tabs",
        "116-status-bar-root": "status-bar",
        "117-status-item-builtin": "status-item",
        "119-status-editor-encoding-eol": "status-bar",
        "120-status-version-update": "status-bar",
        "110-file-viewer-tabs-strip": "editor-tabs-bar",
        "68-panel-placeholder": "panel-placeholder",
    }),
    # Дерево файлов в файловой панели
    ([{"cmd": "emit", "kind": "fileMode", "name": "files"}], {
        "92-file-tree-root": "file-tree",
        "98-file-tree-header-toolbar": "file-tree-header",
        "94-file-tree-folder-row": "file-tree-row",
    }),
    # Customize: навигация + страница
    ([{"cmd": "emit", "kind": "czOpen"}], {
        "35-customize-mode-nav": "customize-nav",
        "78-customize-content-panel": "cz-contrib",
    }),
    ([{"cmd": "emit", "kind": "czPanel", "name": "design"}], {
        "130-design-color-tokens": "cz-body",
    }),
    ([{"cmd": "emit", "kind": "czPanel", "name": "logs"}], {
        "80-logs-panel": "cz-body",
    }),
    ([{"cmd": "emit", "kind": "czPanel", "name": "settings"}], {
        "82-settings-panel": "cz-body",
    }),
    ([{"cmd": "emit", "kind": "czOpen"}], {}),  # выход из Customize
]


def probe(req: dict, timeout: float = 15.0):
    s = socket.create_connection(("127.0.0.1", PORT), timeout=timeout)
    s.settimeout(timeout)
    s.sendall((json.dumps(req) + "\n").encode())
    buf = b""
    while b"\n" not in buf:
        chunk = s.recv(65536)
        if not chunk:
            break
        buf += chunk
    s.close()
    return json.loads(buf.split(b"\n")[0].decode())


def main() -> int:
    dirs = sorted(
        d for d in os.listdir(ROOT) if os.path.isdir(os.path.join(ROOT, d)) and d[0].isdigit()
    )
    covered = {e for g in GROUPS for e in g[1]}
    if "--list" in sys.argv:
        print(f"элементов: {len(dirs)}")
        print(f"  снимаются с приведением состояния: {len(covered)}")
        print(f"  БЕЗ источника кадра: {len(dirs) - len(covered)}")
        for d in dirs:
            if d not in covered:
                print("   ", d)
        return 0

    try:
        from PIL import Image
    except ImportError:
        print("нужен Pillow")
        return 2

    done, skipped = 0, []
    for group in GROUPS:
        emits, elements = group[0], group[1]
        overlay = len(group) > 2 and bool(group[2])
        for e in emits:
            probe(e)
            time.sleep(0.5)
        if not elements:
            continue
        time.sleep(0.8)
        req = {"cmd": "screenshot"}
        if overlay:
            req["target"] = "overlay"
        shot = probe(req)
        path = shot.get("path")
        if not path:
            print("probe не отдал кадр:", shot)
            return 1
        tree = probe({"cmd": "tree"}).get("regions", {})
        im = Image.open(path).convert("RGB")
        win_w = max(r["x"] + r["w"] for r in tree.values()) if tree else 0
        dpr = im.size[0] / win_w if win_w else 1.25
        for elem, rid in elements.items():
            r = tree.get(rid)
            if not r:
                skipped.append(f"{elem} (регион «{rid}» не отрисован)")
                continue
            box = (
                max(0, int((r["x"] - PAD) * dpr)),
                max(0, int((r["y"] - PAD) * dpr)),
                min(im.size[0], int((r["x"] + r["w"] + PAD) * dpr)),
                min(im.size[1], int((r["y"] + r["h"] + PAD) * dpr)),
            )
            if box[2] - box[0] < 4 or box[3] - box[1] < 4:
                skipped.append(f"{elem} (регион вырожден)")
                continue
            im.crop(box).save(os.path.join(ROOT, elem, "ours.png"))
            done += 1
    print(f"снято: {done}")
    for s in skipped:
        print("  пропущено:", s)
    print(f"без источника кадра вообще: {len(dirs) - len(covered)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
