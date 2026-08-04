# 122 prompt-modal — оригинал
Файлы: `kamin-ide/src/renderer/components/overlays/PromptModal.tsx` (71-102), `PromptModal.module.css`

## JSX-структура (кратко, вложенность)
```
div.overlay role=presentation onClick(target===currentTarget → cancel)
└─ div.dialog role=dialog aria-modal=true aria-label={title}
   ├─ h3.title
   ├─ input.input [.invalid] type=text [ref фокус+select] placeholder value
   │    Enter → submit (блокируется при error)
   ├─ (error) div.error {текст валидации}
   └─ div.actions
      ├─ button.cancelBtn "Cancel"
      └─ button.confirmBtn "OK" disabled={!!error}
```
- `validate` бежит на каждый ввод; строка → invalid + inline-ошибка + disabled OK. Esc = cancel. Reset к defaultValue при каждом открытии; восстановление фокуса при закрытии.

## Метрики (ИЗ CSS, точные значения)
`.overlay`: position: fixed; inset: 0; z-index: var(--z-modal); background: var(--overlay-deep); flex центр; animation: fadeIn 0.12s ease-out

`.dialog`:
- background: var(--bg-primary); border: 1px solid var(--bg-surface)
- border-radius: var(--radius-md); padding: var(--space-5)
- min-width: 360px; max-width: 520px
- box-shadow: var(--shadow-modal)

`.title`: margin: 0 0 var(--space-3); font-size: var(--fs-md); font-weight: 600; color: var(--text-primary)

`.input`:
- width: 100%; padding: var(--space-2) var(--space-3)
- border: 1px solid var(--bg-surface); border-radius: var(--radius-sm)
- background: var(--bg-base); color: var(--text-primary)
- font-size: var(--fs-md); font-family: inherit; outline: none
- transition: border-color var(--transition-fast)

`.error`: margin-top: var(--space-2); font-size: var(--fs-xs); color: var(--accent-red)

`.actions`: display: flex; gap: var(--space-2); justify-content: flex-end; margin-top: var(--space-4)

`.cancelBtn`, `.confirmBtn`: padding: var(--space-1) var(--space-4); border-radius: var(--radius-sm); font-size: var(--fs-sm); cursor: pointer; transition: background var(--transition-fast)
`.cancelBtn`: border: 1px solid var(--bg-overlay); background: transparent; color: var(--text-primary)
`.confirmBtn`: border: none; background: var(--accent-action); color: var(--accent-action-fg); font-weight: 600

## Состояния (классы-варианты с метриками)
- `.input:focus`: border-color: var(--accent-primary)
- `.input.invalid`: border-color: var(--accent-red)
- `.cancelBtn:hover`: background: var(--bg-surface)
- `.confirmBtn:hover:not(:disabled)`: background: var(--accent-action-hover)
- `.confirmBtn:disabled`: opacity: 0.5; cursor: not-allowed
