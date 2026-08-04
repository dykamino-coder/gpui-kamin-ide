import type { JSX } from 'preact'
import { useEffect, useLayoutEffect, useRef, useState } from 'preact/hooks'
import { displaySegments, activeSegmentTsByTab, type DisplaySegment } from '../../signals/compact-segments'
import { archivedViewTs } from '../../signals/jsonl'
import { activeTabId } from '../../signals/tabs'
import { replayProgressByTab } from '../../signals/replay-progress'
import { useBridge } from '../../hooks/useBridge'
import { openDisplaySegment } from '../../lib/open-segment'

/** Tab strip for compact-boundary-delimited conversation segments.
 *  Mounted between ChatHeader and the scrollable chat panel so it stays
 *  in view regardless of how far the user has scrolled inside JsonlViewer.
 *  Hidden entirely when the session has no compactions yet. Tabs keep their
 *  natural width — they never squeeze down to an unreadable stub; once they
 *  stop fitting, a ▾ button on the right opens a menu listing every
 *  conversation by date (same behaviour as the file-viewer tab strip). */
export function ConversationSegmentTabs(): JSX.Element | null {
  const bridge = useBridge()
  const segs = displaySegments.value
  const tabId = activeTabId.value
  // Active pill: an archived view wins; otherwise the resident ts pin; otherwise
  // follow-latest (the Current segment). All three are ts-keyed so the highlight
  // survives history arriving/leaving behind it.
  const archivedTs = tabId ? archivedViewTs(tabId) : undefined
  const pinnedTs = tabId ? activeSegmentTsByTab.value.get(tabId) : undefined
  const isActive = (seg: DisplaySegment): boolean => {
    if (archivedTs !== undefined) return (seg.ts ?? '') === archivedTs
    if (pinnedTs !== undefined) return seg.ts === pinnedTs
    return seg.isLatest
  }

  const stripRef = useRef<HTMLDivElement | null>(null)
  const overflowRef = useRef<HTMLDivElement | null>(null)
  const [overflow, setOverflow] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)

  // Do the tabs still fit at full width? Measured after every render (the strip
  // reflows as segments arrive) and on resize.
  useLayoutEffect(() => {
    const el = stripRef.current
    const has = !!el && el.scrollWidth > el.clientWidth + 1
    setOverflow((prev) => (prev !== has ? has : prev))
  })
  useEffect(() => {
    const el = stripRef.current
    if (!el) return undefined
    const ro = new ResizeObserver(() => { setOverflow(el.scrollWidth > el.clientWidth + 1) })
    ro.observe(el)
    return () => { ro.disconnect() }
  }, [])
  useEffect(() => {
    if (!menuOpen) return undefined
    const onDown = (e: MouseEvent): void => { if (!overflowRef.current?.contains(e.target as Node)) setMenuOpen(false) }
    const onKey = (e: KeyboardEvent): void => { if (e.key === 'Escape') setMenuOpen(false) }
    document.addEventListener('mousedown', onDown, true)
    document.addEventListener('keydown', onKey)
    return () => { document.removeEventListener('mousedown', onDown, true); document.removeEventListener('keydown', onKey) }
  }, [menuOpen])

  // While the transcript is still replaying, the compact set + counts are in flux
  // (the mirror-resident strip shows only the in-window compacts until the
  // server's full index lands at replay-complete). Show a loader in the strip's
  // place instead of a wrong/partial set — matches the chat's load-cover.
  const replaying = tabId ? replayProgressByTab.value.get(tabId) != null : false
  if (replaying) {
    return (
      <div style="z-index:5;padding:6px 12px;display:flex;align-items:center;gap:8px;flex-shrink:0;background:var(--bg-base);font-size:11px;color:var(--text-disabled)">
        <i class="fas fa-spinner fa-spin" style="font-size:11px" />
        <span>Loading conversations…</span>
      </div>
    )
  }

  if (segs.length <= 1) return null

  /** Click a segment. Resident (in-window) → the existing index pick. Out-of-
   *  window → load it from the mirror (archived view). Current → back to live. */
  function pick(seg: DisplaySegment): void {
    openDisplaySegment(bridge, seg, segs.length)
    setMenuOpen(false)
    requestAnimationFrame(() => {
      stripRef.current?.querySelector<HTMLElement>(`[data-seg-key="${CSS.escape(seg.key)}"]`)?.scrollIntoView({ inline: 'nearest', block: 'nearest' })
    })
  }

  // Newest-first: Current segment leftmost, then older segments in descending
  // date order. Reversing only the *render* order keeps the chronological list.
  const ordered = segs.slice().reverse()

  return (
    <div style="z-index:5;padding:6px 12px;display:flex;align-items:center;gap:6px;flex-shrink:0;background:var(--bg-base)">
      <div
        ref={stripRef}
        style="display:flex;align-items:center;gap:6px;flex:1;min-width:0;overflow-x:auto;scrollbar-width:none"
        class="conv-seg-strip"
      >
        {ordered.map((seg, ri) => (
          <SegButton
            key={seg.key}
            segKey={seg.key}
            active={isActive(seg)}
            label={segLabel(seg, segs.length - 1 - ri, segs.length)}
            count={seg.count}
            onClick={() => { pick(seg) }}
          />
        ))}
      </div>
      {overflow && (
        <div ref={overflowRef} style="position:relative;flex-shrink:0">
          <button
            type="button"
            aria-label="All conversations"
            data-tooltip="All conversations"
            aria-expanded={menuOpen}
            onClick={() => { setMenuOpen((o) => !o) }}
            style="width:22px;height:22px;display:inline-flex;align-items:center;justify-content:center;border:none;background:transparent;color:var(--text-secondary);cursor:pointer;border-radius:var(--radius-sm)"
          >
            <i class="fas fa-chevron-down" style="font-size:10px" />
          </button>
          {menuOpen && (
            <div
              role="menu"
              style="position:absolute;top:100%;right:0;margin-top:4px;min-width:190px;max-height:320px;overflow-y:auto;padding:4px;background:var(--bg-surface);border:1px solid var(--divider-soft);border-radius:var(--radius-sm);box-shadow:var(--shadow-card);z-index:var(--z-dropdown)"
            >
              {ordered.map((seg, ri) => {
                const act = isActive(seg)
                return (
                  <button
                    key={seg.key}
                    type="button"
                    role="menuitem"
                    onClick={() => { pick(seg) }}
                    style={`display:flex;align-items:center;gap:8px;width:100%;padding:5px 8px;border:none;border-radius:var(--radius-xs);cursor:pointer;text-align:left;font-size:11px;font-weight:600;background:${act ? 'color-mix(in srgb, var(--accent-primary) 12%, transparent)' : 'transparent'};color:${act ? 'var(--accent-primary)' : 'var(--text-secondary)'}`}
                    onMouseEnter={(e) => { if (!act) (e.currentTarget).style.background = 'color-mix(in srgb, var(--text-primary) 10%, transparent)' }}
                    onMouseLeave={(e) => { if (!act) (e.currentTarget).style.background = 'transparent' }}
                  >
                    <span style="flex:1;min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">
                      {segLabel(seg, segs.length - 1 - ri, segs.length)}
                    </span>
                    <span style="flex-shrink:0;font-size:9px;color:var(--text-muted);font-weight:700">{seg.count ?? '·'}</span>
                  </button>
                )
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

/** "Current · from 05.07 14:30" for the live segment, "05.07 14:30" for an
 *  archived one. The segment with no boundary ts is the original conversation
 *  before the first compaction → "Original". */
function segLabel(seg: DisplaySegment, idx: number, total: number): string {
  const isLatest = seg.isLatest || idx === total - 1
  const tsRaw = seg.ts
  if (!tsRaw) return isLatest ? 'Current' : 'Original'
  try {
    const d = new Date(tsRaw)
    const day = d.getDate().toString().padStart(2, '0')
    const mo = (d.getMonth() + 1).toString().padStart(2, '0')
    const hh = d.getHours().toString().padStart(2, '0')
    const mm = d.getMinutes().toString().padStart(2, '0')
    return isLatest ? `Current · from ${day}.${mo} ${hh}:${mm}` : `${day}.${mo} ${hh}:${mm}`
  } catch { return `Segment ${idx + 1}` }
}

function SegButton({
  segKey, active, label, count, onClick,
}: {
  segKey: string
  active: boolean
  label: string
  count?: number
  onClick: () => void
}): JSX.Element {
  return (
    <button
      type="button"
      data-seg-key={segKey}
      onClick={onClick}
      title={count != null ? `${count} entries in this segment` : 'Load this conversation'}
      style={`
        display:inline-flex;align-items:center;gap:6px;
        padding:4px 10px;border-radius:var(--radius-md);cursor:pointer;
        background:${active ? 'color-mix(in srgb, var(--accent-primary) 12%, transparent)' : 'transparent'};
        border:none;
        color:${active ? 'var(--accent-primary)' : 'var(--text-secondary)'};
        font-size:11px;font-weight:600;letter-spacing:0.02em;
        height:26px;flex-shrink:0;white-space:nowrap;
      `}
    >
      <span>{label}</span>
      <span style={`flex-shrink:0;font-size:9px;padding:1px 6px;border-radius:var(--radius-md);background:${active ? 'color-mix(in srgb, var(--accent-primary) 24%, transparent)' : 'var(--tint-muted-medium)'};color:${active ? 'var(--accent-primary)' : 'var(--text-muted)'};font-weight:700`}>
        {count ?? '·'}
      </span>
    </button>
  )
}
