import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { useBridge } from '../../hooks/useBridge'
import {
  filePanelVisible,
  filePanelBottomVisible,
  rightPanelVisible,
} from '../../signals/ui'

/** Three side-panel toggles, grouped like a segmented control:
 *    1. File column (top region of the file-preview column)
 *    2. Bottom region of the file-preview column (console / git / etc)
 *    3. Right activity column (Plan / Todos / Files list)
 *  Each writes to its own signal AND persists via the layout store so
 *  the configuration survives relaunch. The bottom region's toggle is
 *  disabled (and dimmed) when the file column is hidden — there's no
 *  parent surface to render it into. */
export function PanelTripletToggle(): JSX.Element {
  const bridge = useBridge()
  const fileOn = filePanelVisible.value
  const bottomOn = filePanelBottomVisible.value
  const rightOn = rightPanelVisible.value

  function toggleFile(): void {
    const next = !fileOn
    filePanelVisible.value = next
    try { bridge.setLayout({ filePanelVisible: next }) } catch { /* noop */ }
  }
  function toggleBottom(): void {
    if (!fileOn) return
    const next = !bottomOn
    filePanelBottomVisible.value = next
    try { bridge.setLayout({ filePanelBottomVisible: next }) } catch { /* noop */ }
  }
  function toggleRight(): void {
    const next = !rightOn
    rightPanelVisible.value = next
    storage.setItem('right-panel-visible', String(next))
  }

  return (
    <div class="header-btn-group" role="group" aria-label="Side panels">
      <button
        type="button"
        class={`header-btn${fileOn ? ' active' : ''}`}
        onClick={toggleFile}
        data-tooltip={fileOn ? 'Hide file column' : 'Show file column'}
        aria-pressed={fileOn}
      >
        <PanelIcon variant="center" />
      </button>
      <button
        type="button"
        class={`header-btn${bottomOn && fileOn ? ' active' : ''}`}
        onClick={toggleBottom}
        disabled={!fileOn}
        data-tooltip={!fileOn ? 'Show file column first' : (bottomOn ? 'Hide bottom pane' : 'Show bottom pane')}
        aria-pressed={bottomOn && fileOn}
        style={!fileOn ? 'opacity:0.4;cursor:default' : ''}
      >
        <PanelIcon variant="bottom" />
      </button>
      <button
        type="button"
        class={`header-btn${rightOn ? ' active' : ''}`}
        onClick={toggleRight}
        data-tooltip={rightOn ? 'Hide activity panel' : 'Show plan & files'}
        aria-pressed={rightOn}
      >
        <PanelIcon variant="right" />
      </button>
    </div>
  )
}

/** Inline SVG icons depicting which panel slot each button toggles —
 *  small framed rectangle with a highlighted region matching the slot.
 *  Stroke uses currentColor so .header-btn.active recolors correctly. */
function PanelIcon({ variant }: { variant: 'center' | 'bottom' | 'right' }): JSX.Element {
  const w = 14
  const h = 12
  const r = 1.5
  const frame = (
    <rect x={1} y={1} width={w - 2} height={h - 2} rx={r} ry={r} fill="none" stroke="currentColor" stroke-width={1.2} />
  )
  let highlight: JSX.Element | null = null
  if (variant === 'center') {
    // File column lives between chat and right panel — visually a
    // central slot rather than a left edge. The rectangle is centred
    // horizontally so the icon mirrors the actual layout position.
    const cw = 4.5
    highlight = <rect x={(w - cw) / 2} y={1.5} width={cw} height={h - 3} rx={1} ry={1} fill="currentColor" opacity={0.85} />
  } else if (variant === 'right') {
    highlight = <rect x={w - 6} y={1.5} width={4.5} height={h - 3} rx={1} ry={1} fill="currentColor" opacity={0.85} />
  } else {
    highlight = <rect x={1.5} y={h - 5} width={w - 3} height={3.5} rx={1} ry={1} fill="currentColor" opacity={0.85} />
  }
  return (
    <svg width={w} height={h} viewBox={`0 0 ${w} ${h}`} aria-hidden="true">
      {highlight}
      {frame}
    </svg>
  )
}
