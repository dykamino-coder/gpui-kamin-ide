# 51 tool-icon — наша реализация
Единого компонента НЕТ — рендер иконки тула продублирован в 4 местах:
Файлы: `crates/shell/src/ui/activity_bar.rs:16-38` (`phosphor_path()` + `codicon_glyph()`), `crates/shell/src/ui/tool_picker.rs:27-52` (`tool_icon()`), `crates/shell/src/ui/slot_panel.rs:85-104` (иконка таба), `crates/shell/src/ui/right_column.rs:65-78` (иконка рейл-плитки); общие помощники `crates/shell/src/ui/icon.rs` (`codicon()`, бокс 16×16) и `crates/shell/src/ui/codicon_map.rs` (`codicon_by_name`).

## Структура (две ветки vs три у оригинала)
```
1. phosphor_path(icon) → Some("icons/<token>.svg")
   gpui::svg().path(...).w/h(size).text_color(...)      // vendored Phosphor, currentColor-аналог
   токены: folders(projects), tree-view(tree), search, warning(problems),
           terminal, gear(customize)
2. иначе codicon-шрифт:
   div .font_family("codicon") .text_size(size) .child(глиф)
   глиф: codicon_map::codicon_by_name(icon) (пикер/табы/рейлы)
         либо локальный codicon_glyph() (activity_bar: extensions,
         claudeBridgePlan/Todos/Agents + фолбэк \u{ea7b} file)
```
- Ветки `<img src=URL/data:>` НЕТ — image-иконки VSIX не поддержаны.

## Метрики (из кода, точные)
- Размер по вызывающему: бар/рейлы **18×18** (svg; codicon 18px в баре, **16px** в рейле), пикер **15×15** / 15px, стрип-табы **13×13** / 13px.
- Цвет: задаётся вызывающим (`text_muted` #838aa0 / active `text_primary` #cfd4e2 / пикер `text_secondary` #adb3c7); svg красится `.text_color()` = аналог `fill="currentColor"`, но значение фиксируется при построении (hover не перекрашивает — дефект).
- `icon::codicon()` — глиф в flex-боксе **16×16**, размер шрифта параметром.

## Отличия от original.md той же папки
1. **Нет img-ветки** (`isImageIcon`: data:/https?:/file:/`/`) — VSIX-расширения с растровыми/URL-иконками получат codicon-фолбэк `\u{ea7b}`/`\u{eb51}` вместо своей иконки.
2. Phosphor не как инлайн `<path viewBox 256>` из `tool-icon-paths.ts`, а vendored svg-ассеты `icons/*.svg` (assets.rs) — визуально те же глифы.
3. Набор токенов шире: + `tree-view` (в оригинальном TOOL_ICON_PATHS его нет — tree там contributed?); фолбэк-глифы неизвестных различаются между копиями (`\u{ea7b}` в баре vs `\u{eb51}` в табах/рейлах).
4. Рейл-плитка: codicon 16px в боксе 32 (оригинал 18px).
5. Дублирование логики в 4 местах вместо одного `<ToolIcon>` — размеры/фолбэки уже разъехались (18/16/15/13).
6. Hover/active-перекраска svg не работает (цвет вычислен заранее), codicon-ветка в `tile()` перекрашивается — поведение веток различается, у оригинала обе наследуют currentColor.

## Дополнение атрибутов (цикл 10)

- отступы: N/A: отступы — иконка рисуется как svg/глиф без собственных паддингов; отступы задают контейнеры (плитка 32×32, таб px 10)
- гэпы: N/A: гэпы — у иконки нет детей; расстояние до лейбла даёт `gap 6` строки таба (`crates/shell/src/ui/slot_panel.rs`)
- шрифты: кегль codicon-ветки различается по месту вызова: рейл 18 (`activity_bar.rs`, `fn tool_glyph`), таб стрипа 13 (`slot_panel.rs`), пикер 16 (`tool_picker.rs`), правый рейл 18 (`right_column.rs`); font-family везде «codicon», weight не задаётся
