import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { jsonlVisible, viewerVisible, rightPanelVisible, filePanelVisible, filePanelBottomVisible, sidebarMode } from '../../signals/ui'
import { PanelTripletToggle } from './PanelTripletToggle'

/** Three view-toggle buttons (Session log / Console / Plan & files).
 *  Read/write their own signals so they can be mounted next to the
 *  chat tabs strip without prop-threading from App.tsx. */
export function HeaderViewToggles(): JSX.Element | null {
  // Mirror TabsBar's hide-in-customize behaviour — these toggles only
  // make sense when there's a chat panel to apply them to.
  if (sidebarMode.value === 'customize') return null
  const jsonlOn = jsonlVisible.value
  const terminalOn = viewerVisible.value
  // Touch all side-panel-visibility signals so this component re-renders
  // when any panel is shown/hidden via keyboard or layout hydration,
  // keeping the triplet's active state in sync without each child
  // needing its own subscription path.
  void rightPanelVisible.value
  void filePanelVisible.value
  void filePanelBottomVisible.value

  function toggleJsonl(): void {
    const next = !jsonlVisible.value
    jsonlVisible.value = next
    storage.setItem('jsonl-panel-visible', String(next))
  }
  function toggleTerminal(): void {
    const next = !viewerVisible.value
    viewerVisible.value = next
    storage.setItem('jsonl-viewer-visible', String(next))
  }

  return (
    <div style="display:inline-flex;align-items:center;gap:6px;padding:0 10px;-webkit-app-region:no-drag;margin:6px 1px">
      <div class="header-btn-group">
        <button
          class="header-btn"
          data-tooltip={jsonlOn ? 'Hide session log' : 'Show session log'}
          type="button"
          onClick={toggleJsonl}
          style={jsonlOn ? 'color:var(--text-primary)' : ''}
        >
          <i class="fas fa-list" />
        </button>
        <button
          class="header-btn"
          data-tooltip={terminalOn ? 'Hide console' : 'Show console'}
          type="button"
          onClick={toggleTerminal}
          style={terminalOn ? 'color:var(--text-primary)' : ''}
        >
          <i class="fas fa-terminal" />
        </button>
      </div>
      <PanelTripletToggle />
    </div>
  )
}
