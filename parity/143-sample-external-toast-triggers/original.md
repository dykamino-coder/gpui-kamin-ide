# 143 sample-external-toast-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:204-237, design-sections.module.css:215-220 (.btnSecondary)

## Содержание/структура
`ExternalToastTriggers()` в Block «External toasts (out-of-app)», hint: «Standalone BrowserWindows — auto-fire when KaminIDE is unfocused. Bottom timer bar shrinks over 8 s; hover pauses both bar and dismiss timer. Buttons below force one regardless of focus.»
4 кнопки `.btnSecondary`, каждая вызывает `window.kamin?.externalToast.show({...})`:
1. «Info (timed)» → { kind: "info", title: "Build finished", message: "Sample with timer bar — hover to pause." }
2. «Success (timed)» → { kind: "success", title: "Sync complete", message: "All extensions synced — green accent + check glyph." }
3. «Warning (sticky)» → { kind: "warning", title: "Approval pending", message: "Sticky — no auto-dismiss, no timer bar.", sticky: true }
4. «Error (with actions)» → { kind: "error", title: "Activation failed", message: "Pick what to do — Retry runs activate() again, Show log opens the Output channel.", sticky: true, actions: ["Retry", "Show log"] }

## Метрики
Кнопки — `.btnSecondary` (см. 135): padding 4px 16px; radius 8px; fs 12px; border `1px solid var(--bg-overlay)`; hover `--bg-surface`.
Внешний тост — отдельное BrowserWindow (вне renderer-дерева); таймер-бар 8 s, hover ставит на паузу бар и dismiss-таймер (из hint).

## Состояния/варианты
kind: info / success / warning / error; timed (таймер-бар) vs sticky (без автозакрытия и бара); опциональные actions-кнопки.
