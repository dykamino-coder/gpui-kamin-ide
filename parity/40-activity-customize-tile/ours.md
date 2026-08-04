# 40 activity-customize-tile — наша реализация
Файлы: `crates/shell/src/ui/activity_bar.rs:149-155` (внутри `activity_bar()`, тот же `tile()` 58-119); обработчик `crates/shell/src/root.rs:1418-1420` (`ToggleCustomize`), передача `customize_active` — root.rs:5219-5224.

## Структура (gpui-дерево кратко)
```
activity_bar():
  └ tile("customize", "gear", customize_active, on_gear)   // ПЕРВЫЙ child после probe_area
      div#customize 32×32 rounded 8 → svg icons/gear.svg 18×18
```
- Клик → `ShellEvent::ToggleCustomize` (тумблер: открыт → закрыть, закрыт → открыть; ленивая подгрузка prefs хоста).
- Пока `customize_open`: в бар передаётся `active=None` — горит только gear, плитки тулов гаснут (root.rs:5217-5223).
- Не входит в `pinned[]`, не перетаскивается, без RMB — фиксированная системная плитка. Только в сайдбарном баре (в `rail()` правых карт её нет).

## Метрики (из кода, точные)
Идентичны элементу 39 (`tile()`):
- **32×32**, rounded **8px** (`RADIUS_SM`), иконка svg **18×18** (`icons/gear.svg`, vendored Phosphor).
- Базовый цвет `p.text_muted` #838aa0; hover: bg `p.bg_surface` #3d3f51 @0.5 + `p.text_primary` #cfd4e2.
- Active (`customize_open`): bg `p.accent_primary` #89b4fa @0.16, иконка #cfd4e2.
- Тултип «Customize» (`activity_label("customize")`).

## Отличия от original.md той же папки
1. Оригинал: `onClick = isActive ? leaveCustomize() : openCustomize("settings")`; у нас единый toggle `ToggleCustomize` — семантика совпадает (открытие всегда на последней выбранной подпанели `customize_panel`, не форс «settings»).
2. Hover-перекраска иконки на svg-ветке не работает (тот же дефект, что в 39, п.3) — gear на hover получает bg, но остаётся muted.
3. Нет `aria-pressed`/`aria-label`.
4. Позиция совпадает (первый элемент сверху); reverse-варианта у сайдбарного бара нет — не нужен (gear только в sidebar, sidebar всегда align=top).

## Дополнение атрибутов (цикл 10)

- отступы: своих паддингов у плитки нет — размер задан жёстко 32×32, содержимое центрировано flex (`crates/shell/src/ui/activity_bar.rs`, `fn tile`); внешние отступы даёт `.list` (gap 2) и `py SPACE_3` бара
- гэпы: N/A: гэпы — внутри плитки один ребёнок (глиф), gap нечему разделять
