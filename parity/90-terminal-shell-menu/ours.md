# 90 terminal-shell-menu — наша реализация

Файлы: `crates/shell/src/ui/term_toolbar.rs:262-375` (дропдаун профилей
внутри `term_toolbar`), `crates/shell/src/term.rs` (`ShellProfile`,
`profiles()` — PowerShell / cmd / Git Bash, если найден).

## Структура (gpui-дерево)

```
gpui::deferred(menu).with_priority(60), occlude:
#term-shell-menu — absolute, top 29 + 6 (`POPUP_OFFSET_PX` от нижней кромки
  кнопки 28 в обёртке 30), right 0, min_w 200, flex-col, gap 1, p 4,
  rounded 12, bg bg_surface, border 1px, тень дропдауна,
  max_h = viewport_h − 16 (`calc(100vh - 16px)`), UI-шрифт
├─ профилей нет → px 12, py 8, FS_SM(12), text_muted «No shells discovered»
└─ на профиль #term-prof-{id}: flex, gap 8, px 12, py 8, rounded 8,
   FS_SM(12), text_primary, hover bg text_primary 10 %; клик → TermNew(id)
   ├─ itemIcon — бокс w 16, codicon-terminal `ea85` 16
   │    (у `.itemIcon` своего кегля нет → вендорная база 16)
   ├─ label — flex_1
   ├─ is_default → «DEFAULT»: FS_XS(11), text_muted
   └─ #term-star-{id} — 24×24, rounded 8, codicon 12
        `eb59` star-full (default) / `ea6a` star;
        цвет default → accent_primary, иначе text_muted;
        hover bg text_primary 10 % (+ text_primary у не-дефолтного);
        тултип «Default shell» / «Set as default»;
        клик → TermSetDefaultShell (персистится)
```

## Что закрыто (циклы 10-14)

`POPUP_OFFSET_PX` 6 вместо произвольного отступа, тень дропдауна, `min-width`
200 с ростом по контенту и `max-height` вьюпорта, пункт `8px 12px`, ховер
10 %, «DEFAULT» как muted-uppercase текст (не акцентная плашка), звезда
24×24 в `--accent-primary`, пустое состояние «No shells discovered».

## Осталось

1. Позиционирование — `absolute` под якорем через `deferred`, без
   двухпроходного измерения и `clampToViewport`: при узком окне меню может
   упереться в правую кромку (у оригинала оно поджимается).
2. Закрытие по Escape (сейчас — повторный клик по «+» либо клик мимо).
3. Иконка пункта всегда `codicon-terminal`; `s.icon` профиля не читается.

## Атрибуты (сверка ц.15)

- скругления: коробка меню — `--radius-md` 12, пункт и кнопка-звезда —
  `--radius-sm` 8.
