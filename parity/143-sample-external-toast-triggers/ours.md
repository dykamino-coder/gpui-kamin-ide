# 143 sample-external-toast-triggers — наша реализация
Файлы: crates/shell/src/ui/design_samples.rs (`sample_external_toast_triggers`)

## Структура/содержание
Блок «External toasts (out-of-app)» с тем же hint'ом и 4 кнопками `Secondary`: Info (timed) / Success (timed) / Warning (sticky) / Error (with actions ["Retry","Show log"]) — title/message как в оригинале.

## Метрики (из кода, точные)
`ds_btn(Secondary)`: 4/16, radius 8, fs 12, рамка bg-overlay, hover bg-surface. Hint — `.compHint`: fs 11, lh 1.3, text-muted, отбивка снизу 4.

## Отличия от original.md той же папки
Внешние тосты портированы (ц.35): `crates/shell/src/toast.rs` — окно на тост (topmost, вне таскбара, без фокуса), стопка 380×140/16/150, очередь с «+N», авто-закрытие 8 с; `crates/shell/src/ui/toast_card.rs` — карточка с полосой обратного отсчёта и паузой по ховеру. Кнопки шлют `ShellEvent::ExternalToast`, а не строку внутреннего стека. Форма блока (кнопки, подписи, hint) — 1:1.

## Дополнение атрибутов (цикл 10)

- отступы: кнопки px 16 (SPACE_4) / py 4 (SPACE_1) (design_samples.rs:126-127); hint — margin-bottom 4 (SPACE_1) (design_panel.rs:105); у ряда-обёртки padding нет, gap 8 (design_samples.rs:415)
- цвета: все 4 кнопки Secondary — фон прозрачный, text p.text_primary #cfd4e2, border 1px p.bg_overlay #515567, hover bg p.bg_surface #3d3f51 (design_samples.rs:94-101); hint p.text_muted #838aa0 (design_panel.rs:108)
