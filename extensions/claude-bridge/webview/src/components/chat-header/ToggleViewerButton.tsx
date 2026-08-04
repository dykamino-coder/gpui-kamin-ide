import type { JSX } from 'preact'

interface ToggleButtonProps {
  onToggleTerminal: () => void
  onToggleJsonl: () => void
  terminalVisible: boolean
  jsonlVisible: boolean
}

export function ToggleViewerButton({
  onToggleTerminal,
  onToggleJsonl,
  terminalVisible,
  jsonlVisible,
}: ToggleButtonProps): JSX.Element {
  return (
    <div class="header-btn-group">
      <button
        class="header-btn"
        data-tooltip={jsonlVisible ? 'Hide session log' : 'Show session log'}
        type="button"
        onClick={onToggleJsonl}
        style={jsonlVisible ? 'color:var(--text-primary)' : ''}
      >
        <i class="fas fa-list" />
      </button>
      <button
        class="header-btn"
        data-tooltip={terminalVisible ? 'Hide console' : 'Show console'}
        type="button"
        onClick={onToggleTerminal}
        style={terminalVisible ? 'color:var(--text-primary)' : ''}
      >
        <i class="fas fa-terminal" />
      </button>
    </div>
  )
}
