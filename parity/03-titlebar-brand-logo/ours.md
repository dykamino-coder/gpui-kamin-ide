# 03 titlebar-brand-logo — наша реализация

Файлы: crates/shell/src/ui/titlebar.rs:183-197

## Структура (gpui-дерево кратко)
```
div (.brand-аналог, 42×42, flex center, flex_shrink_0)
 └ img("icons/kaminoid.svg") 26×26
```

## Метрики (из кода, точные)
- бокс: w/h = m::TITLEBAR_HEIGHT (42.0) — квадрат = высоте титлбара
- лого: img 26×26 (px(26.0))
- отступы/скругления: нет
- цвета: не заданы (img самодостаточен)
- hover/active: нет (не интерактивен)

## Отличия от original.md той же папки
1. color: var(--accent-primary) на .brand не задан — у нас img, цвет не нужен
   (оригинальный codicon-fallback 18px тоже не реализован — некритично, в
   проде рендерится img).
2. draggable=false / user-select — не применимо (gpui img не перетаскивается).
3. aria-hidden — не применимо (нет accessibility-дерева).
Метрики (42×42 бокс, лого 26×26, flex center) — совпадают полностью.

## Дополнение атрибутов (цикл 10)

- шрифты: N/A: шрифты — brand-слот содержит только `img("icons/kaminoid.svg")` 26×26, текста и глифов нет (`crates/shell/src/ui/titlebar.rs:202-213`); своего text_size нет, наследует FS_SM = 12 корня (`titlebar.rs:197`). Отклонение от оригинала: у него в слоте есть codicon-ветка 18px (`titlebar/Titlebar.module.css:29`), у нас её нет
