import type { JSX } from 'preact'

interface RegenerateTitleButtonProps {
  onClick: () => void
}

// Asks the CLI to regenerate the session's AI title from the chat (old-Bridge
// "Auto-rename"). The extension clears the user's name-sticky flag first, then
// injects `/rename`; the fresh title lands on the native KaminIDE session row.
export function RegenerateTitleButton({ onClick }: RegenerateTitleButtonProps): JSX.Element {
  return (
    <button class="header-btn" data-tooltip="Auto-rename from chat" type="button" onClick={onClick}>
      <i class="fas fa-wand-magic-sparkles" />
    </button>
  )
}
