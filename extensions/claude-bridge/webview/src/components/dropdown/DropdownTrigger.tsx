import type { JSX } from 'preact'

interface DropdownTriggerProps {
  icon: string
  label: string
  onClick?: () => void
  open?: boolean
}

export function DropdownTrigger({ icon, label, onClick, open = false }: DropdownTriggerProps): JSX.Element {
  return (
    <button class="perm-trigger" type="button" onClick={onClick}>
      <i class={`fas ${icon}`} />
      <span>{label}</span>
      <i class={`fas ${open ? 'fa-chevron-up' : 'fa-chevron-down'} perm-chevron`} />
    </button>
  )
}
