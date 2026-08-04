import type { JSX } from 'preact'

interface TreeIconProps {
  type: string
}

export function TreeIcon({ type }: TreeIconProps): JSX.Element | null {
  switch (type) {
    case 'agent':
      return <span class="tree-icon"><i class="fas fa-robot" /></span>
    case 'team':
      return <span class="tree-icon"><i class="fas fa-users" /></span>
    case 'teammate':
      return <span class="tree-icon"><i class="fas fa-user" /></span>
    default:
      return null
  }
}
