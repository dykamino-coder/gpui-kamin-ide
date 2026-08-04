import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { Dropdown } from '../../dropdown/Dropdown'
import { DropdownOption } from '../../dropdown/DropdownOption'

export interface FormSelectOption {
  value: string
  label: string
  icon?: string
  description?: string
}

interface Props {
  value: string
  options: FormSelectOption[]
  onChange: (value: string) => void
}

/** Form-style replacement for native <select> that uses the project's
 *  Dropdown / DropdownOption look (matches Effort/Model/Permissions
 *  dropdowns elsewhere in the app). */
export function FormSelect({ value, options, onChange }: Props): JSX.Element {
  const [open, setOpen] = useState(false)
  const current = options.find(o => o.value === value) ?? options[0]
  return (
    <Dropdown
      open={open}
      onToggle={() => setOpen(v => !v)}
      align="left"
      trigger={
        <button
          type="button"
          style="
            width:100%;display:flex;align-items:center;justify-content:space-between;gap:var(--space-2);
            background:var(--bg-base);border:1px solid var(--bg-surface);
            border-radius:var(--radius-sm);color:var(--text-primary);
            padding:6px 10px;font-size:var(--fs-base);font-family:inherit;cursor:pointer;
          "
        >
          <span style="display:inline-flex;align-items:center;gap:8px;flex:1;min-width:0;text-align:left">
            {current?.icon && <i class={`fas ${current.icon}`} style="color:var(--text-secondary);font-size:var(--fs-sm)" />}
            <span style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{current?.label ?? value}</span>
          </span>
          <i class={`fas ${open ? 'fa-chevron-up' : 'fa-chevron-down'}`} style="color:var(--text-muted);font-size:var(--fs-xs)" />
        </button>
      }
    >
      {options.map(opt => (
        <DropdownOption
          key={opt.value}
          icon={opt.icon ?? 'fa-circle'}
          name={opt.label}
          description={opt.description ?? ''}
          selected={opt.value === value}
          onClick={() => { onChange(opt.value); setOpen(false) }}
        />
      ))}
    </Dropdown>
  )
}
