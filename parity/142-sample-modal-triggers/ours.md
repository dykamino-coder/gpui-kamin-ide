# 142 sample-modal-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_modal_triggers`), ui/modal.rs (`ModalAction::Noop`)

## Структура/содержание
3 кнопки: Confirm (`Secondary`), Confirm danger (`Danger`, danger + confirmLabel «Delete»), Prompt (`Secondary`, prompt-режим с инпутом). Каждая шлёт `ShellEvent::OpenModal(Modal{..})`, действие — `ModalAction::Noop`.

## Метрики (из кода, точные)
См. `ds_btn`: 4/16, radius 8, fs 12. Danger = bg accent-red, цвет bg-primary, weight 600, hover accent-maroon.

## Отличия от original.md той же папки
`bodyHtml` оригинала содержит разметку (`<code>`, `<strong>`); наша модалка принимает простой текст — теги сняты, содержание то же.

## Дополнение атрибутов (цикл 10)

- отступы: кнопки px 16 (SPACE_4) / py 4 (SPACE_1) (design_samples.rs:126-127); у ряда-обёртки padding/margin нет, только flex-wrap gap 8 (design_samples.rs:324-327)
- цвета: Secondary — фон прозрачный, text p.text_primary #cfd4e2, border 1px p.bg_overlay #515567, hover bg p.bg_surface #3d3f51 (design_samples.rs:94-101); Danger — bg p.accent_red #f38ba8, text p.bg_primary #313240, hover bg p.accent_maroon #eba0ac (design_samples.rs:102-109)
