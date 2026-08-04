import type { JSX } from 'preact'

interface DropdownOptionProps {
  icon: string
  iconColor?: string
  name: string
  description: string
  selected: boolean
  onClick: () => void
}

export function DropdownOption({ icon, iconColor, name, description, selected, onClick }: DropdownOptionProps): JSX.Element {
  return (
    <div class="perm-option" onClick={onClick}>
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
