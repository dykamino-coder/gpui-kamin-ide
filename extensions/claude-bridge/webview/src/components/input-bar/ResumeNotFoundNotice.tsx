import type { JSX } from 'preact'

export function ResumeNotFoundNotice({ onNew, onRetry }: { onNew: () => void; onRetry: () => void }): JSX.Element {
  return <div role="alert" style={{ padding: '10px', border: '1px solid var(--vscode-inputValidation-warningBorder, #b89500)', borderRadius: '4px' }}>
    <p>This conversation was not found on the Bridge server. Your saved conversation is still linked to this tab.</p>
    <button onClick={onNew}>Start new conversation</button>{' '}
    <button onClick={onRetry}>Retry</button>
  </div>
}
