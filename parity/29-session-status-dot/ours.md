# 29 session-status-dot — наша реализация
Файлы: `crates\shell\src\ui\sessions_list.rs:150-189` (внутри session_row)

## Структура (gpui-дерево кратко)
```
div#sdot-{sid} .flex_shrink_0 .w(size) .h(size) .rounded_full .bg(color)
  .when_some(status_tip, tooltip)
```
Источник — `session.metadata.bridgeStatus` / `bridgeWorking` (bool), `bridgeWorking` приоритетнее — 1:1.

## Метрики (из кода, точные)
- База 4×4px, radius full — 1:1
- working: **6×6px**, `accent_blue` #89b4fa, tooltip «Working…» — размер/цвет 1:1
- connected: `accent_green` #a6e3a1, «Online»
- connecting: `accent_yellow` #f9e2af, «Connecting…»
- error: `accent_red` #f38ba8, «Error»
- disconnected: `text_muted` #838aa0, «Offline»
- без статуса: active-строка → tab_color, иначе `text_muted` (без тултипа) — 1:1

## Отличия от original.md той же папки
1. **Анимация `bridgeWorkingPulse` (1.1s, opacity 0.5↔1, scale 1↔1.5) НЕ РЕАЛИЗОВАНА** — working-точка статична 6px.
2. Цвета из палитры Catppuccin (#a6e3a1 и т.д.) vs CSS-фоллбеки оригинала (#3fb950/#d29922/#f85149/#58a6ff) — фоллбеки в оригинале срабатывают только без темы, фактические переменные совпадают с нашей палитрой.
3. `aria-label` нет.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — точка 6×6 без содержимого и паддингов (`crates/shell/src/ui/sessions_list.rs`)
- гэпы: N/A: гэпы — детей нет; расстояние до имени задаёт `gap` строки сессии
