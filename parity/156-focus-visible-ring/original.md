# 156 focus-visible-ring — оригинал
Файлы: kamin-ide/src/renderer/theme/global.css:31-43; дубликат для вебвью — theme/skeleton.css:38-41

## Содержание/структура
Единый focus-ring только для клавиатурной навигации. Компоненты часто ставят `outline: none` для чистоты клика; здесь видимость восстанавливается для tab-пользователей — мышиный клик `:focus-visible` не триггерит.

## Метрики
Полные правила (global.css:34-43):
```css
:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
button:focus-visible,
[role='button']:focus-visible,
a:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
```
skeleton.css:38-41 (вебвью, только универсальное правило):
```css
:focus-visible {
  outline: 2px solid var(--accent-primary);
  outline-offset: 2px;
}
```
Значения (dark): outline 2px solid #89b4fa (`--accent-primary`); offset 2px.

## Состояния/варианты
Активен только при keyboard-focus (`:focus-visible`); при mouse-клике не показывается.

## Дополнение атрибутов (цикл 10)

- отступы: собственных padding/margin у правил нет; единственный отступ — `outline-offset: 2px` в обоих блоках (global.css:36, 42) и в вебвью-варианте (skeleton.css:40); глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12)
