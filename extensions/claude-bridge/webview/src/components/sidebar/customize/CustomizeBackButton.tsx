import type { JSX } from 'preact'
import { sidebarMode, activeCustomizePanel } from '../../../signals/ui'

export function CustomizeBackButton(): JSX.Element {
  function handleBack(): void {
    activeCustomizePanel.value = null
    sidebarMode.value = 'sessions'
    window.dispatchEvent(new CustomEvent('customize-exit'))
  }

  return (
    <button class="sidebar-back" onClick={handleBack} type="button">
      <i class="fas fa-arrow-left" />
      Customize
    </button>
  )
}
