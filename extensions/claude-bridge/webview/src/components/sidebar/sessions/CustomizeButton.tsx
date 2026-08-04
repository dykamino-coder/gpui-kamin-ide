import type { JSX } from 'preact'
import { sidebarMode, activeCustomizePanel } from '../../../signals/ui'

export function CustomizeButton(): JSX.Element {
  function open(): void {
    sidebarMode.value = 'customize'
    // Auto-pick Settings unless the user already had another panel
    // selected (re-entering customize). Without this default the
    // CustomizePanel shows the landing card with "pick something"
    // copy, which is one extra click users always have to make.
    const cur = activeCustomizePanel.value
    if (!cur || cur === 'landing') activeCustomizePanel.value = 'settings'
  }
  return (
    <button class="sidebar-action" onClick={open} type="button">
      <i class="fas fa-gear" />
      Customize
    </button>
  )
}
