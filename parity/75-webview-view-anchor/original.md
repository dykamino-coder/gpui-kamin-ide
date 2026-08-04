# 75 webview-view-anchor — оригинал
Файлы: `src/renderer/components/activity-bodies/ContributedContainerBody.tsx` (79-143 — `WebviewViewBody`, JSX 136-142), `src/renderer/components/activity-bodies/ContributedContainerBody.module.css`

## JSX-структура (кратко, вложенность)
```
div [data-webview-anchor=viewId] .frame | .frameFlush (flush ? frameFlush : frame)
└─ пока !hasHtml:
   ├─ exhausted → <WebviewLoadError onRetry />          (элемент 71)
   └─ иначе   → <WebviewLoadingSkeleton attempts />     (элемент 70)
```
Iframe здесь НЕ рендерится — только якорь; `PersistentWebviewLayer` копирует rect + border-radius якоря и позиционирует над ним живой iframe.

Логика resolve-retry: `RESOLVE_MAX_ATTEMPTS=45`, base 350ms, backoff ×1.5, max 3000ms (~2 мин); рестарт по `kamin:exthost:respawned`; `retryNonce` — ручной Retry.

## Метрики (ИЗ CSS, точные значения)
- `.frame`: `flex:1; min-height:0`; margin `0 var(--space-2) var(--space-2)` (top 0, стороны/низ space-2); `overflow:hidden`; `border-radius:var(--radius-lg)`; `position:relative` (якорь для absolute-скелета). Карточка со скруглениями БЕЗ glint-бордера
- `.frameFlush`: `flex:1; min-height:0; overflow:hidden; position:relative` — без inset и radius (view уже внутри host-карточки, Customize)
- Цвета не задаются — вебвью красит свою поверхность сам, radius клипует
- hover/active/focus — нет; transition — нет

## Состояния (классы-варианты с метриками)
- `.frame` — карточный вариант (по умолчанию, `flush=false`)
- `.frameFlush` — flush-вариант (Customize-страницы, `flush=true`)
- Пока html не пришёл: внутри скелет (attempts показывается) или Retry-карточка после исчерпания бюджета

## Дополнение атрибутов (цикл 10)

- цвета: якорь прозрачен — `.frame`/`.frameFlush` фона не задают (`ContributedContainerBody.module.css:52-71`), под ним видна карта `--bg-mantle` #262533 / #fbf7f4, а сам вебвью красит свою поверхность `--editor-bg` #1d1c25 dark (`dark-theme.css:21`) / #fcfaf6 light (`light-theme.css:32`); комментарий CSS прямо фиксирует «webview paints its own surface».
