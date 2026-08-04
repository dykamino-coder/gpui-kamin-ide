# -*- coding: utf-8 -*-
"""Поэлементная съёмка НАШЕЙ стороны: кроп по живым bounds из probe.

Зачем: три цикла подряд ревьюеры находили, что `ours.png`/`original.png` в
папках — ОДИН общий кроп зоны на сторону (в зоне 38-51 все 14 элементов
держали один файл), поэтому вердикты по состояниям (поповер, rename-инпут,
свотчи, drop-плейсхолдеры) держались только на сверке кода.

    python parity/shots.py            # снять всё, что можно снять из текущего
                                      # состояния окна
    python parity/shots.py --list     # что покрыто регионами, что нет

Правила:
  * элемент снимается ТОЛЬКО если у него есть регион в probe-дереве
    (`probe_area` в коде) — иначе честно помечаем «нет источника кадра»;
  * поля вокруг региона `PAD` логических px, чтобы попадали рамка и тень;
  * DPR берём из отношения кадра к ширине окна, а не из константы.

Состояния (поповеры/меню/драг) снимаются отдельным прогоном: сначала эмит,
потом этот скрипт. Соответствие «элемент → эмит» ведётся в STATES.
"""
import io
import json
import os
import socket
import subprocess
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
PORT = int(os.environ.get("KAMIN_PROBE_PORT", "9333"))
PAD = 6.0

# элемент → id региона probe. Пусто = кадр этим способом не снять.
REGION = {
    "01-titlebar": "titlebar",
    "08-titlebar-quick-actions-row": "toggle-sidebar",
    "09-titlebar-quick-action-button": "toggle-sidebar",
    "13-theme-quick-toggle": "theme-toggle",
    "18-session-tabs-strip": "session-strip",
    "04-titlebar-tabs-slot": "tabs-slot",
    "20-sidebar-root": "sidebar",
    "38-activity-bar-nav": "activity-bar",
    "52-app-shell": "body",
    "88-terminal-view": "central-bottom",
    "110-file-viewer-tabs-strip": "file-tabs",
    "108-file-viewer-wrapper": "file-viewer-wrapper",
    "116-status-bar-root": "status-bar",
    "117-status-item-builtin": "status-bar",
    "118-status-item-contributed": "status-item-contributed",
    "119-status-editor-encoding-eol": "status-encoding",
    "120-status-version-update": "status-version",
    "56-right-panel-column": "right-panel-column",
    "58-right-panel-top-card": "right-top",
    "59-right-panel-bottom-card": "right-bottom",
    "60-right-panel-split-handle": "right-split-handle",
    "63-file-panel-bottom-handle": "file-bottom-handle",
    "114-webview-panel-view": "czShared",
    "19-session-tab-chip": "session-chip",
    "03-titlebar-brand-logo": "brand",
    "05-titlebar-command-search-button": "command-search",
    "08-titlebar-quick-actions-row": "quick-actions",
    "09-titlebar-quick-action-button": "toggle-sidebar",
    "10-layout-toggles-trigger": "layout-toggles",
    "39-activity-tile": "activity-tile",
    "117-status-item-builtin": "status-item",
    "89-terminal-toolbar": "terminal",
    "92-file-tree-root": "file-tree",
    "35-customize-mode-nav": "customize-nav",
    "77-welcome-placeholder": "welcome",
    "86-problems-panel": "problems-header",
    "89-terminal-toolbar": "term-toolbar",
    # Добавлено ц.26: регионы у элементов ЖИВЫ (probe их отдаёт), но в карту
    # не были заведены — из-за этого 124 досье считались «без источника кадра»
    # и держали общий кадр окна
    "40-activity-rail-right": "rail-right-top",
    "41-activity-rail-tile": "rail-tile-right-top",
    "42-panel-slot-strip": "strip-main-0",
    "45-panel-slot-picker-anchor": "picker-anchor-main",
    "53-main-column": "main",
    "64-file-panel-mode-tabs": "file-mode-tabs",
    "90-terminal-shell-menu": "term-add-btn",
    "91-terminal-session-host": "terminal",
    "94-file-tree-folder-row": "file-tree-folder-row",
    "95-file-tree-file-row": "file-tree-file-row",
    "97-file-tree-row-badge": "file-tree-row-badge",
    "98-file-tree-header-toolbar": "file-tree-header",
    "109-file-viewer-tab-strip-row": "editor-tabs-bar",
    "111-file-viewer-tab": "file-viewer-tab",
    "146-sample-horizontal-tab-strip": "sample-tab-strip",
}

# элемент → эмит, который приводит окно в нужное состояние (для следующих
# прогонов; сам скрипт состояние НЕ меняет, чтобы не ломать чужие замеры).
STATES = {
    "11-layout-toggles-menu": '{"cmd":"emit","kind":"toggleLayoutPopover"}',
    "14-theme-popover": '{"cmd":"emit","kind":"toggleAppearancePopover"}',
    "44-activity-picker-menu": '{"cmd":"emit","kind":"toolPicker","name":"sidebar"}',
    "78-customize-content-panel": '{"cmd":"emit","kind":"czOpen"}',
    "82-settings-panel": '{"cmd":"emit","kind":"czPanel","name":"settings"}',
    "80-logs-panel": '{"cmd":"emit","kind":"czPanel","name":"logs"}',
    # Добавлено ц.26: состояния, эмиты для которых уже есть в probe
    "81-system-log-panel": '{"cmd":"emit","kind":"czPanel","name":"system"}',
    "79-design-panel-shell": '{"cmd":"emit","kind":"czPanel","name":"design"}',
    "84-extensions-panel": '{"cmd":"emit","kind":"czPanel","name":"extensions"}',
    "100-file-context-menu": '{"cmd":"emit","kind":"fileMenu","name":"C:\\Users","dir":true,"x":600,"y":300}',
    "101-file-context-submenu": '{"cmd":"emit","kind":"fileMenuOpenIn","dir":true}',
    "67-browser-pane": '{"cmd":"emit","kind":"fileMode","name":"web"}',
    "71-webview-load-error": '{"cmd":"emit","kind":"viewLoadError","name":"helloView"}',
    "121-confirm-modal": '{"cmd":"emit","kind":"confirm"}',
    "123-quick-pick-modal": '{"cmd":"emit","kind":"quickPick"}',
    "127-command-palette": '{"cmd":"emit","kind":"palette"}',
}


def ambiguous_regions() -> dict:
    """Регионы, на которые претендует БОЛЬШЕ одного элемента.

    Один и тот же кроп в разных досье — ровно тот дефект, из-за которого
    вердикты держались на сверке кода (ц.25: 14 групп общих кадров). Такие
    элементы кадр НЕ получают: им нужен свой `probe_area` в порте, и гейт
    показывает их как незакрытые.
    """
    used: dict = {}
    for slug, region in REGION.items():
        used.setdefault(region, []).append(slug)
    return {r: sorted(v) for r, v in used.items() if len(v) > 1}


def probe(req: dict, timeout: float = 8.0):
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
    if "--list" in sys.argv:
        have = [d for d in dirs if d in REGION]
        state = [d for d in dirs if d in STATES]
        print(f"элементов: {len(dirs)}")
        print(f"  снимаются по региону: {len(have)}")
        print(f"  требуют состояния (эмит известен): {len(state)}")
        print(f"  БЕЗ источника кадра: {len(dirs) - len(have) - len(state)}")
        for d in dirs:
            if d not in REGION and d not in STATES:
                print("   ", d)
        return 0

    try:
        from PIL import Image
    except ImportError:
        print("нужен Pillow")
        return 2

    shot = probe({"cmd": "screenshot"})
    path = shot.get("path")
    if not path:
        print("probe не отдал кадр:", shot)
        return 1
    tree = probe({"cmd": "tree"}).get("regions", {})
    im = Image.open(path).convert("RGB")
    win_w = max(r["x"] + r["w"] for r in tree.values()) if tree else 0
    dpr = im.size[0] / win_w if win_w else 1.25
    print(f"кадр {im.size[0]}×{im.size[1]}, окно {win_w:.0f} лог., DPR {dpr:.3f}")

    amb = ambiguous_regions()
    claimed = {slug for group in amb.values() for slug in group}
    if amb:
        print("НЕ снимаю (регион делят несколько элементов, нужен свой probe_area):")
        for region, group in sorted(amb.items()):
            print(f"  {region}: {', '.join(group)}")

    done, skipped = 0, []
    for d in dirs:
        rid = REGION.get(d)
        if not rid or rid not in tree or d in claimed:
            skipped.append(d)
            continue
        r = tree[rid]
        box = (
            max(0, int((r["x"] - PAD) * dpr)),
            max(0, int((r["y"] - PAD) * dpr)),
            min(im.size[0], int((r["x"] + r["w"] + PAD) * dpr)),
            min(im.size[1], int((r["y"] + r["h"] + PAD) * dpr)),
        )
        if box[2] - box[0] < 4 or box[3] - box[1] < 4:
            skipped.append(d)
            continue
        im.crop(box).save(os.path.join(ROOT, d, "ours.png"))
        done += 1
        print(f"  {d}: {box[2]-box[0]}×{box[3]-box[1]} физ. из региона «{rid}»")
    print(f"\nснято: {done}; без источника: {len(skipped)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
