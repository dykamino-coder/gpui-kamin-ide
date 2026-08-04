# 49 bottom-tab — наша реализация

Файлы: `crates/shell/src/ui/slot_panel.rs` (`tab`).

> Досье переписано в цикле 14: прежний текст говорил про кегль 12 без
> начертания, px 12 / gap 4, базовый text-muted, отсутствие ellipsis и
> `.tabDragging`, индикатор через `border_l_2`.

## Метрики (из кода, точные)
- Высота **24**, `px` **10**, `gap` **6**, `rounded` RADIUS_SM **8**.
- Кегль **11**, начертание `MEDIUM` (500), цвет `text_secondary` **#adb3c7**.
- Ховер (только у неактивного): фон `bg_surface`@**0.5** + `text_primary`.
- Активный: фон `accent_primary`@**0.16** + `text_primary`, ховер его не
  перебивает.
- Иконка тула **13** (единый резолв `tool_glyph`).
- Лейбл: `min_w 0` + `overflow_hidden` + `text_ellipsis` + `whitespace_nowrap`.
- Перетаскиваемый таб: `opacity 0.3` (`.tabDragging`).
- Движение с зажатой ЛКМ → позиционный индекс: левая половина «перед», правая
  «после» (опора — probe-регион `strip-<slot>-<index>`).
- ПКМ → меню тула.

## Отличия от original.md той же папки
1. `letter-spacing: .02em` — свойства в gpui нет.
2. Неизвестный id рисует таб «Tool» с иконкой gear, оригинал такой таб
   пропускает.
3. Тултипа с подписью тула нет.

## Атрибуты
- отступы: px 10, вертикальных нет (высота фиксирована 24)
- цвета: text-secondary #adb3c7 → ховер text-primary #cfd4e2 на bg-surface@50 %;
  активный accent-primary@16 % + text-primary
- шрифты: 11 / MEDIUM 500; иконка 13
- скругления: RADIUS_SM 8
- гэпы: 6 между иконкой и подписью
- ховер: bg-surface@50 % + text-primary у неактивного; активный держит
  accent@16 %
