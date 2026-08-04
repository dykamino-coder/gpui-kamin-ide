# 114 webview-panel-view — наша реализация
Файлы: %PROJECTS%\gpui-kamin-ide\crates\shell\src\root.rs:2762-2824 (webview_panel/webview_body), 2844-2911 (webview_body_dyn), 3703-3779 (показ/скрытие живых вебвью), crates\shell\src\ui\chat_webview.rs (build_webview/stage_html/has_html)

## Структура (gpui-дерево кратко)
Вебвью у нас — ПАНЕЛЬНЫЕ (слоты layout), не редакторские табы. Два хостинга:
```
1) visual hosting (wv_visual, Windows): composition-визуал WebView2 + канвас-зона
   → visual_wv_body(id) (дыры в фонах gpui, скругление угловыми масками)
2) обычный: Entity<gpui_component::webview::WebView> (WebView2-чайлд-окно)
div (match):
├─ has_html && alive → div#id.relative.size_full [probe_area] + wv
├─ has_html && !alive → центр: codicon "\u{eb19}" 22px accent_primary + «Loading…»
└─ иначе → panel_placeholder («Open new tool or drag-n-drop tool from other panels»)
```
`webview_body_dyn` (contributed-тулы): + px 8 / pb 8 воздух, подложка rounded 12 bg editor_bg под вебвью.
HTML: `stage_html` пишет в cache-файл (`webview-html/{id}.html`) и грузит file-URL; `alive` — по `WebviewAlive`-пингу из вебвью.

## Метрики (из кода, точные)
- Loading-состояние: gap 8, глиф 22px p.accent_primary #89b4fa, текст fs 13 (FS_MD) p.text_secondary #adb3c7
- Дин-вебвью: px 8, pb 8, подложка rounded 12 (RADIUS_MD) bg p.editor_bg #1d1c25
- Карта — glint-рамка (glint_surface_wv_holed)

## Отличия от original.md той же папки
1. Не iframe: WebView2 (чайлд-окно или composition-визуал) — sandbox="allow-scripts allow-forms" и `kaminwebview://`/`http://kaminwebview.localhost` схемы отсутствуют; HTML грузится из staged-файла.
2. Fade-cover со спиннером (opacity-transition 180ms, spinner 22px 0.7s) не реализован — вместо него статичное «Loading…» с глифом (без анимации вращения).
3. Load-watchdog 20s, crash-ping 4s×8, BUSY_GRACE и ретрай-карточка WebviewLoadError («This panel didn't load» + Retry) — НЕ РЕАЛИЗОВАНЫ.
4. `__kaminReady`/READY_FALLBACK_MS-логики нет; готовность = `WebviewAlive`-пинг.
5. Retained-панели: вебвью живут в HashMap постоянно; скрытие — прятание чайлд-окон/визуалов (root.rs:3703-3760), а не display:none у слоя.
6. Скругление и «дыра» в фонах — angular-маски glint-канваса (visual hosting), у оригинала — обычный CSS.
