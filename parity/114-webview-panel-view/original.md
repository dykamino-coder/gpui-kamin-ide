# 114 webview-panel-view — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/WebviewPanelView.tsx` (рендер 369-387), `WebviewPanelView.module.css`; ретрай-карточка — `panel-placeholder/WebviewLoadingSkeleton.tsx:65-76` (`WebviewLoadError`, стили из её module.css)

## JSX-структура (кратко, вложенность)
```
div.container
├─ iframe.frame [ref] sandbox="allow-scripts allow-forms" src={kaminwebview://…} title="Extension webview" onLoad
└─ stalled && !loaded && !painted
   ? <WebviewLoadError onRetry>                          (карточка с retry)
   :  div.loader [.loaderHidden при painted] aria-hidden
      └─ div.spinner
```
- HTML сервится с `http://kaminwebview.localhost` (Windows/WebView2); НЕ srcdoc.
- Cover держится до `__kaminReady` постмессаджа, fallback `READY_FALLBACK_MS = 1200` мс.
- Load-watchdog `LOAD_WATCHDOG_MS = 20000` мс → stalled → ретрай-карточка.
- Crash-watchdog: ping каждые `CRASH_PING_INTERVAL_MS = 4000` мс, `CRASH_MISS_LIMIT = 8` подряд (~32с) → reload; `BUSY_GRACE_MS = 180000`.
- WebviewLoadError: `div.errWrap[role=alert] > i.fas.fa-triangle-exclamation.errIcon + div.errTitle "This panel didn't load" + div.errHint + button.retry (fa-rotate + "Retry")`.

## Метрики (ИЗ CSS, точные значения)
`.container`: position: relative; width: 100%; height: 100%

`.frame`:
- width: 100%; height: 100%; border: none; display: block
- background: transparent

`.loader`:
- position: absolute; inset: 0; z-index: 2
- display: flex; align-items: center; justify-content: center
- background: var(--bg-surface, var(--editor-bg, #22222e))
- opacity: 1; transition: opacity 180ms ease
- pointer-events: none

`.loaderHidden`: opacity: 0

`.spinner`:
- width: 22px; height: 22px; border-radius: 50%
- border: 2.5px solid color-mix(in srgb, var(--text-primary, #cdd6f4) 16%, transparent)
- border-top-color: var(--accent-action, var(--accent-primary, #d77757))
- animation: kaminWvSpin 0.7s linear infinite (`to { transform: rotate(360deg) }`)

## Состояния (классы-варианты с метриками)
- loading: `.loader` opacity 1 поверх iframe (z-index 2).
- painted: `.loader.loaderHidden` opacity 0 (fade 180ms).
- stalled (watchdog 20s, не loaded/painted): вместо loader — `WebviewLoadError` ретрай-карточка.
- hidden retained-панель: управляется родителем (№108, display: none) — компонент остаётся смонтирован.

## Дополнение атрибутов (цикл 10)

- отступы: `.container`/`.frame` padding и margin не задают (WebviewPanelView.module.css:1-16), работает глобальный сброс `* { margin: 0; padding: 0 }` (global.css:12); `.loader` — inset 0, padding нет (WebviewPanelView.module.css:24-25); ретрай-карточка `.errWrap` — inset 0 + padding 24px (WebviewLoadingSkeleton.module.css:99,105), `.errIcon` margin-bottom 4px (:114), `.retry` padding 6px 16px (:134)
