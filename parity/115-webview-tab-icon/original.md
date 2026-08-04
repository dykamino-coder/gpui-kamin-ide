# 115 webview-tab-icon — оригинал
Файлы: `kamin-ide/src/renderer/components/file-viewer/WebviewTabIcon.tsx` (29-38); css-модуля нет — размеры даёт класс потребителя (`FileViewerTabs.module.css` `.tabIcon`: width/height 14px)

## JSX-структура (кратко, вложенность)
```
extId неизвестен | иконки нет | ещё грузится:
  i.codicon.codicon-browser {className} [aria-hidden]      (fallback)
иконка загружена (data URL):
  img {className} src={dataUrl} alt=""
```
- `extId` = `webviewPanels.find(p.id === id)?.ownerExtId`.
- Резолв через `hostRpc.extensions.icon(extId)` (тот же эндпоинт, что Extensions panel); data URL кешируется module-wide (signal `iconCache`), повторный fetch не делается; ошибка → удаление из `requested` для ретрая на следующем рендере.

## Метрики (ИЗ CSS, точные значения)
Собственных стилей нет. В контексте таба (№111) получает `.tabIcon`:
- flex-shrink: 0; width: 14px; height: 14px

## Состояния (классы-варианты с метриками)
- fallback (codicon-browser) ↔ img — переключение по факту загрузки data URL; визуальных hover/active нет.

## Дополнение атрибутов (цикл 10)

- цвета: собственных нет — `<img>` рисует data-URL иконки расширения как есть; codicon-fallback наследует `color` таба = `var(--text-secondary)` #adb3c7 (FileViewerTabs.module.css:92), на hover и у активного таба — `var(--text-primary)` #cfd4e2 (:102-109)
- отступы: N/A: отступы — у компонента нет своего CSS-модуля; в табе расстояние даёт `.tab { gap: 6px }` (FileViewerTabs.module.css:86), а `.tabIcon` задаёт только flex-shrink 0 и width/height 14px (:116-120)
