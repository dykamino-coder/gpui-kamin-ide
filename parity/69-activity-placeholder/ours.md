# 69 activity-placeholder — наша реализация
Файлы: `crates/shell/src/ui/panel_placeholder.rs` (`fn activity_placeholder`)

## Структура/содержание
Пустое состояние УЖЕ выбранной активности, у которой ещё нет тела:
```
div (size-full, flex-col, items-center, justify-center)
├─ обёртка глифа (mb 4) → Phosphor-svg 36 или codicon 36
├─ заголовок активности
└─ «Nothing to show here yet.»
```
Путь Phosphor-иконки берётся ИЗ мапы `activity_bar::phosphor_path` (алиасы вроде «problems» дают `icons/warning.svg`), неизвестное имя уходит в codicon-фолбэк.

## Метрики (из кода, точные)
- Контейнер: gap SPACE_2 8, padding SPACE_5 20, центровка по обеим осям, цвет text-muted #838aa0.
- Глиф: 36×36, цвет text-disabled #60667b, отбивка снизу SPACE_1 4.
- Заголовок: fs FS_MD 13, weight 600, цвет text-primary #cfd4e2.
- Подпись: fs FS_XS 11, line-height 1.3, max-width 240, цвет наследуется (text-muted).
- Скругления: N/A: скругления — плейсхолдер рисуется на поверхности карты, своего фона и рамки не имеет.
- Ховер: N/A: ховер — состояние неинтерактивное.

## Отличия от original.md той же папки
1. `ToolIcon` оригинала умеет ветку `<img>` для URL-иконок расширений — у нас только Phosphor-ассет или codicon.
2. Кадра пары в досье нет: элемент виден лишь у активности без нативного тела.
