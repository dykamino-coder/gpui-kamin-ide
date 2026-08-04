import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { escapeHtml } from './utils'

export function ThinkingBlock({ thinking }: { thinking: string }): JSX.Element | null {
  // Хук ДО раннего return (rules of hooks): пустой thinking, ставший
  // непустым в том же mounted-инстансе, раньше сдвигал порядок хуков.
  const [expanded, setExpanded] = useState(false)
  if (!thinking || !thinking.trim()) return null
  const preview = thinking.slice(0, 80).replace(/\n/g, ' ')

  return (
    <div class={`block-thinking ${expanded ? 'expanded' : ''}`}>
      <div class="thinking-toggle" onClick={() => setExpanded(!expanded)}>
        <span class="arrow">&#9654;</span>
        {' '}
        <i class="fas fa-lightbulb" style="font-size:10px;color:var(--accent-yellow)" />
        {' '}Thinking{' '}
        <span style="color:var(--text-disabled);font-size:10px;margin-left:4px">
          {escapeHtml(preview)}{(thinking || '').length > 80 ? '...' : ''}
        </span>
      </div>
      <div class="thinking-content">{thinking || ''}</div>
    </div>
  )
}
