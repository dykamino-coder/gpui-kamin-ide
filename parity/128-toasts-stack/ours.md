# 128 toasts-stack — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\ui\toasts.rs:57-180 (toast_card, toasts), crates\shell\src\overlay.rs:728-745 (позиция в overlay-окне), root.rs:1547-1556 (авто-скрытие 5s)

## Структура (gpui-дерево кратко)
```
overlay-обёртка: absolute.bottom(36).right(16).w(360) + hit_area()
└─ стек: flex_col.gap(8).w_full
   └─ card × N: items_start.gap(12)
      ├─ severity-codicon (info \u{ea74} | pass \u{eba4} | warning \u{ea6c} | error \u{ea87})
      ├─ content: (title) + message + (action-чипы)
      └─ dismiss 16×16 (codicon close)
```
Action-чип и dismiss тостов `shellreq-N` (showMessage) отвечают хосту (label / null). Не-sticky тосты авто-скрываются через 5s.

## Метрики (из кода, точные)
- Позиция: bottom 36, right 16 (SPACE_4), ширина обёртки 360
- Card: p 12 (SPACE_3) + px 16 (SPACE_4), rounded 12 (RADIUS_MD), bg p.bg_surface #3d3f51 a=.92, border 1 p.bg_surface a=.7, fs 12 (FS_SM)
- Иконка: 13px (FS_MD), mt 2, цвет severity: info p.accent_blue #89b4fa, success p.accent_green #a6e3a1, warning p.accent_yellow #f9e2af, error p.accent_red #f38ba8
- Title: weight 600, mb 2, p.text_primary; message p.text_secondary
- Actions: flex-wrap, gap 8, mt 8; чип px 12 py 2 rounded 4, border p.accent_primary a=.4, текст fs 11 p.accent_primary, hover bg accent a=.14
- Dismiss: 16×16, fs 11, p.text_disabled #60667b, hover text_primary

## Отличия от original.md той же папки
1. Фон карточки — почти непрозрачный bg_surface 92% вместо 50% + backdrop-blur(8px): blur невозможен в overlay-окне без альфы, поэтому тинт добит до непрозрачности.
2. Анимации slide-in 0.18s и `.leaving` slideOut — НЕ РЕАЛИЗОВАНЫ (появление/уход мгновенны).
3. box-shadow (shadow-card-popup) отсутствует.
4. max-width 360 стал ФИКСИРОВАННОЙ шириной обёртки (карточки всегда 360, у оригинала — по контенту до 360).
5. role=region/alert/status и aria-label нет.
6. Позиция bottom 36 / right space-4, gap 8, паддинги, severity-мэппинг иконок и цветов, чипы и dismiss — совпадают.
