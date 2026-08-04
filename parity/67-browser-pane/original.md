# 67 browser-pane — оригинал
Файлы: kamin-ide/src/renderer/components/file-panel/BrowserPane.tsx (строки 77-104), kamin-ide/src/renderer/components/file-panel/BrowserPane.module.css

## JSX-структура (кратко, вложенность)
```
div.pane
├─ div.navbar
│  ├─ button.navBtn [data-tooltip="Back"]    → i.codicon.codicon-arrow-left
│  ├─ button.navBtn [data-tooltip="Forward"] → i.codicon.codicon-arrow-right
│  ├─ button.navBtn [data-tooltip="Reload"]  → i.codicon.codicon-refresh
│  └─ form.addrForm (onSubmit → browser.navigate(draft))
│     └─ input.addr [type=text] [spellcheck=false] [placeholder="Search or enter address"]
│        value = editing ? draft : url; onFocus → select() + editing
└─ div.viewport [data-browser-viewport]  ref=viewportRef
```
Поведение: нативный child-webview позиционируется по rect вьюпорта (`browser.setBounds` × devicePixelRatio, ResizeObserver + window resize). Скрывается (`browser.hide()`) когда перекрыт поповером: MutationObserver по body, rAF-coalesce; POPUP_SELECTOR = `[role='menu'], [role='dialog'], [role='listbox'], [data-tooltip-popup]`, проверка пересечения rect'ов.

## Метрики (ИЗ CSS, точные значения)
### .pane
- display: flex; flex-direction: column; flex: 1; min-height: 0

### .navbar
- display: flex; align-items: center; gap: 4px
- padding: 4px 6px; flex-shrink: 0

### .navBtn
- display: inline-flex; align-items: center; justify-content: center
- width: 26px; height: 26px
- border: none; border-radius: var(--radius-sm)
- background: transparent; color: var(--text-secondary); cursor: pointer

### .addrForm
- flex: 1; display: flex

### .addr
- flex: 1; height: 26px; padding: 0 10px
- border: 1px solid var(--divider-soft); border-radius: var(--radius-sm)
- background: var(--bg-base); color: var(--text-primary)
- font: inherit; font-size: var(--fs-sm)

### .viewport
- flex: 1; min-height: 0
- margin: 0 6px 6px (боковой/нижний инсет 6px — рамка под нативный webview внутри скруглённой карточки)
- border-radius: var(--radius-md)
- фон не задан (прозрачен: несинхронный кадр показывает поверхность панели, не дыру)

## Состояния (классы-варианты с метриками)
- `.navBtn:hover`: background var(--bg-surface-hover); color var(--text-primary)
- `.addr:focus`: outline none; border-color var(--accent-primary)
- transition не объявлены
