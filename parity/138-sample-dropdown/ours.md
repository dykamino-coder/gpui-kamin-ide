# 138 sample-dropdown — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_dropdown`), design_panel.rs (блок «Dropdown menu»), root.rs (событие `DesignSample`, состояние `DesignState`)

## Структура/содержание
Форма ThemeQuickToggle 1:1:
```
div .relative .flex                         ← .dropdownAnchor
├─ ds_btn(Secondary) .flex .items_center .gap 8   ← .btnSecondary .dropdownTrigger
│   ├─ codicon color-mode 13 · "Theme" · codicon chevron-down 13
└─ (open) deferred(priority 60)             ← z-index: var(--z-dropdown)
    div .absolute .top(100%) .mt 4 .left 0 .min_w 220 .flex_col .gap 1 .p 4
        .rounded 12 .bg bg-mantle .shadow(dropdown)
    ├─ "BUILT-IN"                           ← .dropdownGroupLabel
    └─ 3 × item (dark / light / system)
```
Клик по триггеру — `DesignAction::ToggleDropdown`; по пункту — `Pick(id)` (выбор + закрытие). Стартовый picked = "dark".

## Метрики (из кода, точные)
- Меню: min-w 220, bg `--bg-mantle` #262533, radius RADIUS_MD 12, `shadows::dropdown()` = 0 4 16 rgba(0,0,0,.5), padding SPACE_1 4, gap 1.
- `.dropdownGroupLabel`: px 12 / py 4, fs FS_XS 11, uppercase (Rust `to_uppercase`), text-muted.
- `.dropdownItem`: gap 8, w-full, px 12 / py 8, radius RADIUS_SM 8, fs FS_SM 12, text-primary; глиф 13.
- `.dropdownItemHint`: JetBrains Mono, fs 11, text-muted.
- picked: bg accent-primary 12% + текст accent-primary + codicon-check; hover невыбранного: bg-surface 60%.

## Отличия от original.md той же папки
1. `letter-spacing .04em` у group-label в gpui недоступен (общий deviation порта).
2. Light-вариант picked (сплошная заливка accent + `--accent-action-fg` + weight 600) отдельной веткой не сделан — цвета берутся из активной палитры.
