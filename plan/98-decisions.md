# 98 — Решения по открытым вопросам (все закрыты до старта)

## Q1. Editor: gpui-component vs Monaco — РЕШЕНО: gpui-component editor как база
Разведка модуля input/ (github longbridge/gpui-component): есть selection/cursor, **search (find/replace)**, **lsp/** (клиент), **popovers/** (completion/hover), display_map, indent, rope_ext, tree-sitter подсветка, заявлено 200K строк стабильно + LSP diagnostics/completion/hover.
- **Есть из коробки**: базовое редактирование, подсветка (tree-sitter), LSP-мост (у нас провайдеры уже с хоста), find/replace, номера строк.
- **Гэп против Monaco (закрывается инкрементально в editor/-слое)**: multi-cursor (selection.rs есть, множественность не подтверждена), code folding UI, minimap, inlay hints рендер, peek-виджет, column select, семантические токены поверх tree-sitter.
- **Contributed TextMate-грамматики**: закрыты через host-токенизацию (план 25 §A2) — не зависят от tree-sitter.
- **Фолбэк при жёстком блокере**: форк/расширение редактора (открытый Rust-код) — НЕ возврат к Monaco (Monaco = webview = главное окно снова WebView2 = фриз). Порядок паритета: ядро+LSP (есть) → folding/multi-cursor/find-widget → minimap/peek/inlay hints.

## Q2. DevTools-кнопка титлбара — РЕШЕНО: девтулзы активного вебвью + системный лог
Нативное GPUI-окно page-devtools не имеет. Кнопка: (а) если фокус/активна вебвью-панель (чат и пр.) — открыть DevTools этого wry-вебвью (WebView2 OpenDevToolsWindow); (б) иначе — открыть SystemLogPanel (Customize→System). Тултип «DevTools». Поведение и иконка (fa-bug) сохраняются.

## Q3. Формат апдейтера — РЕШЕНО: остаёмся tauri-updater-совместимыми
GPUI-приложение реализует ТОТ ЖЕ протокол клиентом: GET {serverUrl-производный}/updates/kaminide/{target}/{arch}/{version} → JSON {version,notes,pub_date,url,signature} → скачивание + **minisign-верификация тем же pubkey** → замена + рестарт. Сервер (/updates/kaminide/*) и пайплайн подписи (.tauri/kaminide-updater.key) НЕ меняются — ноль работы на серверной стороне, обе линии (Tauri и GPUI) обслуживаются одним эндпоинтом. Инсталлер — NSIS (как сейчас), артефакт updater = подписанный setup.

## Q4. Контент-протокол вебвью — РЕШЕНО: loopback-HTTP хоста
Использовать **kamin-host HTTP `/__webview/<id>?token=`** — ⚠ НА ТЕКУЩЕМ main его НЕТ (ws-server.ts:56 отвергает всё кроме /rpc; сейчас вебвью через Tauri `kaminwebview://` протокол + src-tauri/webview.rs). Реализация ЕСТЬ на ветке cef-migration (commit 642da7d, проверена в Servo/Chrome-исследовании) — забрать оттуда + расширить `/__resource/<id>/<path>` (root-confined, mime, percent-decode) — перенос логики из webview.rs в ws-server.ts (Node). Плюсы: асинхронность из коробки (нет WebView2 UI-thread wedge), нет кастомной регистрации схемы в wry, одна реализация для любых движков. Origin-изоляция: пер-вебвью токен в URL + отдельный порт хоста; CSP инжектится хостом в HTML (как webview.rs делал). wry-вебвью просто грузит http://127.0.0.1:<hostPort>/…
- Примечание: kamin:webview:setHtml уже написан (архивная ветка cef-migration, commit 642da7d) — это движко-агностичный код «отдать HTML по loopback», НЕ возврат к CEF/Chrome (те отменены). Просто копируем готовую логику в kamin-host на main.
- Контекст веток: kamin-ide = main (WebView2/Tauri 0.2.87, прод). CEF/Chrome/IWA/Servo отменены; cef-migration — только архив. Курс: GPUI + gpui_wry. GPUI-план живёт в отдельной папке gpui-kamin-ide.

## Q5. activateStartup listFiles-гейт — РЕШЕНО: bound + не блокировать onStartupFinished
- **Полевые замеры 2026-07-24 (GPUI-шелл, воркспейс %PROJECTS%):** во время фоновой прогулки индекса ВСЕ fs-задействованные RPC хоста голодают: kamin:fs:listDir 48s+ (чистый хост с малым воркспейсом — 21ms), даже kamin:workspace:set (запись workspace.json) висит >10s. Память-only методы (sessions:*, workspace:get) мгновенны. UV_THREADPOOL_SIZE=16 смягчает до 6-9s. Прогулка перезапускается на КАЖДЫЙ старт хоста (stale-while-revalidate). → приоритет фикса поднят; walker обязан уступать интерактивным fs-запросам (очередь с приоритетом/чанк-yield), не только activation-гейт.
Фикс в kamin-host (loader.ts:332 / activation-manager): (а) listFiles для workspaceContains-проверки ограничить (глубина/количество/таймаут ~2s); (б) onStartupFinished НЕ ждать полного listFiles — файрить после установленного таймаута/первого батча. Инвариант: claude-bridge активируется всегда, даже на гигантском workspace (домашняя папка — известный репро). Фикс полезен и текущему Tauri-приложению — можно закоммитить в kamin-ide до/независимо от GPUI.

Все 5 вопросов закрыты. Блокеров старта имплементации нет.
