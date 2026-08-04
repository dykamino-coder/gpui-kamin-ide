# 122 prompt-modal — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\modal.rs:14-27 (`Modal` + поле `placeholder`), 75-197 (`render_modal`), 110-116 (ширины prompt-режима), 155-177 (input-блок + Enter), 199-245 (кнопки); crates\shell\src\overlay.rs:1023-1042 (ленивое создание `InputState` в overlay-окне, `placeholder`, фокус), 1055-1070 (`ConfirmModalInput`); Esc — crates\shell\src\root.rs:5809, 5837-5838

## Структура/содержание
Отдельного компонента нет — это та же `ConfirmModal` (№121) с `prompt: Some(seed)`:
```
scrim div.absolute.size_full.flex.items_center.justify_center.bg(rgba(0,0,0,.6))  [клик = cancel]
└─ dialog div.relative.min_w(360).max_w(520).p(20).rounded(12)
          .bg(bg_primary).border_1(bg_surface).shadow(modal)   [клик внутри = stop_propagation]
   ├─ title  div.mb(12)
   ├─ body   div.mb(16)
   ├─ (prompt) input-блок div.mb(16).px(8).py(4).rounded(8).bg(bg_surface α .6).border_1(bg_overlay)
   │            ├─ on_key_down «enter» → stop_propagation + confirm
   │            └─ gpui_component::input::Input::new(input).appearance(false)
   └─ actions div.flex.gap(8).justify_end
      ├─ Cancel  (бордер bg_overlay, без фона)
      └─ Confirm (confirm_label; accent_action, danger → accent_red)
```
- `InputState` создаётся лениво в OVERLAY-окне (`InputState` требует `Window` того окна, где рендерится) и получает фокус (overlay.rs:1029-1037); при закрытии обнуляется — поэтому значение сбрасывается к `seed` при каждом открытии.
- Поле `Modal.placeholder: Option<SharedString>` доходит до `InputState::placeholder` (overlay.rs:1028-1033). Задано ТОЛЬКО в демо Design-панели (design_samples.rs:366, «e.g. my-extension»); боевые вызовы (`CreateEntry`, `RenameFs`, `SaveLayoutPreset`, `RenamePreset`) передают `None`.
- Подтверждение: `ConfirmModalInput(value)` → `run_modal_action`. Esc закрывает через `CloseOverlay` (root.rs:5837-5838).

## Метрики (из кода, точные)
- отступы: диалог p 20 (SPACE_5); input-блок px 8 (SPACE_2) / py 4; кнопки px 16 (SPACE_4) / py 4 (SPACE_1); отбивки — margin-bottom: заголовок 12 (SPACE_3), тело 16 (SPACE_4), input-блок 16 (SPACE_4)
- гэпы: ряд кнопок gap 8 (SPACE_2); вертикальных `gap` у диалога нет (всё на margin-bottom)
- цвета: скрим rgba(0,0,0,0.6) (= `--overlay-deep`); диалог bg p.bg_primary #313240 + border 1px p.bg_surface #3d3f51; заголовок p.text_primary #cfd4e2; тело p.text_secondary #adb3c7; input-блок bg p.bg_surface #3d3f51 α 0.6 + border 1px p.bg_overlay #515567; Cancel — текст p.text_primary #cfd4e2, border p.bg_overlay #515567; Confirm — bg p.accent_action #89b4fa (danger: p.accent_red #f38ba8), текст p.accent_action_fg #313240; placeholder красит vendored Input цветом `muted_foreground` СВОЕЙ темы, не нашей палитры (element.rs:956-959)
- скругления: диалог 12 (RADIUS_MD); input-блок 8 (RADIUS_SM); обе кнопки 8 (RADIUS_SM)
- шрифты: заголовок 13 (FS_MD) / 600 SEMIBOLD; тело 12 (FS_SM), line-height 15.6 (12 × 1.3); кнопки 12 (FS_SM) — Cancel weight 400, Confirm weight 600; сам `Input` размера не задаёт — наследует базовый кегль окна
- фоны по ховеру: Cancel — p.bg_surface #3d3f51 (сплошной); Confirm — p.accent_action_hover #74c7ec, в danger-режиме p.accent_maroon #eba0ac; у input-блока ни hover, ни focus-подсветки нет

## Отличия от original.md той же папки
1. Ширины 360 / 520 — совпадают с оригиналом (prompt шире confirm 320/480).
2. Enter в инпуте = сабмит — реализовано (modal.rs:169-174), совпадает с оригиналом; но у оригинала Enter блокируется при ошибке валидации, у нас сабмитит всегда.
3. Live-валидация НЕ РЕАЛИЗОВАНА: `validate` на каждый ввод, класс `.invalid` (border-color accent-red), inline `.error` (margin-top space-2, fs-xs, accent-red) и `disabled` у OK (opacity 0.5, cursor not-allowed) — ничего этого нет.
4. Инпут: bg `bg-surface 60%` + border `bg-overlay` вместо `bg-base` + border `bg-surface`; focus-подсветки `border-color: accent-primary` нет; `transition: border-color` нет.
5. `placeholder` поддержан в модели и доходит до `InputState`, но боевые prompt-вызовы его не задают — виден только в демо-блоке Design-панели.
6. Select-all значения при фокусе нет (overlay.rs:1037 делает только `window.focus`); сброс к `defaultValue` работает через пересоздание `InputState` при открытии — совпадает по эффекту.
7. Скрим 0.6 (= overlay-deep) совпадает; `animation: fadeIn 0.12s ease-out` отсутствует; `z-index: var(--z-modal)` → порядок детей overlay-слоя.
8. Восстановление фокуса при закрытии отсутствует.
9. Esc = cancel и клик по бэкдропу = cancel — совпадают с оригиналом.
10. Прочие общие расхождения №121 (роль `dialog`/`aria-modal` отсутствует) действуют и здесь.
