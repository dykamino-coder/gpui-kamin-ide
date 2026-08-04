# 121 confirm-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/ConfirmModal.tsx` (73-98), `ConfirmModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(target===currentTarget → cancel)
└─ div.dialog role=dialog aria-modal=true aria-label={title}
   ├─ h3.title
   ├─ div.body dangerouslySetInnerHTML={sanitized bodyHtml}    (вырезаны <script>, on*=, javascript:)
   └─ div.actions
      ├─ button.cancelBtn {cancelLabel="Cancel"}
      └─ button.confirmBtn [.danger] [ref автофокус] {confirmLabel="Confirm"}
```
- Esc = cancel; backdrop-клик = cancel; автофокус Confirm (Enter принимает); восстановление фокуса на предыдущий элемент при закрытии.

## Метрики (ИЗ CSS, точные значения)
`.overlay`:
- position: fixed; inset: 0; z-index: var(--z-modal)
- background: var(--overlay-deep)
- display: flex; align-items: center; justify-content: center
- animation: fadeIn 0.12s ease-out (opacity 0→1)

`.dialog`:
- background: var(--bg-primary)
- border: 1px solid var(--bg-surface)
- border-radius: var(--radius-md)
- padding: var(--space-5)
- min-width: 320px; max-width: 480px
- box-shadow: var(--shadow-modal)

`.title`:
- margin: 0 0 var(--space-3)
- font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)

`.body`:
- margin: 0 0 var(--space-4)
- font-size: var(--fs-sm); color: var(--text-secondary); line-height: var(--lh-snug)

`.actions`: display: flex; gap: var(--space-2); justify-content: flex-end

`.cancelBtn`, `.confirmBtn` (общее):
- padding: var(--space-1) var(--space-4)
- border-radius: var(--radius-sm); font-size: var(--fs-sm); cursor: pointer
- transition: background var(--transition-fast)

`.cancelBtn`: border: 1px solid var(--bg-overlay); background: transparent; color: var(--text-primary)
`.confirmBtn`: border: none; background: var(--accent-action); color: var(--accent-action-fg); font-weight: 600

## Состояния (классы-варианты с метриками)
- `.cancelBtn:hover`: background: var(--bg-surface)
- `.confirmBtn:hover`: background: var(--accent-action-hover)
- `.confirmBtn.danger`: background: var(--accent-red)
- `.confirmBtn.danger:hover`: background: var(--accent-maroon)
