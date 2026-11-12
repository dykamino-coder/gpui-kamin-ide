import type { JSX } from 'preact'

interface DropdownOptionProps {
  icon: string
  iconColor?: string
  name: string
  description: string
  selected: boolean
  /** Shown but not selectable (the description says why). */
  disabled?: boolean
  onClick: () => void
}

export function DropdownOption({ icon, iconColor, name, description, selected, disabled, onClick }: DropdownOptionProps): JSX.Element {
  return (
    <div
      class={disabled ? 'perm-option perm-option-disabled' : 'perm-option'}
      aria-disabled={disabled ? 'true' : undefined}
      onClick={disabled ? undefined : onClick}
    >
      <span class="perm-option-icon" style={iconColor ? { color: iconColor } : undefined}>
        <i class={`fas ${icon}`} />
      </span>
      <div class="perm-option-info">
        <div class="perm-option-name">{name}</div>
        <div class="perm-option-desc">{description}</div>
      </div>
      <span class="perm-option-check">
        {selected && <i class="fas fa-check" />}
      </span>
    </div>
  )
}
