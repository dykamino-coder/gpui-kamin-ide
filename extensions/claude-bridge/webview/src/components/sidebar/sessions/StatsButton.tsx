import type { JSX } from 'preact'
import { sidebarMode, activeCustomizePanel } from '../../../signals/ui'

export function StatsButton(): JSX.Element {
  function open(): void {
    sidebarMode.value = 'customize'
    activeCustomizePanel.value = 'stats'
  }
  return (
    <button class="sidebar-action" onClick={open} type="button">
      <i class="fas fa-chart-column" />
      Stats
    </button>
  )
}
