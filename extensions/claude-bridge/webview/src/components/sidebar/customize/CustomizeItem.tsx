import type { JSX } from 'preact'

interface CustomizeItemProps {
  id: string
  icon: string
  label: string
  panel: string
  isActive: boolean
  onClick: () => void
}

export function CustomizeItem({ id, icon, label, isActive, onClick }: CustomizeItemProps): JSX.Element {
  return (
    <button
      id={id}
      class={`sidebar-customize-item${isActive ? ' active' : ''}`}
      onClick={onClick}
      type="button"
    >
      <i class={icon} />
      {label}
    </button>
  )
}
