# Инвентарь визуальной сверки — 159 элементов

Одна папка на элемент: `original.md` (описание + код оригинала), `ours.md` (наша сторона), `original.png` / `ours.png` (кадры обеих сторон), `verdict.md` (история циклов ревью).

Таблица генерируется: `python parity/index.py`. Гейт: `python parity/gate.py`. Задание на следующий цикл: `python parity/cycle.py`. Полнота описаний (отступы/цвета/гэпы/скругления/шрифты/ховер): `python parity/attrs.py`.

**Статус: 127 MATCH / 32 DIVERGES из 159.**

## 1-19 — Титлбар  (14 MATCH / 5 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 1 | [titlebar](01-titlebar/) | MATCH | 19 | [✔](01-titlebar/original.md) | [✔](01-titlebar/ours.md) | ✔ | ✔ |
| 2 | [titlebar-left-cluster](02-titlebar-left-cluster/) | DIVERGES | 26 | [✔](02-titlebar-left-cluster/original.md) | [✔](02-titlebar-left-cluster/ours.md) | ✔ | ✔ |
| 3 | [titlebar-brand-logo](03-titlebar-brand-logo/) | MATCH | 19 | [✔](03-titlebar-brand-logo/original.md) | [✔](03-titlebar-brand-logo/ours.md) | ✔ | ✔ |
| 4 | [titlebar-tabs-slot](04-titlebar-tabs-slot/) | MATCH | 19 | [✔](04-titlebar-tabs-slot/original.md) | [✔](04-titlebar-tabs-slot/ours.md) | ✔ | ✔ |
| 5 | [titlebar-command-search-button](05-titlebar-command-search-button/) | MATCH | 19 | [✔](05-titlebar-command-search-button/original.md) | [✔](05-titlebar-command-search-button/ours.md) | ✔ | ✔ |
| 6 | [titlebar-window-controls-cluster](06-titlebar-window-controls-cluster/) | MATCH | 19 | [✔](06-titlebar-window-controls-cluster/original.md) | [✔](06-titlebar-window-controls-cluster/ours.md) | ✔ | ✔ |
| 7 | [titlebar-button](07-titlebar-button/) | MATCH | 19 | [✔](07-titlebar-button/original.md) | [✔](07-titlebar-button/ours.md) | ✔ | ✔ |
| 8 | [titlebar-quick-actions-row](08-titlebar-quick-actions-row/) | MATCH | 19 | [✔](08-titlebar-quick-actions-row/original.md) | [✔](08-titlebar-quick-actions-row/ours.md) | ✔ | ✔ |
| 9 | [titlebar-quick-action-button](09-titlebar-quick-action-button/) | MATCH | 23 | [✔](09-titlebar-quick-action-button/original.md) | [✔](09-titlebar-quick-action-button/ours.md) | ✔ | ✔ |
| 10 | [layout-toggles-trigger](10-layout-toggles-trigger/) | MATCH | 19 | [✔](10-layout-toggles-trigger/original.md) | [✔](10-layout-toggles-trigger/ours.md) | ✔ | ✔ |
| 11 | [layout-toggles-menu](11-layout-toggles-menu/) | DIVERGES | 26 | [✔](11-layout-toggles-menu/original.md) | [✔](11-layout-toggles-menu/ours.md) | ✔ | ✔ |
| 12 | [layout-presets-section](12-layout-presets-section/) | MATCH | 19 | [✔](12-layout-presets-section/original.md) | [✔](12-layout-presets-section/ours.md) | ✔ | ✔ |
| 13 | [theme-quick-toggle-trigger](13-theme-quick-toggle-trigger/) | MATCH | 19 | [✔](13-theme-quick-toggle-trigger/original.md) | [✔](13-theme-quick-toggle-trigger/ours.md) | ✔ | ✔ |
| 14 | [theme-popover](14-theme-popover/) | MATCH | 19 | [✔](14-theme-popover/original.md) | [✔](14-theme-popover/ours.md) | ✔ | ✔ |
| 15 | [theme-popover-column](15-theme-popover-column/) | DIVERGES | 26 | [✔](15-theme-popover-column/original.md) | [✔](15-theme-popover-column/ours.md) | ✔ | ✔ |
| 16 | [theme-popover-item](16-theme-popover-item/) | MATCH | 19 | [✔](16-theme-popover-item/original.md) | [✔](16-theme-popover-item/ours.md) | ✔ | ✔ |
| 17 | [panel-icon-svg](17-panel-icon-svg/) | MATCH | 23 | [✔](17-panel-icon-svg/original.md) | [✔](17-panel-icon-svg/ours.md) | ✔ | ✔ |
| 18 | [session-tabs-strip](18-session-tabs-strip/) | DIVERGES | 26 | [✔](18-session-tabs-strip/original.md) | [✔](18-session-tabs-strip/ours.md) | ✔ | ✔ |
| 19 | [session-tab-chip](19-session-tab-chip/) | DIVERGES | 26 | [✔](19-session-tab-chip/original.md) | [✔](19-session-tab-chip/ours.md) | ✔ | ✔ |

## 20-37 — Сайдбар  (15 MATCH / 3 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 20 | [sidebar-root](20-sidebar-root/) | MATCH | 20 | [✔](20-sidebar-root/original.md) | [✔](20-sidebar-root/ours.md) | ✔ | ✔ |
| 21 | [sidebar-resize-handle](21-sidebar-resize-handle/) | MATCH | 20 | [✔](21-sidebar-resize-handle/original.md) | [✔](21-sidebar-resize-handle/ours.md) | ✔ | ✔ |
| 22 | [sidebar-body-resolver](22-sidebar-body-resolver/) | MATCH | 20 | [✔](22-sidebar-body-resolver/original.md) | [✔](22-sidebar-body-resolver/ours.md) | ✔ | ✔ |
| 23 | [sessions-mode-root](23-sessions-mode-root/) | DIVERGES | 26 | [✔](23-sessions-mode-root/original.md) | [✔](23-sessions-mode-root/ours.md) | ✔ | ✔ |
| 24 | [project-group-header](24-project-group-header/) | MATCH | 23 | [✔](24-project-group-header/original.md) | [✔](24-project-group-header/ours.md) | ✔ | ✔ |
| 25 | [project-actions-popover](25-project-actions-popover/) | MATCH | 20 | [✔](25-project-actions-popover/original.md) | [✔](25-project-actions-popover/ours.md) | ✔ | ✔ |
| 26 | [project-sessions-list](26-project-sessions-list/) | MATCH | 20 | [✔](26-project-sessions-list/original.md) | [✔](26-project-sessions-list/ours.md) | ✔ | ✔ |
| 27 | [project-inactive-toggle](27-project-inactive-toggle/) | MATCH | 20 | [✔](27-project-inactive-toggle/original.md) | [✔](27-project-inactive-toggle/ours.md) | ✔ | ✔ |
| 28 | [session-item-row](28-session-item-row/) | DIVERGES | 26 | [✔](28-session-item-row/original.md) | [✔](28-session-item-row/ours.md) | ✔ | ✔ |
| 29 | [session-status-dot](29-session-status-dot/) | MATCH | 20 | [✔](29-session-status-dot/original.md) | [✔](29-session-status-dot/ours.md) | ✔ | ✔ |
| 30 | [session-pin-button](30-session-pin-button/) | MATCH | 20 | [✔](30-session-pin-button/original.md) | [✔](30-session-pin-button/ours.md) | ✔ | ✔ |
| 31 | [session-rename-input](31-session-rename-input/) | DIVERGES | 26 | [✔](31-session-rename-input/original.md) | [✔](31-session-rename-input/ours.md) | ✔ | ✔ |
| 32 | [session-actions-popover](32-session-actions-popover/) | MATCH | 20 | [✔](32-session-actions-popover/original.md) | [✔](32-session-actions-popover/ours.md) | ✔ | ✔ |
| 33 | [session-context-menu](33-session-context-menu/) | MATCH | 23 | [✔](33-session-context-menu/original.md) | [✔](33-session-context-menu/ours.md) | ✔ | ✔ |
| 34 | [session-color-swatches](34-session-color-swatches/) | MATCH | 20 | [✔](34-session-color-swatches/original.md) | [✔](34-session-color-swatches/ours.md) | ✔ | ✔ |
| 35 | [customize-mode-nav](35-customize-mode-nav/) | MATCH | 20 | [✔](35-customize-mode-nav/original.md) | [✔](35-customize-mode-nav/ours.md) | ✔ | ✔ |
| 36 | [customize-nav-item](36-customize-nav-item/) | MATCH | 20 | [✔](36-customize-nav-item/original.md) | [✔](36-customize-nav-item/ours.md) | ✔ | ✔ |
| 37 | [customize-contributed-tree](37-customize-contributed-tree/) | MATCH | 20 | [✔](37-customize-contributed-tree/original.md) | [✔](37-customize-contributed-tree/ours.md) | ✔ | ✔ |

## 38-51 — Activity, рейлы, стрипы  (14 MATCH / 0 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 38 | [activity-bar-nav](38-activity-bar-nav/) | MATCH | 23 | [✔](38-activity-bar-nav/original.md) | [✔](38-activity-bar-nav/ours.md) | ✔ | ✔ |
| 39 | [activity-tile](39-activity-tile/) | MATCH | 23 | [✔](39-activity-tile/original.md) | [✔](39-activity-tile/ours.md) | ✔ | ✔ |
| 40 | [activity-customize-tile](40-activity-customize-tile/) | MATCH | 21 | [✔](40-activity-customize-tile/original.md) | [✔](40-activity-customize-tile/ours.md) | ✔ | ✔ |
| 41 | [activity-drop-placeholder](41-activity-drop-placeholder/) | MATCH | 21 | [✔](41-activity-drop-placeholder/original.md) | [✔](41-activity-drop-placeholder/ours.md) | ✔ | ✔ |
| 42 | [activity-picker-dots-trigger](42-activity-picker-dots-trigger/) | MATCH | 23 | [✔](42-activity-picker-dots-trigger/original.md) | [✔](42-activity-picker-dots-trigger/ours.md) | ✔ | ✔ |
| 43 | [activity-picker-open-tool-pill](43-activity-picker-open-tool-pill/) | MATCH | 21 | [✔](43-activity-picker-open-tool-pill/original.md) | [✔](43-activity-picker-open-tool-pill/ours.md) | ✔ | ✔ |
| 44 | [activity-picker-menu](44-activity-picker-menu/) | MATCH | 21 | [✔](44-activity-picker-menu/original.md) | [✔](44-activity-picker-menu/ours.md) | ✔ | ✔ |
| 45 | [activity-context-menu](45-activity-context-menu/) | MATCH | 23 | [✔](45-activity-context-menu/original.md) | [✔](45-activity-context-menu/ours.md) | ✔ | ✔ |
| 46 | [activity-context-submenu](46-activity-context-submenu/) | MATCH | 23 | [✔](46-activity-context-submenu/original.md) | [✔](46-activity-context-submenu/ours.md) | ✔ | ✔ |
| 47 | [activity-drag-ghost](47-activity-drag-ghost/) | MATCH | 21 | [✔](47-activity-drag-ghost/original.md) | [✔](47-activity-drag-ghost/ours.md) | ✔ | ✔ |
| 48 | [bottom-tab-bar-strip](48-bottom-tab-bar-strip/) | MATCH | 21 | [✔](48-bottom-tab-bar-strip/original.md) | [✔](48-bottom-tab-bar-strip/ours.md) | ✔ | ✔ |
| 49 | [bottom-tab](49-bottom-tab/) | MATCH | 23 | [✔](49-bottom-tab/original.md) | [✔](49-bottom-tab/ours.md) | ✔ | ✔ |
| 50 | [bottom-tab-drop-placeholder](50-bottom-tab-drop-placeholder/) | MATCH | 21 | [✔](50-bottom-tab-drop-placeholder/original.md) | [✔](50-bottom-tab-drop-placeholder/ours.md) | ✔ | ✔ |
| 51 | [tool-icon](51-tool-icon/) | MATCH | 23 | [✔](51-tool-icon/original.md) | [✔](51-tool-icon/ours.md) | ✔ | ✔ |

## 52-91 — Панели и экраны  (35 MATCH / 5 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 52 | [app-shell](52-app-shell/) | MATCH | 19 | [✔](52-app-shell/original.md) | [✔](52-app-shell/ours.md) | ✔ | ✔ |
| 53 | [main-content](53-main-content/) | MATCH | 19 | [✔](53-main-content/original.md) | [✔](53-main-content/ours.md) | ✔ | ✔ |
| 54 | [main-bottom-panel](54-main-bottom-panel/) | MATCH | 19 | [✔](54-main-bottom-panel/original.md) | [✔](54-main-bottom-panel/ours.md) | ✔ | ✔ |
| 55 | [main-bottom-resize-handle](55-main-bottom-resize-handle/) | MATCH | 19 | [✔](55-main-bottom-resize-handle/original.md) | [✔](55-main-bottom-resize-handle/ours.md) | ✔ | ✔ |
| 56 | [right-panel-column](56-right-panel-column/) | DIVERGES | 26 | [✔](56-right-panel-column/original.md) | [✔](56-right-panel-column/ours.md) | ✔ | ✔ |
| 57 | [right-panel-width-handle](57-right-panel-width-handle/) | MATCH | 19 | [✔](57-right-panel-width-handle/original.md) | [✔](57-right-panel-width-handle/ours.md) | ✔ | ✔ |
| 58 | [right-panel-top-card](58-right-panel-top-card/) | MATCH | 19 | [✔](58-right-panel-top-card/original.md) | [✔](58-right-panel-top-card/ours.md) | ✔ | ✔ |
| 59 | [right-panel-split-handle](59-right-panel-split-handle/) | MATCH | 19 | [✔](59-right-panel-split-handle/original.md) | [✔](59-right-panel-split-handle/ours.md) | ✔ | ✔ |
| 60 | [right-panel-bottom-card](60-right-panel-bottom-card/) | MATCH | 19 | [✔](60-right-panel-bottom-card/original.md) | [✔](60-right-panel-bottom-card/ours.md) | ✔ | ✔ |
| 61 | [file-panel-column](61-file-panel-column/) | MATCH | 19 | [✔](61-file-panel-column/original.md) | [✔](61-file-panel-column/ours.md) | ✔ | ✔ |
| 62 | [file-panel-width-handle](62-file-panel-width-handle/) | MATCH | 19 | [✔](62-file-panel-width-handle/original.md) | [✔](62-file-panel-width-handle/ours.md) | ✔ | ✔ |
| 63 | [file-panel-top-card](63-file-panel-top-card/) | MATCH | 23 | [✔](63-file-panel-top-card/original.md) | [✔](63-file-panel-top-card/ours.md) | ✔ | ✔ |
| 64 | [file-panel-split-handle](64-file-panel-split-handle/) | MATCH | 23 | [✔](64-file-panel-split-handle/original.md) | [✔](64-file-panel-split-handle/ours.md) | ✔ | ✔ |
| 65 | [file-panel-bottom-card](65-file-panel-bottom-card/) | MATCH | 23 | [✔](65-file-panel-bottom-card/original.md) | [✔](65-file-panel-bottom-card/ours.md) | ✔ | ✔ |
| 66 | [file-panel-mode-tabs](66-file-panel-mode-tabs/) | MATCH | 19 | [✔](66-file-panel-mode-tabs/original.md) | [✔](66-file-panel-mode-tabs/ours.md) | ✔ | ✔ |
| 67 | [browser-pane](67-browser-pane/) | MATCH | 24 | [✔](67-browser-pane/original.md) | [✔](67-browser-pane/ours.md) | ✔ | ✔ |
| 68 | [panel-placeholder](68-panel-placeholder/) | MATCH | 19 | [✔](68-panel-placeholder/original.md) | [✔](68-panel-placeholder/ours.md) | ✔ | ✔ |
| 69 | [activity-placeholder](69-activity-placeholder/) | MATCH | 19 | [✔](69-activity-placeholder/original.md) | [✔](69-activity-placeholder/ours.md) | ✔ | ✔ |
| 70 | [webview-loading-skeleton](70-webview-loading-skeleton/) | MATCH | 23 | [✔](70-webview-loading-skeleton/original.md) | [✔](70-webview-loading-skeleton/ours.md) | — | ✔ |
| 71 | [webview-load-error](71-webview-load-error/) | MATCH | 19 | [✔](71-webview-load-error/original.md) | [✔](71-webview-load-error/ours.md) | — | — |
| 72 | [chat-switch-skeleton](72-chat-switch-skeleton/) | MATCH | 23 | [✔](72-chat-switch-skeleton/original.md) | [✔](72-chat-switch-skeleton/ours.md) | ✔ | ✔ |
| 73 | [contributed-container-body](73-contributed-container-body/) | MATCH | 22 | [✔](73-contributed-container-body/original.md) | [✔](73-contributed-container-body/ours.md) | ✔ | ✔ |
| 74 | [contributed-view-section](74-contributed-view-section/) | MATCH | 19 | [✔](74-contributed-view-section/original.md) | [✔](74-contributed-view-section/ours.md) | ✔ | ✔ |
| 75 | [webview-view-anchor](75-webview-view-anchor/) | MATCH | 23 | [✔](75-webview-view-anchor/original.md) | [✔](75-webview-view-anchor/ours.md) | ✔ | ✔ |
| 76 | [persistent-webview-layer](76-persistent-webview-layer/) | MATCH | 23 | [✔](76-persistent-webview-layer/original.md) | [✔](76-persistent-webview-layer/ours.md) | ✔ | ✔ |
| 77 | [welcome-placeholder](77-welcome-placeholder/) | DIVERGES | 26 | [✔](77-welcome-placeholder/original.md) | [✔](77-welcome-placeholder/ours.md) | ✔ | ✔ |
| 78 | [customize-content-panel](78-customize-content-panel/) | MATCH | 23 | [✔](78-customize-content-panel/original.md) | [✔](78-customize-content-panel/ours.md) | ✔ | ✔ |
| 79 | [design-panel-shell](79-design-panel-shell/) | MATCH | 23 | [✔](79-design-panel-shell/original.md) | [✔](79-design-panel-shell/ours.md) | ✔ | ✔ |
| 80 | [logs-panel](80-logs-panel/) | DIVERGES | 26 | [✔](80-logs-panel/original.md) | [✔](80-logs-panel/ours.md) | ✔ | ✔ |
| 81 | [system-log-panel](81-system-log-panel/) | DIVERGES | 26 | [✔](81-system-log-panel/original.md) | [✔](81-system-log-panel/ours.md) | ✔ | ✔ |
| 82 | [settings-panel](82-settings-panel/) | DIVERGES | 26 | [✔](82-settings-panel/original.md) | [✔](82-settings-panel/ours.md) | ✔ | ✔ |
| 83 | [legacy-bridge-card](83-legacy-bridge-card/) | MATCH | 23 | [✔](83-legacy-bridge-card/original.md) | [✔](83-legacy-bridge-card/ours.md) | ✔ | ✔ |
| 84 | [extensions-panel](84-extensions-panel/) | MATCH | 19 | [✔](84-extensions-panel/original.md) | [✔](84-extensions-panel/ours.md) | ✔ | ✔ |
| 85 | [extension-row](85-extension-row/) | MATCH | 23 | [✔](85-extension-row/original.md) | [✔](85-extension-row/ours.md) | ✔ | ✔ |
| 86 | [problems-panel](86-problems-panel/) | MATCH | 19 | [✔](86-problems-panel/original.md) | [✔](86-problems-panel/ours.md) | ✔ | ✔ |
| 87 | [problem-row](87-problem-row/) | MATCH | 19 | [✔](87-problem-row/original.md) | [✔](87-problem-row/ours.md) | ✔ | ✔ |
| 88 | [terminal-view](88-terminal-view/) | MATCH | 19 | [✔](88-terminal-view/original.md) | [✔](88-terminal-view/ours.md) | ✔ | ✔ |
| 89 | [terminal-toolbar](89-terminal-toolbar/) | MATCH | 23 | [✔](89-terminal-toolbar/original.md) | [✔](89-terminal-toolbar/ours.md) | ✔ | ✔ |
| 90 | [terminal-shell-menu](90-terminal-shell-menu/) | MATCH | 23 | [✔](90-terminal-shell-menu/original.md) | [✔](90-terminal-shell-menu/ours.md) | — | ✔ |
| 91 | [terminal-session-host](91-terminal-session-host/) | MATCH | 23 | [✔](91-terminal-session-host/original.md) | [✔](91-terminal-session-host/ours.md) | ✔ | ✔ |

## 92-107 — Дерево файлов  (12 MATCH / 4 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 92 | [file-tree-root](92-file-tree-root/) | DIVERGES | 25 | [✔](92-file-tree-root/original.md) | [✔](92-file-tree-root/ours.md) | ✔ | ✔ |
| 93 | [file-tree-empty-state](93-file-tree-empty-state/) | MATCH | 21 | [✔](93-file-tree-empty-state/original.md) | [✔](93-file-tree-empty-state/ours.md) | ✔ | ✔ |
| 94 | [file-tree-folder-row](94-file-tree-folder-row/) | MATCH | 23 | [✔](94-file-tree-folder-row/original.md) | [✔](94-file-tree-folder-row/ours.md) | ✔ | ✔ |
| 95 | [file-tree-file-row](95-file-tree-file-row/) | DIVERGES | 25 | [✔](95-file-tree-file-row/original.md) | [✔](95-file-tree-file-row/ours.md) | ✔ | ✔ |
| 96 | [file-tree-children-states](96-file-tree-children-states/) | MATCH | 23 | [✔](96-file-tree-children-states/original.md) | [✔](96-file-tree-children-states/ours.md) | ✔ | ✔ |
| 97 | [file-tree-row-badge](97-file-tree-row-badge/) | MATCH | 21 | [✔](97-file-tree-row-badge/original.md) | [✔](97-file-tree-row-badge/ours.md) | ✔ | ✔ |
| 98 | [file-tree-header-toolbar](98-file-tree-header-toolbar/) | DIVERGES | 25 | [✔](98-file-tree-header-toolbar/original.md) | [✔](98-file-tree-header-toolbar/ours.md) | ✔ | ✔ |
| 99 | [tree-icon-img](99-tree-icon-img/) | MATCH | 23 | [✔](99-tree-icon-img/original.md) | [✔](99-tree-icon-img/ours.md) | ✔ | ✔ |
| 100 | [file-context-menu](100-file-context-menu/) | DIVERGES | 25 | [✔](100-file-context-menu/original.md) | [✔](100-file-context-menu/ours.md) | ✔ | ✔ |
| 101 | [file-context-submenu](101-file-context-submenu/) | MATCH | 24 | [✔](101-file-context-submenu/original.md) | [✔](101-file-context-submenu/ours.md) | ✔ | ✔ |
| 102 | [generic-tree](102-generic-tree/) | MATCH | 21 | [✔](102-generic-tree/original.md) | [✔](102-generic-tree/ours.md) | ✔ | ✔ |
| 103 | [generic-tree-row](103-generic-tree-row/) | MATCH | 21 | [✔](103-generic-tree-row/original.md) | [✔](103-generic-tree-row/ours.md) | ✔ | ✔ |
| 104 | [contributed-tree-view-body](104-contributed-tree-view-body/) | MATCH | 22 | [✔](104-contributed-tree-view-body/original.md) | [✔](104-contributed-tree-view-body/ours.md) | ✔ | ✔ |
| 105 | [contributed-tree-node-row](105-contributed-tree-node-row/) | MATCH | 23 | [✔](105-contributed-tree-node-row/original.md) | [✔](105-contributed-tree-node-row/ours.md) | ✔ | ✔ |
| 106 | [contributed-tree-checkbox](106-contributed-tree-checkbox/) | MATCH | 23 | [✔](106-contributed-tree-checkbox/original.md) | [✔](106-contributed-tree-checkbox/ours.md) | ✔ | ✔ |
| 107 | [contributed-tree-node-icon](107-contributed-tree-node-icon/) | MATCH | 22 | [✔](107-contributed-tree-node-icon/original.md) | [✔](107-contributed-tree-node-icon/ours.md) | ✔ | ✔ |

## 108-129 — Редактор, оверлеи, статус  (12 MATCH / 10 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 108 | [file-viewer-wrapper](108-file-viewer-wrapper/) | DIVERGES | 26 | [✔](108-file-viewer-wrapper/original.md) | [✔](108-file-viewer-wrapper/ours.md) | ✔ | ✔ |
| 109 | [file-viewer-empty](109-file-viewer-empty/) | MATCH | 20 | [✔](109-file-viewer-empty/original.md) | [✔](109-file-viewer-empty/ours.md) | ✔ | ✔ |
| 110 | [file-viewer-tabs-strip](110-file-viewer-tabs-strip/) | MATCH | 23 | [✔](110-file-viewer-tabs-strip/original.md) | [✔](110-file-viewer-tabs-strip/ours.md) | ✔ | ✔ |
| 111 | [file-viewer-tab](111-file-viewer-tab/) | DIVERGES | 26 | [✔](111-file-viewer-tab/original.md) | [✔](111-file-viewer-tab/ours.md) | ✔ | ✔ |
| 112 | [file-viewer-tabs-overflow](112-file-viewer-tabs-overflow/) | MATCH | 23 | [✔](112-file-viewer-tabs-overflow/original.md) | [✔](112-file-viewer-tabs-overflow/ours.md) | ✔ | ✔ |
| 113 | [monaco-editor-host](113-monaco-editor-host/) | MATCH | 23 | [✔](113-monaco-editor-host/original.md) | [✔](113-monaco-editor-host/ours.md) | ✔ | ✔ |
| 114 | [webview-panel-view](114-webview-panel-view/) | DIVERGES | 26 | [✔](114-webview-panel-view/original.md) | [✔](114-webview-panel-view/ours.md) | ✔ | ✔ |
| 115 | [webview-tab-icon](115-webview-tab-icon/) | DIVERGES | 26 | [✔](115-webview-tab-icon/original.md) | [✔](115-webview-tab-icon/ours.md) | ✔ | ✔ |
| 116 | [status-bar-root](116-status-bar-root/) | MATCH | 17 | [✔](116-status-bar-root/original.md) | [✔](116-status-bar-root/ours.md) | ✔ | ✔ |
| 117 | [status-item-builtin](117-status-item-builtin/) | DIVERGES | 26 | [✔](117-status-item-builtin/original.md) | [✔](117-status-item-builtin/ours.md) | ✔ | ✔ |
| 118 | [status-item-contributed](118-status-item-contributed/) | MATCH | 24 | [✔](118-status-item-contributed/original.md) | [✔](118-status-item-contributed/ours.md) | ✔ | ✔ |
| 119 | [status-editor-encoding-eol](119-status-editor-encoding-eol/) | MATCH | 20 | [✔](119-status-editor-encoding-eol/original.md) | [✔](119-status-editor-encoding-eol/ours.md) | ✔ | ✔ |
| 120 | [status-version-update](120-status-version-update/) | MATCH | 23 | [✔](120-status-version-update/original.md) | [✔](120-status-version-update/ours.md) | ✔ | ✔ |
| 121 | [confirm-modal](121-confirm-modal/) | DIVERGES | 26 | [✔](121-confirm-modal/original.md) | [✔](121-confirm-modal/ours.md) | ✔ | ✔ |
| 122 | [prompt-modal](122-prompt-modal/) | MATCH | 23 | [✔](122-prompt-modal/original.md) | [✔](122-prompt-modal/ours.md) | ✔ | ✔ |
| 123 | [quick-pick-modal](123-quick-pick-modal/) | DIVERGES | 26 | [✔](123-quick-pick-modal/original.md) | [✔](123-quick-pick-modal/ours.md) | — | ✔ |
| 124 | [quick-open](124-quick-open/) | MATCH | 26 | [✔](124-quick-open/original.md) | [✔](124-quick-open/ours.md) | ✔ | ✔ |
| 125 | [find-in-files](125-find-in-files/) | DIVERGES | 26 | [✔](125-find-in-files/original.md) | [✔](125-find-in-files/ours.md) | ✔ | ✔ |
| 126 | [workspace-symbols](126-workspace-symbols/) | DIVERGES | 26 | [✔](126-workspace-symbols/original.md) | [✔](126-workspace-symbols/ours.md) | ✔ | ✔ |
| 127 | [command-palette](127-command-palette/) | DIVERGES | 26 | [✔](127-command-palette/original.md) | [✔](127-command-palette/ours.md) | ✔ | ✔ |
| 128 | [toasts-stack](128-toasts-stack/) | MATCH | 20 | [✔](128-toasts-stack/original.md) | [✔](128-toasts-stack/ours.md) | ✔ | ✔ |
| 129 | [tooltip](129-tooltip/) | MATCH | 20 | [✔](129-tooltip/original.md) | [✔](129-tooltip/ours.md) | ✔ | ✔ |

## 130-159 — Токены, семплы, глобальные стили  (25 MATCH / 5 DIVERGES)

| # | элемент | вердикт | цикл | original.md | ours.md | original.png | ours.png |
|---|---|---|---|---|---|---|---|
| 130 | [design-color-tokens](130-design-color-tokens/) | MATCH | 18 | [✔](130-design-color-tokens/original.md) | [✔](130-design-color-tokens/ours.md) | ✔ | ✔ |
| 131 | [design-typography-tokens](131-design-typography-tokens/) | MATCH | 18 | [✔](131-design-typography-tokens/original.md) | [✔](131-design-typography-tokens/ours.md) | ✔ | ✔ |
| 132 | [design-spacing-tokens](132-design-spacing-tokens/) | MATCH | 18 | [✔](132-design-spacing-tokens/original.md) | [✔](132-design-spacing-tokens/ours.md) | ✔ | ✔ |
| 133 | [design-radius-tokens](133-design-radius-tokens/) | MATCH | 18 | [✔](133-design-radius-tokens/original.md) | [✔](133-design-radius-tokens/ours.md) | ✔ | ✔ |
| 134 | [design-shadow-tokens](134-design-shadow-tokens/) | MATCH | 18 | [✔](134-design-shadow-tokens/original.md) | [✔](134-design-shadow-tokens/ours.md) | ✔ | ✔ |
| 135 | [sample-buttons](135-sample-buttons/) | MATCH | 18 | [✔](135-sample-buttons/original.md) | [✔](135-sample-buttons/ours.md) | ✔ | ✔ |
| 136 | [sample-list-item](136-sample-list-item/) | MATCH | 18 | [✔](136-sample-list-item/original.md) | [✔](136-sample-list-item/ours.md) | ✔ | ✔ |
| 137 | [sample-input](137-sample-input/) | MATCH | 24 | [✔](137-sample-input/original.md) | [✔](137-sample-input/ours.md) | ✔ | ✔ |
| 138 | [sample-dropdown](138-sample-dropdown/) | DIVERGES | 26 | [✔](138-sample-dropdown/original.md) | [✔](138-sample-dropdown/ours.md) | ✔ | ✔ |
| 139 | [sample-tree](139-sample-tree/) | MATCH | 23 | [✔](139-sample-tree/original.md) | [✔](139-sample-tree/ours.md) | ✔ | ✔ |
| 140 | [sample-chips-kbd-code-badge](140-sample-chips-kbd-code-badge/) | MATCH | 18 | [✔](140-sample-chips-kbd-code-badge/original.md) | [✔](140-sample-chips-kbd-code-badge/ours.md) | ✔ | ✔ |
| 141 | [sample-toast-triggers](141-sample-toast-triggers/) | MATCH | 18 | [✔](141-sample-toast-triggers/original.md) | [✔](141-sample-toast-triggers/ours.md) | ✔ | ✔ |
| 142 | [sample-modal-triggers](142-sample-modal-triggers/) | MATCH | 23 | [✔](142-sample-modal-triggers/original.md) | [✔](142-sample-modal-triggers/ours.md) | ✔ | ✔ |
| 143 | [sample-external-toast-triggers](143-sample-external-toast-triggers/) | DIVERGES | 26 | [✔](143-sample-external-toast-triggers/original.md) | [✔](143-sample-external-toast-triggers/ours.md) | ✔ | ✔ |
| 144 | [sample-tooltip](144-sample-tooltip/) | MATCH | 18 | [✔](144-sample-tooltip/original.md) | [✔](144-sample-tooltip/ours.md) | ✔ | ✔ |
| 145 | [sample-block-wrapper](145-sample-block-wrapper/) | MATCH | 18 | [✔](145-sample-block-wrapper/original.md) | [✔](145-sample-block-wrapper/ours.md) | ✔ | ✔ |
| 146 | [sample-horizontal-tab-strip](146-sample-horizontal-tab-strip/) | DIVERGES | 26 | [✔](146-sample-horizontal-tab-strip/original.md) | [✔](146-sample-horizontal-tab-strip/ours.md) | ✔ | ✔ |
| 147 | [sample-vertical-icon-column](147-sample-vertical-icon-column/) | MATCH | 23 | [✔](147-sample-vertical-icon-column/original.md) | [✔](147-sample-vertical-icon-column/ours.md) | ✔ | ✔ |
| 148 | [sample-checkbox-dropdown](148-sample-checkbox-dropdown/) | MATCH | 18 | [✔](148-sample-checkbox-dropdown/original.md) | [✔](148-sample-checkbox-dropdown/ours.md) | ✔ | ✔ |
| 149 | [sample-context-menu](149-sample-context-menu/) | MATCH | 18 | [✔](149-sample-context-menu/original.md) | [✔](149-sample-context-menu/ours.md) | ✔ | ✔ |
| 150 | [sample-section-header](150-sample-section-header/) | MATCH | 18 | [✔](150-sample-section-header/original.md) | [✔](150-sample-section-header/ours.md) | ✔ | ✔ |
| 151 | [sample-status-bar-items](151-sample-status-bar-items/) | DIVERGES | 26 | [✔](151-sample-status-bar-items/original.md) | [✔](151-sample-status-bar-items/ours.md) | ✔ | ✔ |
| 152 | [sample-panel-icon-family](152-sample-panel-icon-family/) | MATCH | 18 | [✔](152-sample-panel-icon-family/original.md) | [✔](152-sample-panel-icon-family/ours.md) | ✔ | ✔ |
| 153 | [sample-placeholders](153-sample-placeholders/) | MATCH | 18 | [✔](153-sample-placeholders/original.md) | [✔](153-sample-placeholders/ours.md) | ✔ | ✔ |
| 154 | [global-scrollbar](154-global-scrollbar/) | MATCH | 18 | [✔](154-global-scrollbar/original.md) | [✔](154-global-scrollbar/ours.md) | ✔ | ✔ |
| 155 | [glint-surface-card-ring](155-glint-surface-card-ring/) | MATCH | 18 | [✔](155-glint-surface-card-ring/original.md) | [✔](155-glint-surface-card-ring/ours.md) | ✔ | ✔ |
| 156 | [focus-visible-ring](156-focus-visible-ring/) | DIVERGES | 26 | [✔](156-focus-visible-ring/original.md) | [✔](156-focus-visible-ring/ours.md) | — | ✔ |
| 157 | [activity-drop-highlight](157-activity-drop-highlight/) | MATCH | 23 | [✔](157-activity-drop-highlight/original.md) | [✔](157-activity-drop-highlight/ours.md) | — | ✔ |
| 158 | [dragging-body-classes](158-dragging-body-classes/) | MATCH | 18 | [✔](158-dragging-body-classes/original.md) | [✔](158-dragging-body-classes/ours.md) | — | — |
| 159 | [legacy-app-shell-css](159-legacy-app-shell-css/) | MATCH | 18 | [✔](159-legacy-app-shell-css/original.md) | [✔](159-legacy-app-shell-css/ours.md) | — | — |
