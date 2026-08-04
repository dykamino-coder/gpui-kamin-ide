import type { JSX } from 'preact'
import { useState } from 'preact/hooks'

interface CollapsibleBlockProps {
  label: string
  content: string
  color: string
}

export function CollapsibleBlock({ label, content, color }: CollapsibleBlockProps): JSX.Element {
  const [open, setOpen] = useState(false)
  const truncated = content.slice(0, 1000) + (content.length > 1000 ? '\n... (truncated)' : '')
  return (
    <div style={`margin-top:4px;border-left:2px solid ${color}33;padding-left:8px;`}>
      <div
        style={`font-size:11px;color:${color};cursor:pointer;opacity:0.7;`}
        onClick={() => setOpen(!open)}
      >
        {open ? '\u25BC' : '\u25B6'} {label}
      </div>
      {open && (
        <div style="font-size:11px;color:var(--text-disabled);white-space:pre-wrap;max-height:150px;overflow-y:auto;margin-top:4px;">
          {truncated}
        </div>
      )}
    </div>
  )
}
