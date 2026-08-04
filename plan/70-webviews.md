# 70 — Вебвью-подсистема (gpui-wry / WebView2)

Источники: `exthost/api/webview*.ts`, `src-tauri/src/webview.rs`, `WebviewPanelView.tsx`, `PersistentWebviewLayer.tsx`, `signals/webviews*.ts`.

Решение: VSIX-вебвью (чат/консоль/план/кастомайз) рендерятся **WebView2 через gpui-wry** — у пользователя фризит только ГЛАВНОЕ окно WebView2, панельные вебвью не фризят. Главное окно — GPUI (нативное), вебвью — оверлеи поверх него (модель как Tauri multiwebview, паттерны уже в проде).

## Хост-сторона (Node, без изменений)
- Webviews-класс: createPanel (WebviewPanel) + resolveView (WebviewView, ленивый resolve на первый показ)
- Панель: html-сеттер → kamin:webview:html; postMessage → WebviewPostQueue (коалессация, кап по кадру, порядок vs других broadcast, purge-on-close) → kamin:webview:post; onDidReceiveMessage ← kamin:webview:inbound; retainContextWhenHidden: скрытая non-retained панель ОТКАЗЫВАЕТ postMessage (false), retained — принимает; WebviewView скрытый — всегда отказывает; re-resolve на перезагрузке ре-шлёт кэшированный HTML (не пустой вью)
- Персист между рестартами: serializer + restore() на первом коннекте + persistPanelState (webview-panels.json)
- asWebviewUri/cspSource: origin kaminwebview.localhost; /__resource/<id>/<path>; roots по умолчанию = папка расширения + workspace folders

## Что обязан реализовать GPUI-шелл

### 1. Контент-протокол (замена webview.rs)
Пер-вебвью документ со СВОИМ origin + permissive CSP (inline-скрипты должны работать) и /__resource/<id>/<path> (canonicalize + root-confinement + mime + percent-decode, 403/404). **АСИНХРОННО/не на UI-треде** (синхронный хэндлер клинил WebView2 — задокументированный фриз). Варианты: wry custom protocol (если async ок) ИЛИ loopback-HTTP как в Servo-исследовании (проверено: host уже умеет /__webview/<id>?token= — вариант Б переиспользует его). Команда webview_set_html(id, html, roots).

### 2. acquireVsCodeApi-шим + postMessage-релей
Инъекция шима в начало каждого дока: postMessage→наружу, getState/setState (сид window.__kaminInitialState, {__kaminState} для персиста, стор переживает анмаунт). Релей: вебвью → шелл → kamin:webview:inbound → host; host → kamin:webview:post {batch} → шелл → вебвью. В wry: IPC-хэндлер wry ↔ evaluate_script (вместо parent.postMessage iframe'а).

### 3. Позиционирование-оверлеи (замена PersistentWebviewLayer)
- Каждый вебвью создаётся ОДИН раз на жизнь приложения; позиционируется по ректу плейсхолдера панели; скрытый = спарковать (офскрин/hide), НЕ уничтожать (буфер/состояние живут)
- Синк ректа при сплиттер-драгах/resize окна (per-frame) + burst на смену сессии/layout
- Во время драга сплиттера вебвью «заморожен» (не ест pointer-events)
- Чат получает switch-cover шиммер при переключении сессии

### 4. Правило штор (поповеры над вебвью)
Wry-оверлей рисуется ПОВЕРХ GPUI-контента → любые меню/диалоги/тултипы, пересекающие вебвью, требуют: скрыть/зашторить вебвью (как BrowserPane прячется при DOM-поповере) ИЛИ рисовать поповер отдельным GPUI-окном поверх. Тултипы из вебвью уже решены: __kaminTooltip → рисует ХОСТ-слой (портируется 1:1).

### 5. Вотчдог
Ping/pong (__kaminPong/__kaminBusy), reload по последовательным промахам; подписка на WebView2 ProcessFailed (wry/webview2-com) → классификация → kamin:webview:process-failed → in-place recovery. Спиннер загрузки + Retry-ошибка (WebviewLoadingSkeleton/WebviewLoadError).

### 6. Тема в вебвью
Live-push палитры (webview-theme): при смене темы шелл шлёт актуальные --vscode-*/токены в каждый вебвью (evaluate_script). Contributed-темы → raw --vscode-* семья (см. 20-theme §8).

### 7. Embedded browser (Files/Web-панель)
Отдельный wry-вебвью: set_bounds (создание на первый вызов, physical px), hide (парковка -32000), navigate (URL/host/DuckDuckGo-нормализация), back/forward/reload (history.*/location.reload), событие navigated. DOM нав-бар рисует GPUI.

## Данные/сигналы (перенос в state)
webviewPanels (create/html/title/reveal/dispose), webviewViewHtml + roots, webview-tooltip, вкладки webview://<id> в FileViewerTabs (иконка расширения-владельца), закрытие вкладки → уведомить host (closed/onDidDispose)

## Чеклист паритета (вебвью)
- [ ] Контент-протокол async + root-confinement + resource-сервер
- [ ] Шим + релей + getState/setState персист
- [ ] Один-раз-смонтированные оверлеи + rect-синк + парковка
- [ ] Правило штор для всех поповеров/меню/диалогов
- [ ] Вотчдог + ProcessFailed + Retry UI
- [ ] Live-тема в вебвью
- [ ] Embedded browser полный
- [ ] retainContextWhenHidden-семантика + restore() + re-resolve
