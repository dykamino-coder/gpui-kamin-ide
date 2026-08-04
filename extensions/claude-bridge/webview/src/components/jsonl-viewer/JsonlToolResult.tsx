import type { JSX } from 'preact'
import { useState } from 'preact/hooks'

function formatTokens(n: number): string {
  if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M'
  if (n >= 1000) return (n / 1000).toFixed(1) + 'K'
  return String(n)
}

interface JsonlToolResultProps {
  toolUseId?: string
  content: string
  isError?: boolean
  /** Original char count when `content` was truncated in-memory (heavy-tool-
   *  result offload). Falsy = content is whole; label falls back to its length. */
  fullLen?: number
}

export function JsonlToolResult({ content, isError, fullLen }: JsonlToolResultProps): JSX.Element {
  const [expanded, setExpanded] = useState(false)
  const truncated = content.length > 2000 ? content.slice(0, 2000) + '\n... (truncated)' : content

  return (
    <div class={`block-tool-result ${expanded ? 'expanded' : ''}`}>
      <div class="result-header" onClick={() => setExpanded(!expanded)}>
        <span class="result-arrow">
          <i class={isError ? 'fas fa-circle-xmark' : 'fas fa-chevron-right'} style={isError ? 'color:var(--accent-red)' : undefined} />
        </span>
        {' '}Result{' '}
        <span style={`color:${isError ? 'var(--accent-red)' : 'var(--text-disabled)'};font-size:10px`}>
          {formatTokens(fullLen ?? content.length)} chars
        </span>
      </div>
      <div class="result-body" style={isError ? 'color:var(--accent-red)' : undefined}>
        {truncated}
      </div>
    </div>
  )
}
