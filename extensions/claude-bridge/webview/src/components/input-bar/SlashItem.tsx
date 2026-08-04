import type { JSX } from 'preact'

interface SlashItemProps {
  name: string
  description: string
  isSelected: boolean
  onClick: () => void
  onMouseEnter: () => void
}

export function SlashItem({ name, description, isSelected, onClick, onMouseEnter }: SlashItemProps): JSX.Element {
  return (
    <div
      class={`slash-item${isSelected ? ' selected' : ''}`}
      onClick={onClick}
      onMouseEnter={onMouseEnter}
    >
      <span class="slash-item-cmd">{name}</span>
      <span class="slash-item-desc">{description}</span>
    </div>
  )
}
