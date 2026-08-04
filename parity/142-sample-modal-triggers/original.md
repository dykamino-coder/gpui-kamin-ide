# 142 sample-modal-triggers — оригинал
Файлы: kamin-ide/src/renderer/components/main/component-samples.tsx:194-202, design-sections.module.css:215-228 (.btnSecondary/.btnDanger); showConfirm/showPrompt — renderer/signals/overlays.ts

## Содержание/структура
`ModalTriggers()` в Block «Modals»: 3 кнопки:
1. «Confirm» (`.btnSecondary`) → `showConfirm({ title: "Sample confirm", bodyHtml: "This is a <code>ConfirmModal</code> demo." })`
2. «Confirm danger» (`.btnDanger`) → `showConfirm({ title: "Delete?", bodyHtml: "This action <strong>cannot be undone</strong>.", isDanger: true, confirmLabel: "Delete" })`
3. «Prompt» (`.btnSecondary`) → `showPrompt({ title: "Enter name", placeholder: "e.g. my-extension" })`

## Метрики
`.btnSecondary`: padding 4px 16px; radius 8px; fs 12px; transparent bg; border `1px solid var(--bg-overlay)`; hover `--bg-surface`.
`.btnDanger`: то же + background `--accent-red`; color `--bg-primary`; border none; font-weight 600; hover `--accent-maroon`.
Сами модалки — компоненты зоны Overlays; здесь только триггеры.

## Состояния/варианты
Confirm обычный / danger (isDanger + кастомный confirmLabel) / Prompt (текстовый ввод с placeholder). bodyHtml поддерживает HTML-разметку.
