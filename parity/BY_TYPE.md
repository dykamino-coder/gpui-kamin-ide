# Инвентарь по ТИПАМ элементов

Тот же набор досье, что в `INDEX.md`, но разложенный по видам, а не по зонам экрана. В каждой папке: `original.md` (код и метрики оригинала), `ours.md` (наша реализация), `original.png` / `ours.png` (кадры), `verdict.md` (история циклов).

Генерируется: `python parity/by_type.py`.

## Кнопки — 15/17 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 5 | titlebar-command-search-button | MATCH | 19 | [05-titlebar-command-search-button/](05-titlebar-command-search-button/) |
| 6 | titlebar-window-controls-cluster | MATCH | 19 | [06-titlebar-window-controls-cluster/](06-titlebar-window-controls-cluster/) |
| 7 | titlebar-button | MATCH | 19 | [07-titlebar-button/](07-titlebar-button/) |
| 8 | titlebar-quick-actions-row | MATCH | 19 | [08-titlebar-quick-actions-row/](08-titlebar-quick-actions-row/) |
| 9 | titlebar-quick-action-button | MATCH | 23 | [09-titlebar-quick-action-button/](09-titlebar-quick-action-button/) |
| 10 | layout-toggles-trigger | MATCH | 19 | [10-layout-toggles-trigger/](10-layout-toggles-trigger/) |
| 11 | layout-toggles-menu | DIVERGES | 26 | [11-layout-toggles-menu/](11-layout-toggles-menu/) |
| 13 | theme-quick-toggle-trigger | MATCH | 19 | [13-theme-quick-toggle-trigger/](13-theme-quick-toggle-trigger/) |
| 25 | project-actions-popover | MATCH | 20 | [25-project-actions-popover/](25-project-actions-popover/) |
| 27 | project-inactive-toggle | MATCH | 20 | [27-project-inactive-toggle/](27-project-inactive-toggle/) |
| 30 | session-pin-button | MATCH | 20 | [30-session-pin-button/](30-session-pin-button/) |
| 32 | session-actions-popover | MATCH | 20 | [32-session-actions-popover/](32-session-actions-popover/) |
| 42 | activity-picker-dots-trigger | MATCH | 23 | [42-activity-picker-dots-trigger/](42-activity-picker-dots-trigger/) |
| 135 | sample-buttons | MATCH | 18 | [135-sample-buttons/](135-sample-buttons/) |
| 141 | sample-toast-triggers | MATCH | 18 | [141-sample-toast-triggers/](141-sample-toast-triggers/) |
| 142 | sample-modal-triggers | MATCH | 23 | [142-sample-modal-triggers/](142-sample-modal-triggers/) |
| 143 | sample-external-toast-triggers | DIVERGES | 26 | [143-sample-external-toast-triggers/](143-sample-external-toast-triggers/) |

## Иконки и глифы — 11/12 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 3 | titlebar-brand-logo | MATCH | 19 | [03-titlebar-brand-logo/](03-titlebar-brand-logo/) |
| 17 | panel-icon-svg | MATCH | 23 | [17-panel-icon-svg/](17-panel-icon-svg/) |
| 29 | session-status-dot | MATCH | 20 | [29-session-status-dot/](29-session-status-dot/) |
| 42 | activity-picker-dots-trigger | MATCH | 23 | [42-activity-picker-dots-trigger/](42-activity-picker-dots-trigger/) |
| 51 | tool-icon | MATCH | 23 | [51-tool-icon/](51-tool-icon/) |
| 97 | file-tree-row-badge | MATCH | 21 | [97-file-tree-row-badge/](97-file-tree-row-badge/) |
| 99 | tree-icon-img | MATCH | 23 | [99-tree-icon-img/](99-tree-icon-img/) |
| 107 | contributed-tree-node-icon | MATCH | 22 | [107-contributed-tree-node-icon/](107-contributed-tree-node-icon/) |
| 115 | webview-tab-icon | DIVERGES | 26 | [115-webview-tab-icon/](115-webview-tab-icon/) |
| 140 | sample-chips-kbd-code-badge | MATCH | 18 | [140-sample-chips-kbd-code-badge/](140-sample-chips-kbd-code-badge/) |
| 147 | sample-vertical-icon-column | MATCH | 23 | [147-sample-vertical-icon-column/](147-sample-vertical-icon-column/) |
| 152 | sample-panel-icon-family | MATCH | 18 | [152-sample-panel-icon-family/](152-sample-panel-icon-family/) |

## Инпуты и селекторы — 8/10 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 5 | titlebar-command-search-button | MATCH | 19 | [05-titlebar-command-search-button/](05-titlebar-command-search-button/) |
| 31 | session-rename-input | DIVERGES | 26 | [31-session-rename-input/](31-session-rename-input/) |
| 42 | activity-picker-dots-trigger | MATCH | 23 | [42-activity-picker-dots-trigger/](42-activity-picker-dots-trigger/) |
| 43 | activity-picker-open-tool-pill | MATCH | 21 | [43-activity-picker-open-tool-pill/](43-activity-picker-open-tool-pill/) |
| 44 | activity-picker-menu | MATCH | 21 | [44-activity-picker-menu/](44-activity-picker-menu/) |
| 106 | contributed-tree-checkbox | MATCH | 23 | [106-contributed-tree-checkbox/](106-contributed-tree-checkbox/) |
| 122 | prompt-modal | MATCH | 23 | [122-prompt-modal/](122-prompt-modal/) |
| 137 | sample-input | MATCH | 24 | [137-sample-input/](137-sample-input/) |
| 138 | sample-dropdown | DIVERGES | 26 | [138-sample-dropdown/](138-sample-dropdown/) |
| 148 | sample-checkbox-dropdown | MATCH | 18 | [148-sample-checkbox-dropdown/](148-sample-checkbox-dropdown/) |

## Плейсхолдеры и пустые состояния — 10/11 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 41 | activity-drop-placeholder | MATCH | 21 | [41-activity-drop-placeholder/](41-activity-drop-placeholder/) |
| 50 | bottom-tab-drop-placeholder | MATCH | 21 | [50-bottom-tab-drop-placeholder/](50-bottom-tab-drop-placeholder/) |
| 68 | panel-placeholder | MATCH | 19 | [68-panel-placeholder/](68-panel-placeholder/) |
| 69 | activity-placeholder | MATCH | 19 | [69-activity-placeholder/](69-activity-placeholder/) |
| 70 | webview-loading-skeleton | MATCH | 23 | [70-webview-loading-skeleton/](70-webview-loading-skeleton/) |
| 71 | webview-load-error | MATCH | 19 | [71-webview-load-error/](71-webview-load-error/) |
| 72 | chat-switch-skeleton | MATCH | 23 | [72-chat-switch-skeleton/](72-chat-switch-skeleton/) |
| 77 | welcome-placeholder | DIVERGES | 26 | [77-welcome-placeholder/](77-welcome-placeholder/) |
| 93 | file-tree-empty-state | MATCH | 21 | [93-file-tree-empty-state/](93-file-tree-empty-state/) |
| 109 | file-viewer-empty | MATCH | 20 | [109-file-viewer-empty/](109-file-viewer-empty/) |
| 153 | sample-placeholders | MATCH | 18 | [153-sample-placeholders/](153-sample-placeholders/) |

## Меню и поповеры — 14/17 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 11 | layout-toggles-menu | DIVERGES | 26 | [11-layout-toggles-menu/](11-layout-toggles-menu/) |
| 14 | theme-popover | MATCH | 19 | [14-theme-popover/](14-theme-popover/) |
| 15 | theme-popover-column | DIVERGES | 26 | [15-theme-popover-column/](15-theme-popover-column/) |
| 16 | theme-popover-item | MATCH | 19 | [16-theme-popover-item/](16-theme-popover-item/) |
| 25 | project-actions-popover | MATCH | 20 | [25-project-actions-popover/](25-project-actions-popover/) |
| 32 | session-actions-popover | MATCH | 20 | [32-session-actions-popover/](32-session-actions-popover/) |
| 33 | session-context-menu | MATCH | 23 | [33-session-context-menu/](33-session-context-menu/) |
| 44 | activity-picker-menu | MATCH | 21 | [44-activity-picker-menu/](44-activity-picker-menu/) |
| 45 | activity-context-menu | MATCH | 23 | [45-activity-context-menu/](45-activity-context-menu/) |
| 46 | activity-context-submenu | MATCH | 23 | [46-activity-context-submenu/](46-activity-context-submenu/) |
| 90 | terminal-shell-menu | MATCH | 23 | [90-terminal-shell-menu/](90-terminal-shell-menu/) |
| 100 | file-context-menu | DIVERGES | 25 | [100-file-context-menu/](100-file-context-menu/) |
| 101 | file-context-submenu | MATCH | 24 | [101-file-context-submenu/](101-file-context-submenu/) |
| 112 | file-viewer-tabs-overflow | MATCH | 23 | [112-file-viewer-tabs-overflow/](112-file-viewer-tabs-overflow/) |
| 129 | tooltip | MATCH | 20 | [129-tooltip/](129-tooltip/) |
| 144 | sample-tooltip | MATCH | 18 | [144-sample-tooltip/](144-sample-tooltip/) |
| 149 | sample-context-menu | MATCH | 18 | [149-sample-context-menu/](149-sample-context-menu/) |

## Строки списков и деревьев — 23/32 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 4 | titlebar-tabs-slot | MATCH | 19 | [04-titlebar-tabs-slot/](04-titlebar-tabs-slot/) |
| 8 | titlebar-quick-actions-row | MATCH | 19 | [08-titlebar-quick-actions-row/](08-titlebar-quick-actions-row/) |
| 16 | theme-popover-item | MATCH | 19 | [16-theme-popover-item/](16-theme-popover-item/) |
| 18 | session-tabs-strip | DIVERGES | 26 | [18-session-tabs-strip/](18-session-tabs-strip/) |
| 19 | session-tab-chip | DIVERGES | 26 | [19-session-tab-chip/](19-session-tab-chip/) |
| 28 | session-item-row | DIVERGES | 26 | [28-session-item-row/](28-session-item-row/) |
| 36 | customize-nav-item | MATCH | 20 | [36-customize-nav-item/](36-customize-nav-item/) |
| 39 | activity-tile | MATCH | 23 | [39-activity-tile/](39-activity-tile/) |
| 40 | activity-customize-tile | MATCH | 21 | [40-activity-customize-tile/](40-activity-customize-tile/) |
| 48 | bottom-tab-bar-strip | MATCH | 21 | [48-bottom-tab-bar-strip/](48-bottom-tab-bar-strip/) |
| 49 | bottom-tab | MATCH | 23 | [49-bottom-tab/](49-bottom-tab/) |
| 50 | bottom-tab-drop-placeholder | MATCH | 21 | [50-bottom-tab-drop-placeholder/](50-bottom-tab-drop-placeholder/) |
| 66 | file-panel-mode-tabs | MATCH | 19 | [66-file-panel-mode-tabs/](66-file-panel-mode-tabs/) |
| 67 | browser-pane | MATCH | 24 | [67-browser-pane/](67-browser-pane/) |
| 85 | extension-row | MATCH | 23 | [85-extension-row/](85-extension-row/) |
| 87 | problem-row | MATCH | 19 | [87-problem-row/](87-problem-row/) |
| 94 | file-tree-folder-row | MATCH | 23 | [94-file-tree-folder-row/](94-file-tree-folder-row/) |
| 95 | file-tree-file-row | DIVERGES | 25 | [95-file-tree-file-row/](95-file-tree-file-row/) |
| 97 | file-tree-row-badge | MATCH | 21 | [97-file-tree-row-badge/](97-file-tree-row-badge/) |
| 103 | generic-tree-row | MATCH | 21 | [103-generic-tree-row/](103-generic-tree-row/) |
| 105 | contributed-tree-node-row | MATCH | 23 | [105-contributed-tree-node-row/](105-contributed-tree-node-row/) |
| 107 | contributed-tree-node-icon | MATCH | 22 | [107-contributed-tree-node-icon/](107-contributed-tree-node-icon/) |
| 110 | file-viewer-tabs-strip | MATCH | 23 | [110-file-viewer-tabs-strip/](110-file-viewer-tabs-strip/) |
| 111 | file-viewer-tab | DIVERGES | 26 | [111-file-viewer-tab/](111-file-viewer-tab/) |
| 112 | file-viewer-tabs-overflow | MATCH | 23 | [112-file-viewer-tabs-overflow/](112-file-viewer-tabs-overflow/) |
| 115 | webview-tab-icon | DIVERGES | 26 | [115-webview-tab-icon/](115-webview-tab-icon/) |
| 117 | status-item-builtin | DIVERGES | 26 | [117-status-item-builtin/](117-status-item-builtin/) |
| 118 | status-item-contributed | MATCH | 24 | [118-status-item-contributed/](118-status-item-contributed/) |
| 136 | sample-list-item | MATCH | 18 | [136-sample-list-item/](136-sample-list-item/) |
| 140 | sample-chips-kbd-code-badge | MATCH | 18 | [140-sample-chips-kbd-code-badge/](140-sample-chips-kbd-code-badge/) |
| 146 | sample-horizontal-tab-strip | DIVERGES | 26 | [146-sample-horizontal-tab-strip/](146-sample-horizontal-tab-strip/) |
| 151 | sample-status-bar-items | DIVERGES | 26 | [151-sample-status-bar-items/](151-sample-status-bar-items/) |

## Панели, карты и колонки — 52/66 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 1 | titlebar | MATCH | 19 | [01-titlebar/](01-titlebar/) |
| 2 | titlebar-left-cluster | DIVERGES | 26 | [02-titlebar-left-cluster/](02-titlebar-left-cluster/) |
| 3 | titlebar-brand-logo | MATCH | 19 | [03-titlebar-brand-logo/](03-titlebar-brand-logo/) |
| 4 | titlebar-tabs-slot | MATCH | 19 | [04-titlebar-tabs-slot/](04-titlebar-tabs-slot/) |
| 5 | titlebar-command-search-button | MATCH | 19 | [05-titlebar-command-search-button/](05-titlebar-command-search-button/) |
| 6 | titlebar-window-controls-cluster | MATCH | 19 | [06-titlebar-window-controls-cluster/](06-titlebar-window-controls-cluster/) |
| 7 | titlebar-button | MATCH | 19 | [07-titlebar-button/](07-titlebar-button/) |
| 8 | titlebar-quick-actions-row | MATCH | 19 | [08-titlebar-quick-actions-row/](08-titlebar-quick-actions-row/) |
| 9 | titlebar-quick-action-button | MATCH | 23 | [09-titlebar-quick-action-button/](09-titlebar-quick-action-button/) |
| 15 | theme-popover-column | DIVERGES | 26 | [15-theme-popover-column/](15-theme-popover-column/) |
| 17 | panel-icon-svg | MATCH | 23 | [17-panel-icon-svg/](17-panel-icon-svg/) |
| 18 | session-tabs-strip | DIVERGES | 26 | [18-session-tabs-strip/](18-session-tabs-strip/) |
| 20 | sidebar-root | MATCH | 20 | [20-sidebar-root/](20-sidebar-root/) |
| 21 | sidebar-resize-handle | MATCH | 20 | [21-sidebar-resize-handle/](21-sidebar-resize-handle/) |
| 22 | sidebar-body-resolver | MATCH | 20 | [22-sidebar-body-resolver/](22-sidebar-body-resolver/) |
| 24 | project-group-header | MATCH | 23 | [24-project-group-header/](24-project-group-header/) |
| 38 | activity-bar-nav | MATCH | 23 | [38-activity-bar-nav/](38-activity-bar-nav/) |
| 48 | bottom-tab-bar-strip | MATCH | 21 | [48-bottom-tab-bar-strip/](48-bottom-tab-bar-strip/) |
| 54 | main-bottom-panel | MATCH | 19 | [54-main-bottom-panel/](54-main-bottom-panel/) |
| 56 | right-panel-column | DIVERGES | 26 | [56-right-panel-column/](56-right-panel-column/) |
| 57 | right-panel-width-handle | MATCH | 19 | [57-right-panel-width-handle/](57-right-panel-width-handle/) |
| 58 | right-panel-top-card | MATCH | 19 | [58-right-panel-top-card/](58-right-panel-top-card/) |
| 59 | right-panel-split-handle | MATCH | 19 | [59-right-panel-split-handle/](59-right-panel-split-handle/) |
| 60 | right-panel-bottom-card | MATCH | 19 | [60-right-panel-bottom-card/](60-right-panel-bottom-card/) |
| 61 | file-panel-column | MATCH | 19 | [61-file-panel-column/](61-file-panel-column/) |
| 62 | file-panel-width-handle | MATCH | 19 | [62-file-panel-width-handle/](62-file-panel-width-handle/) |
| 63 | file-panel-top-card | MATCH | 23 | [63-file-panel-top-card/](63-file-panel-top-card/) |
| 64 | file-panel-split-handle | MATCH | 23 | [64-file-panel-split-handle/](64-file-panel-split-handle/) |
| 65 | file-panel-bottom-card | MATCH | 23 | [65-file-panel-bottom-card/](65-file-panel-bottom-card/) |
| 66 | file-panel-mode-tabs | MATCH | 19 | [66-file-panel-mode-tabs/](66-file-panel-mode-tabs/) |
| 68 | panel-placeholder | MATCH | 19 | [68-panel-placeholder/](68-panel-placeholder/) |
| 70 | webview-loading-skeleton | MATCH | 23 | [70-webview-loading-skeleton/](70-webview-loading-skeleton/) |
| 71 | webview-load-error | MATCH | 19 | [71-webview-load-error/](71-webview-load-error/) |
| 73 | contributed-container-body | MATCH | 22 | [73-contributed-container-body/](73-contributed-container-body/) |
| 74 | contributed-view-section | MATCH | 19 | [74-contributed-view-section/](74-contributed-view-section/) |
| 75 | webview-view-anchor | MATCH | 23 | [75-webview-view-anchor/](75-webview-view-anchor/) |
| 76 | persistent-webview-layer | MATCH | 23 | [76-persistent-webview-layer/](76-persistent-webview-layer/) |
| 78 | customize-content-panel | MATCH | 23 | [78-customize-content-panel/](78-customize-content-panel/) |
| 79 | design-panel-shell | MATCH | 23 | [79-design-panel-shell/](79-design-panel-shell/) |
| 80 | logs-panel | DIVERGES | 26 | [80-logs-panel/](80-logs-panel/) |
| 81 | system-log-panel | DIVERGES | 26 | [81-system-log-panel/](81-system-log-panel/) |
| 82 | settings-panel | DIVERGES | 26 | [82-settings-panel/](82-settings-panel/) |
| 83 | legacy-bridge-card | MATCH | 23 | [83-legacy-bridge-card/](83-legacy-bridge-card/) |
| 84 | extensions-panel | MATCH | 19 | [84-extensions-panel/](84-extensions-panel/) |
| 86 | problems-panel | MATCH | 19 | [86-problems-panel/](86-problems-panel/) |
| 88 | terminal-view | MATCH | 19 | [88-terminal-view/](88-terminal-view/) |
| 89 | terminal-toolbar | MATCH | 23 | [89-terminal-toolbar/](89-terminal-toolbar/) |
| 98 | file-tree-header-toolbar | DIVERGES | 25 | [98-file-tree-header-toolbar/](98-file-tree-header-toolbar/) |
| 104 | contributed-tree-view-body | MATCH | 22 | [104-contributed-tree-view-body/](104-contributed-tree-view-body/) |
| 108 | file-viewer-wrapper | DIVERGES | 26 | [108-file-viewer-wrapper/](108-file-viewer-wrapper/) |
| 109 | file-viewer-empty | MATCH | 20 | [109-file-viewer-empty/](109-file-viewer-empty/) |
| 110 | file-viewer-tabs-strip | MATCH | 23 | [110-file-viewer-tabs-strip/](110-file-viewer-tabs-strip/) |
| 111 | file-viewer-tab | DIVERGES | 26 | [111-file-viewer-tab/](111-file-viewer-tab/) |
| 112 | file-viewer-tabs-overflow | MATCH | 23 | [112-file-viewer-tabs-overflow/](112-file-viewer-tabs-overflow/) |
| 114 | webview-panel-view | DIVERGES | 26 | [114-webview-panel-view/](114-webview-panel-view/) |
| 115 | webview-tab-icon | DIVERGES | 26 | [115-webview-tab-icon/](115-webview-tab-icon/) |
| 116 | status-bar-root | MATCH | 17 | [116-status-bar-root/](116-status-bar-root/) |
| 145 | sample-block-wrapper | MATCH | 18 | [145-sample-block-wrapper/](145-sample-block-wrapper/) |
| 146 | sample-horizontal-tab-strip | DIVERGES | 26 | [146-sample-horizontal-tab-strip/](146-sample-horizontal-tab-strip/) |
| 147 | sample-vertical-icon-column | MATCH | 23 | [147-sample-vertical-icon-column/](147-sample-vertical-icon-column/) |
| 150 | sample-section-header | MATCH | 18 | [150-sample-section-header/](150-sample-section-header/) |
| 151 | sample-status-bar-items | DIVERGES | 26 | [151-sample-status-bar-items/](151-sample-status-bar-items/) |
| 152 | sample-panel-icon-family | MATCH | 18 | [152-sample-panel-icon-family/](152-sample-panel-icon-family/) |
| 154 | global-scrollbar | MATCH | 18 | [154-global-scrollbar/](154-global-scrollbar/) |
| 155 | glint-surface-card-ring | MATCH | 18 | [155-glint-surface-card-ring/](155-glint-surface-card-ring/) |
| 158 | dragging-body-classes | MATCH | 18 | [158-dragging-body-classes/](158-dragging-body-classes/) |

## Оверлеи и модалки — 8/12 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 8 | titlebar-quick-actions-row | MATCH | 19 | [08-titlebar-quick-actions-row/](08-titlebar-quick-actions-row/) |
| 9 | titlebar-quick-action-button | MATCH | 23 | [09-titlebar-quick-action-button/](09-titlebar-quick-action-button/) |
| 13 | theme-quick-toggle-trigger | MATCH | 19 | [13-theme-quick-toggle-trigger/](13-theme-quick-toggle-trigger/) |
| 121 | confirm-modal | DIVERGES | 26 | [121-confirm-modal/](121-confirm-modal/) |
| 122 | prompt-modal | MATCH | 23 | [122-prompt-modal/](122-prompt-modal/) |
| 123 | quick-pick-modal | DIVERGES | 26 | [123-quick-pick-modal/](123-quick-pick-modal/) |
| 124 | quick-open | MATCH | 26 | [124-quick-open/](124-quick-open/) |
| 127 | command-palette | DIVERGES | 26 | [127-command-palette/](127-command-palette/) |
| 128 | toasts-stack | MATCH | 20 | [128-toasts-stack/](128-toasts-stack/) |
| 141 | sample-toast-triggers | MATCH | 18 | [141-sample-toast-triggers/](141-sample-toast-triggers/) |
| 142 | sample-modal-triggers | MATCH | 23 | [142-sample-modal-triggers/](142-sample-modal-triggers/) |
| 143 | sample-external-toast-triggers | DIVERGES | 26 | [143-sample-external-toast-triggers/](143-sample-external-toast-triggers/) |

## Токены, семплы, глобальные стили — 22/27 MATCH

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 34 | session-color-swatches | MATCH | 20 | [34-session-color-swatches/](34-session-color-swatches/) |
| 130 | design-color-tokens | MATCH | 18 | [130-design-color-tokens/](130-design-color-tokens/) |
| 131 | design-typography-tokens | MATCH | 18 | [131-design-typography-tokens/](131-design-typography-tokens/) |
| 132 | design-spacing-tokens | MATCH | 18 | [132-design-spacing-tokens/](132-design-spacing-tokens/) |
| 133 | design-radius-tokens | MATCH | 18 | [133-design-radius-tokens/](133-design-radius-tokens/) |
| 134 | design-shadow-tokens | MATCH | 18 | [134-design-shadow-tokens/](134-design-shadow-tokens/) |
| 135 | sample-buttons | MATCH | 18 | [135-sample-buttons/](135-sample-buttons/) |
| 136 | sample-list-item | MATCH | 18 | [136-sample-list-item/](136-sample-list-item/) |
| 137 | sample-input | MATCH | 24 | [137-sample-input/](137-sample-input/) |
| 138 | sample-dropdown | DIVERGES | 26 | [138-sample-dropdown/](138-sample-dropdown/) |
| 139 | sample-tree | MATCH | 23 | [139-sample-tree/](139-sample-tree/) |
| 140 | sample-chips-kbd-code-badge | MATCH | 18 | [140-sample-chips-kbd-code-badge/](140-sample-chips-kbd-code-badge/) |
| 141 | sample-toast-triggers | MATCH | 18 | [141-sample-toast-triggers/](141-sample-toast-triggers/) |
| 142 | sample-modal-triggers | MATCH | 23 | [142-sample-modal-triggers/](142-sample-modal-triggers/) |
| 143 | sample-external-toast-triggers | DIVERGES | 26 | [143-sample-external-toast-triggers/](143-sample-external-toast-triggers/) |
| 144 | sample-tooltip | MATCH | 18 | [144-sample-tooltip/](144-sample-tooltip/) |
| 145 | sample-block-wrapper | MATCH | 18 | [145-sample-block-wrapper/](145-sample-block-wrapper/) |
| 146 | sample-horizontal-tab-strip | DIVERGES | 26 | [146-sample-horizontal-tab-strip/](146-sample-horizontal-tab-strip/) |
| 147 | sample-vertical-icon-column | MATCH | 23 | [147-sample-vertical-icon-column/](147-sample-vertical-icon-column/) |
| 148 | sample-checkbox-dropdown | MATCH | 18 | [148-sample-checkbox-dropdown/](148-sample-checkbox-dropdown/) |
| 149 | sample-context-menu | MATCH | 18 | [149-sample-context-menu/](149-sample-context-menu/) |
| 150 | sample-section-header | MATCH | 18 | [150-sample-section-header/](150-sample-section-header/) |
| 151 | sample-status-bar-items | DIVERGES | 26 | [151-sample-status-bar-items/](151-sample-status-bar-items/) |
| 152 | sample-panel-icon-family | MATCH | 18 | [152-sample-panel-icon-family/](152-sample-panel-icon-family/) |
| 153 | sample-placeholders | MATCH | 18 | [153-sample-placeholders/](153-sample-placeholders/) |
| 154 | global-scrollbar | MATCH | 18 | [154-global-scrollbar/](154-global-scrollbar/) |
| 156 | focus-visible-ring | DIVERGES | 26 | [156-focus-visible-ring/](156-focus-visible-ring/) |

## Прочее — 20

| # | элемент | вердикт | цикл | досье |
|---|---|---|---|---|
| 12 | layout-presets-section | MATCH | 19 | [12-layout-presets-section/](12-layout-presets-section/) |
| 23 | sessions-mode-root | DIVERGES | 26 | [23-sessions-mode-root/](23-sessions-mode-root/) |
| 26 | project-sessions-list | MATCH | 20 | [26-project-sessions-list/](26-project-sessions-list/) |
| 35 | customize-mode-nav | MATCH | 20 | [35-customize-mode-nav/](35-customize-mode-nav/) |
| 37 | customize-contributed-tree | MATCH | 20 | [37-customize-contributed-tree/](37-customize-contributed-tree/) |
| 47 | activity-drag-ghost | MATCH | 21 | [47-activity-drag-ghost/](47-activity-drag-ghost/) |
| 52 | app-shell | MATCH | 19 | [52-app-shell/](52-app-shell/) |
| 53 | main-content | MATCH | 19 | [53-main-content/](53-main-content/) |
| 55 | main-bottom-resize-handle | MATCH | 19 | [55-main-bottom-resize-handle/](55-main-bottom-resize-handle/) |
| 91 | terminal-session-host | MATCH | 23 | [91-terminal-session-host/](91-terminal-session-host/) |
| 92 | file-tree-root | DIVERGES | 25 | [92-file-tree-root/](92-file-tree-root/) |
| 96 | file-tree-children-states | MATCH | 23 | [96-file-tree-children-states/](96-file-tree-children-states/) |
| 102 | generic-tree | MATCH | 21 | [102-generic-tree/](102-generic-tree/) |
| 113 | monaco-editor-host | MATCH | 23 | [113-monaco-editor-host/](113-monaco-editor-host/) |
| 119 | status-editor-encoding-eol | MATCH | 20 | [119-status-editor-encoding-eol/](119-status-editor-encoding-eol/) |
| 120 | status-version-update | MATCH | 23 | [120-status-version-update/](120-status-version-update/) |
| 125 | find-in-files | DIVERGES | 26 | [125-find-in-files/](125-find-in-files/) |
| 126 | workspace-symbols | DIVERGES | 26 | [126-workspace-symbols/](126-workspace-symbols/) |
| 157 | activity-drop-highlight | MATCH | 23 | [157-activity-drop-highlight/](157-activity-drop-highlight/) |
| 159 | legacy-app-shell-css | MATCH | 18 | [159-legacy-app-shell-css/](159-legacy-app-shell-css/) |
