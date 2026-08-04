# 113 monaco-editor-host — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:4316-4354 (создание `InputState::code_editor` + LSP + EOL + зеркало минимапы), root.rs:5267-5348 (рендер рамки, breadcrumb, редактора, минимапы, скроллбара), root.rs:2915 (`editor_lang`); crates\shell\src\ui\editor_minimap.rs:22-36, 86-276 (минимапа), 281-... (скроллбар); crates\shell\src\ui\sticky_scroll.rs:17-19; crates\metrics\src\lib.rs:47; редактор — vendored gpui-component Input: vendor\gpui-component\src\input\ (input.rs:256 `LINE_HEIGHT`, element.rs — рендер, mode.rs — tree-sitter highlighter)

## Структура/содержание
Monaco НЕТ. Вместо него — gpui-component `Input` в режиме `code_editor(language)` (vendored, с нашими патчами).
```
div.flex_col.flex_1.mx(4).mt(4).mb(4).rounded(12).overflow_hidden.bg(editor_bg)
├─ breadcrumb: div.h(24).px(12).text_size(11).font_family("JetBrains Mono").text_muted
│                └─ путь с ~-сокращением домашней папки, ellipsis + nowrap
└─ div.flex.flex_1
   ├─ div.relative.flex_1.min_w(0).h_full.font_family("JetBrains Mono").text_size(13)
   │  ├─ Input::new(input).h_full().appearance(false).hide_scrollbar()
   │  └─ children(sticky) — наш sticky-scroll оверлей
   ├─ editor_minimap::minimap(input, minimap_input, p)   — 67px
   └─ editor_minimap::scrollbar(input, p)                — 12px
```
- Язык по расширению (`editor_lang`, root.rs:2915), подсветка tree-sitter, номера строк встроены в `code_editor`.
- LSP: `HostLsp::new(path, lang)` → `st.lsp.hover_provider` / `definition_provider` (root.rs:4321-4329).
- EOL детект при открытии: `text.contains("\r\n")` → CRLF (root.rs:4318), уходит в статус-бар (№119).
- Минимапа — ЗЕРКАЛЬНЫЙ `InputState` (`.line_number(false).minimap()`, root.rs:4341-4354) поверх канваса с thumb; порядок колонок как в Zed: текст → минимапа → скроллбар.
- LRU-лимит табов: `MAX_EDITOR_TABS`, вытесняется самый давний чистый (root.rs:4372-4386).

## Метрики (из кода, точные)
- отступы: рамка редактора mx 4 (SPACE_1) / mt 4 / mb 4 (SPACE_1); breadcrumb h 24 + px 12 (SPACE_3); у самого `Input` padding нет (flex_1 + h_full); минимапа и скроллбар — соседние колонки без отступов
- гэпы: N/A: гэпы — три колонки (текст / минимапа 67 / скроллбар 12) идут подряд, `gap` не задан
- цвета: фон рамки p.editor_bg #1d1c25; breadcrumb p.text_muted #838aa0; thumb минимапы p.text_primary #cfd4e2 α 0.08 + левая полоска 1px p.text_primary α 0.16; скроллбар — трек-бордер p.text_primary α 0.06, thumb p.bg_overlay #515567 α 0.55
- скругления: рамка редактора 12 (RADIUS_MD); у `Input`, минимапы, скроллбара и thumb скруглений нет
- шрифты: обёртка редактора — семейство «JetBrains Mono» + `text_size(px(m::EDITOR_FONT_SIZE))` = 13 (root.rs:5323-5326, ПРИМЕНЯЕТСЯ); высота строки 20 (`LINE_HEIGHT = Rems(1.25)` при rem 16, input.rs:256; `sticky_scroll::EDITOR_LINE_H = 20.0`); breadcrumb «JetBrains Mono» 11 (FS_XS); минимапа text_size 2.0 (`MM_FONT`) с line-height 3.08 (`MM_LINE_H = MM_FONT × ED_LINE_H / ED_FONT = 2 × 20/13`)
- ховер: N/A: ховер — ни у обёртки редактора, ни у минимапы, ни у скроллбара нет `.hover(...)`; реакции только на wheel / mouse-down / drag
- прочие константы: `MM_WIDTH` 67, `MIN_THUMB` 25, скроллбар `SB_W` 12, `MAX_STICKY` 5

## Отличия от original.md той же папки
1. Полная замена движка: gpui-component `Input` (rope + tree-sitter) вместо Monaco — опции `automaticLayout`, `smoothScrolling`, `fixedOverflowWidgets`, `largeFileOptimizations`, `maxTokenizationLineLength`, `scrollBeyondLastLine` неприменимы.
2. `.error` («Failed to open: {error}», accent-red, font-mono fs-sm, padding space-5) НЕ РЕАЛИЗОВАН — сбой чтения файла отдельного вью не даёт.
3. `fontSize: 13` совпадает: `m::EDITOR_FONT_SIZE` = 13 применяется к обёртке редактора (root.rs:5326).
4. `fontFamily` у оригинала берётся из `getComputedStyle --font-mono`; у нас семейство «JetBrains Mono» проставлено строкой в двух местах (root.rs:5302, 5323) — фоллбеков нет.
5. stickyScroll: у оригинала семантический Monaco; у нас — свой indentation-оверлей, максимум 5 строк (`MAX_STICKY`).
6. minimap: у оригинала Monaco-минимапа; у нас — зеркальный `Input` шириной 67px + канвас-thumb с клик-центрированием и драг-слежением (порт Zed).
7. Скроллбар: свой, 12px, трек с бордером слева, thumb min 25px — вместо Monaco `verticalScrollbarSize: 8` / `horizontalScrollbarSize: 8` со slider `border-radius: var(--radius-xs)`; горизонтального скроллбара нет (`soft_wrap(false)`, прокрутка по X — колесом).
8. Hover/suggest-виджеты — LSP-поповеры vendored input (input/popovers), а не Monaco-виджеты в body-слое.
9. Добавлен breadcrumb-ряд внутри рамки редактора (h 24, mono 11, text-muted) — в оригинальном `MonacoEditor` его нет.
10. `line-height` в original.md не указан (задаётся Monaco по fontSize) — сравнить нечем; у нас жёстко 20px.
