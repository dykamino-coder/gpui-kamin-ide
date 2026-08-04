# 22 sidebar-body-resolver — оригинал
Файлы: `%PROJECTS%\kamin-ide\src\renderer\components\sidebar\Sidebar.tsx` (81-85), без собственного CSS

## JSX-структура (кратко, вложенность)
```
function SidebarBody():
  id = getPanelSignal("sidebar").value.active
  if (!id) → <ActivityPlaceholder icon="circle-large" label="No tool selected"/>
  else    → <ActivityBody id={id} slot="sidebar"/>
```
Чисто логический компонент — выбирает тело по активной активности слота `sidebar`. Визуальные метрики принадлежат `ActivityPlaceholder` (элемент 69) и телам активностей.

## Метрики (ИЗ CSS, точные значения)
- Собственных стилей нет (нет css-модуля, нет классов, нет обёрточного DOM — рендерит ребёнка напрямую).

## Состояния (классы-варианты с метриками)
- `active == null` → `ActivityPlaceholder` с `icon="circle-large"`, `label="No tool selected"`.
- `active == id` → `<ActivityBody id slot="sidebar">` (сегодня реальная реализация только у `projects` → `SessionsMode`).

## Дополнение атрибутов (цикл 10)

- цвета: N/A: цвета — `SidebarBody()` чистый резолвер, собственного DOM и CSS-модуля нет, возвращает либо `<ActivityPlaceholder>`, либо `<ActivityBody>` (`sidebar/Sidebar.tsx:81-85`). Цвета фолбэк-ветки: `.placeholder { color: var(--text-muted) }` #838aa0, `.glyph { color: var(--text-disabled) }` #60667b, `.label { color: var(--text-primary) }` #cfd4e2, `.hint { color: var(--text-muted) }` #838aa0 (`panel-placeholder/ActivityPlaceholder.module.css`, блоки `.placeholder`/`.glyph`/`.label`/`.hint`)
- отступы: N/A: отступы — у резолвера своего бокса нет (`Sidebar.tsx:81-85`). Отступы фолбэк-ветки: `.placeholder` padding var(--space-5) = 20 + gap var(--space-2) = 8; `.glyph` margin-bottom var(--space-1) = 4; `.label`/`.hint` margin 0 (`ActivityPlaceholder.module.css`)
