# 135 sample-buttons — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:58-67, design-sections.module.css:195-235

## Содержание/структура
`ButtonsRow()` в Block «Buttons»: 4 кнопки `<button type=button>` — Primary (`.btnPrimary`), Secondary (`.btnSecondary`), Danger (`.btnDanger`), Ghost (`.btnGhost`).

## Метрики
Общее для всех 4 классов:
- padding `var(--space-1) var(--space-4)` = 4px 16px
- border-radius `--radius-sm` (8px)
- font: inherit; font-size `--fs-sm` (12px)
- cursor pointer; transition `background var(--transition-fast)` (150ms ease)

`.btnPrimary`: background `--accent-action` (#89b4fa dark); color `--accent-action-fg` (#313240); border none; font-weight 600.
`.btnSecondary`: background transparent; color `--text-primary`; border `1px solid var(--bg-overlay)` (#515567).
`.btnDanger`: background `--accent-red` (#f38ba8); color `--bg-primary` (#313240); border none; font-weight 600.
`.btnGhost`: background transparent; color `--text-secondary`; border `1px solid transparent`.

## Состояния/варианты
- `.btnPrimary:hover` → background `--accent-action-hover` (#74c7ec)
- `.btnSecondary:hover` → background `--bg-surface` (#3d3f51)
- `.btnDanger:hover` → background `--accent-maroon` (#eba0ac)
- `.btnGhost:hover` → background `--bg-surface`; color `--text-primary`
