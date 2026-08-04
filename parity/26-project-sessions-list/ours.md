# 26 project-sessions-list — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:770-850` (сборка списков внутри sessions_sidebar)

## Структура (gpui-дерево кратко)
```
group: div .flex_col
├─ project_header(...)                  ← элемент 24
└─ .when(!collapsed)
   └─ sessions: div .flex_col .gap(2)
      ├─ active.map(session_row)        ← сортировка ПО АЛФАВИТУ (lowercase)
      ├─ .when(inact>0) inactive_toggle ← элемент 27
      └─ .when(show) inact.map(session_row)  ← сортировка по last_opened desc
```

## Метрики (из кода, точные)
- `.sessions`: flex-col, gap 2 — 1:1
- Прочих собственных стилей нет

## Отличия от original.md той же папки
1. **Empty-состояние «No sessions yet.» НЕ РЕАЛИЗОВАНО**: проект без сессий вообще пропускается (`if act.is_empty() && inact.is_empty() { continue; }`) — оригинал показывает группу с абзацем `.empty` (padding 2 0 2 18, fs-xs, text-muted).
2. Доп. поведение (в оригинале не описано): активные сортируются по алфавиту (стабильная позиция при клике), неактивные — свежие сверху.
3. Свёрнутость (`collapsed`) — 1:1: весь блок сессий не рендерится.

## Дополнение атрибутов (цикл 10)

- цвета: контейнер `.sessions` ни background, ни text_color не задаёт (`crates/shell/src/ui/sessions_list.rs:901` — только flex/flex_col/gap 2) — прозрачный, наследует text_size FS_SM = 12 и цвет от корня сайдбара (`sessions_list.rs:803`); единственный собственный цвет внутри контейнера — плашка «No sessions yet.» text_muted #838aa0 (`sessions_list.rs:910`); цвета строк задаёт `session_row`: покой text_secondary #adb3c7 (`sessions_list.rs:123`), hover bg = bg_surface #3d3f51 при альфе 0.55 + text_primary #cfd4e2 (`sessions_list.rs:106,169`), active — градиент tab_color 0.26 → 0.14 + border tab_color 0.45 (`sessions_list.rs:134-139`); свёрнутый inactive-хвост — text_disabled #60667b (`sessions_list.rs:607`)
