import type { JSX } from 'preact'

interface TreeToggleProps {
  hasChildren: boolean
  isCollapsed: boolean
}

export function TreeToggle({ hasChildren, isCollapsed }: TreeToggleProps): JSX.Element {
  if (!hasChildren) {
    return <span class="tree-toggle empty" />
  }
  return (
    <span class="tree-toggle">
      {isCollapsed ? '\u25B6' : '\u25BC'}
    </span>
  )
}
