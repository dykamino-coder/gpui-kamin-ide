import type { JSX } from 'preact'
import { AGENT_PALETTE, PALETTE_TOKENS, getAgentColor, resolvePinnedColor } from '../../utils/agent-color'
import { activeTabId, tabs } from '../../signals/tabs'
import { pinnedTitleColors, setPinnedTitleColor } from '../../signals/ui'
import { useState } from 'preact/hooks'

interface TabTitleProps {
  title: string
  folderName?: string
}

const BASE_STYLE = 'font-size: var(--fs-md);font-weight:600;margin:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;user-select:none;min-width:0'

export function TabTitle({ title, folderName }: TabTitleProps): JSX.Element {
  const tab = tabs.value.find(t => t.id === activeTabId.value)
  const sessionTitle = tab?.sessionTitle
  const pinnedKey = sessionTitle ?? null
  const pinnedRaw = pinnedKey ? pinnedTitleColors.value[pinnedKey] : null
  const pinned = resolvePinnedColor(pinnedRaw) ?? null
  const color = pinned ?? getAgentColor(sessionTitle)
  const [open, setOpen] = useState(false)

  // Folder name is rendered as a separate breadcrumb chip in ChatHeader,
  // so the title shows only the session topic (or `title` / placeholder
  // when there's no topic yet — shown muted).
  const headingText = sessionTitle || title || folderName || ''
  const isPlaceholder = !sessionTitle
  const titleColor = isPlaceholder ? 'var(--text-muted)' : 'var(--text-primary)'
  const heading = (
    <h1
      style={`${BASE_STYLE};cursor:${sessionTitle ? 'pointer' : 'default'};color:${titleColor}`}
      onClick={() => sessionTitle && setOpen(v => !v)}
      data-tooltip={sessionTitle ? (pinned ? 'Click to change pinned color' : 'Click to pin a color') : undefined}
    >
      {headingText}
    </h1>
  )

  if (!sessionTitle) return heading

  function pickColor(c: string | null) {
    if (!pinnedKey) return
    setPinnedTitleColor(pinnedKey, c)
    setOpen(false)
  }

  return (
    <div style="display:flex;align-items:center;min-width:0;position:relative">
      {heading}
      {open && (
        <div
          style="position:absolute;top:100%;left:0;margin-top:4px;display:flex;flex-wrap:wrap;gap:4px;padding:6px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);box-shadow:var(--shadow-card);z-index:var(--z-dropdown)"
          onMouseLeave={() => setOpen(false)}
        >
          {PALETTE_TOKENS.map((tok, i) => {
            const swatch = AGENT_PALETTE[i] // resolved hex for the swatch fill
            const isActive = pinnedRaw === tok
            return (
              <button
                key={tok}
                type="button"
                onClick={() => pickColor(tok)}
                data-tooltip={tok.replace('--accent-', '').replace('--', '')}
                style={`width:18px;height:18px;border-radius:50%;border:2px solid ${isActive ? 'var(--text-primary)' : 'transparent'};background:${swatch};cursor:pointer;padding:0`}
              />
            )
          })}
          <button
            type="button"
            onClick={() => pickColor(null)}
            data-tooltip="Clear (use auto color)"
            style="width:18px;height:18px;border-radius:50%;border:1px dashed var(--text-muted);background:transparent;color:var(--text-muted);cursor:pointer;padding:0;font-size:9px;display:flex;align-items:center;justify-content:center"
          >
            <i class="fas fa-xmark" />
          </button>
        </div>
      )}
    </div>
  )
}
