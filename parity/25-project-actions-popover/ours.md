# 25 project-actions-popover — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:382-417` (pill_wrap), `:344-376` (pill_btn), `:657-697` (project_actions_pill), `:863-872` (overlay_pill), `crates\shell\src\overlay.rs:747-781` (рендер в overlay-окне), `crates\shell\src\root.rs:1889-1903` (grace-закрытие)

## Структура (gpui-дерево кратко)
```
overlay-окно (поверх вебвью):
div .absolute .left(anchor.right + 4) .top(anchor.y − 2)
├─ hit_area()                          ← регион ввода overlay-окна
└─ pill_wrap#pill-p-{pid} .occlude .flex .items_center .gap(2) .p(3)
     .rounded(RADIUS_MD=12) .bg(bg_surface) .border_1(tint(text_primary,0.06))
     .shadow(0 4 16 rgba(0,0,0,0.35)) .on_hover(HoverPill grp:{pid})
   ├─ pill_btn codicon-add "New session here"
   └─ pill_btn codicon-trash "Delete project + its sessions" (danger)
```
pill_btn: 24×24, radius XS=4, codicon 13px, base `text_muted`, hover `bg tint(text_primary,0.12)` + `text_primary`; danger hover `bg tint(accent_red,0.16)` + `accent_red`. Показ/скрытие — state `hover_pill` (grace через generation-счётчик + отложенный сброс, а не CSS-мост).

## Метрики (из кода, точные)
- Обёртка: gap 2, padding 3, radius 12, `bg_surface` #3d3f51, border `text_primary @ 6%`, shadow 0 4 16 rgba(0,0,0,.35) — 1:1 c shadow-md
- Кнопка: 24×24, radius 4; offset от строки: +4 по x — 1:1 (POPOVER_OFFSET_PX=4)

## Отличия от original.md той же папки
1. **Иконки 13px vs оригинальные 14px** (`.popAction .codicon { font-size: 14px }` у ProjectGroup-версии; наш pill_btn общий с session-версией, где 13px).
2. Базовый цвет кнопок `text_muted` vs оригинальный `text-secondary`.
3. `.add:hover { color: accent-primary }` не реализован — add при ховере белеет (`text_primary`), а не синеет.
4. `.delete:hover` bg: у нас 16% accent_red, у оригинала 15%.
5. Вертикаль: `top = anchor.y − 2` (пилюля h=30 центрируется относительно строки 26) — оригинал позиционирует через clampToViewport(side:"right"); кламп к вьюпорту у нас ОТСУТСТВУЕТ (у правого края экрана пилюля может уехать за край).
6. Hover-мост `::before` (невидимые 10px слева) заменён event-driven механикой (HoverPill + generation grace) — поведенчески эквивалентно, но зазор 4px не является hit-зоной.
7. transition на кнопках нет (мгновенный hover).
8. Рендер в отдельном overlay-окне (пилюля живёт поверх вебвью) вместо `createPortal(document.body)` + z-dropdown.
