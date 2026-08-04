# 141 sample-toast-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:182-192, design-sections.module.css:215-220 (.btnSecondary); pushToast — renderer/signals/state.ts

## Содержание/структура
`ToastTriggers()` в Block «In-app toasts»: 5 кнопок `.btnSecondary`, каждая вызывает `pushToast({...})`:
1. «Push info» → { severity: "info", message: "Sample info toast.", timestamp: Date.now() }
2. «Push success» → { severity: "success", message: "Sample success toast." }
3. «Push warning» → { severity: "warning", message: "Sample warning." }
4. «Push error» → { severity: "error", message: "Sample error." }
5. «With actions» → { severity: "info", message: "Pick an action.", actions: ["Save", "Discard"], sticky: true }

## Метрики
Кнопки — `.btnSecondary`: padding 4px 16px; border-radius 8px; font-size 12px; background transparent; color `--text-primary`; border `1px solid var(--bg-overlay)`; hover background `--bg-surface`; transition 150ms ease.
Сам тост — отдельный компонент (зона Overlays), здесь только триггеры.

## Состояния/варианты
4 severity (info/success/warning/error) + вариант с actions и sticky: true (не автоскрывается).
