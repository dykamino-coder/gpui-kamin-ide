import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import {
  filePanelVisible,
  filePanelWidth,
  filePanelBottomVisible,
  filePanelSplit,
  rightPanelVisible,
  rightPanelWidth,
  sidebarWidth,
  activeCustomizePanel,
  getEffectiveSidebarWidth,
} from '../../signals/ui'
import { useBridge } from '../../hooks/useBridge'
import { FilePanelTerminal } from './FilePanelTerminal'
import { FileViewerTabs } from './FileViewerTabs'
import { FileEditor } from './FileEditor'
import { tabs, activeTabId } from '../../signals/tabs'

const MIN_WIDTH = 300
const MIN_SPLIT = 0.2
const MAX_SPLIT = 0.85
const CHAT_MIN_WIDTH = 550
const SIDEBAR_MIN_WIDTH = 200

/** Shared sizing base for all side panels — identical to
 *  Sidebar/RightPanel.computeSideArea. Independent of sibling widths so
 *  window shrink/expand round-trips cleanly back to the user's intended
 *  proportions, clamped only by per-panel MIN/MAX. */
function computeSideArea(viewportW: number): number {
  return Math.max(MIN_WIDTH, viewportW - CHAT_MIN_WIDTH)
}

/** New file-preview column. Vertical stack of two cards:
 *    - top: file viewer (selected file content/metadata)
 *    - bottom: tooling pane (console / git history / etc — toggleable)
 *  Both card backgrounds match the right panel's so the layout reads as
 *  three sibling cards (chat / file / right) sharing the same chrome. */
export function FilePanel(): JSX.Element | null {
  const customizing = activeCustomizePanel.value !== null && activeCustomizePanel.value !== 'landing'

  // Symmetric resize: stable intended ratio (panel / sideArea).
  // Updated only on user drag — never from the clamped value — so
  // shrink/expand round-trips back to the original size.
  const widthRef = useRef<number>(filePanelWidth.value)
  widthRef.current = filePanelWidth.value
  const intendedRatioRef = useRef<number>(
    filePanelWidth.value / Math.max(1, Math.max(MIN_WIDTH, window.innerWidth - SIDEBAR_MIN_WIDTH - CHAT_MIN_WIDTH)),
  )
  const bridge = useBridge()
  const persistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  function persistWidth(w: number): void {
    if (persistTimerRef.current) clearTimeout(persistTimerRef.current)
    persistTimerRef.current = setTimeout(() => {
      // Persist BOTH absolute width (legacy) and ratio (canonical — survives
      // window resize between sessions). Loaders prefer ratio.
      const ratio = intendedRatioRef.current
      try { bridge.setLayout({ filePanelWidth: w, filePanelWidthRatio: ratio }) } catch { /* noop */ }
    }, 250)
  }

  useEffect(() => {
    const onWinResize = (): void => {
      const newW = window.innerWidth
      if (newW === 0) return
      const newSideArea = computeSideArea(newW)
      const right = rightPanelVisible.value && !customizing ? rightPanelWidth.value : 0
      const dynamicMax = Math.max(MIN_WIDTH, newW - getEffectiveSidebarWidth() - right - CHAT_MIN_WIDTH)
      const next = Math.max(MIN_WIDTH, Math.min(dynamicMax, Math.round(intendedRatioRef.current * newSideArea)))
      if (next === widthRef.current) return
      widthRef.current = next
      filePanelWidth.value = next
      // Don't persist auto-resize-clamped values — only user drag persists.
    }

    bridge.getLayout().then((l) => {
      let ratio: number | null = null
      if (typeof l?.filePanelWidthRatio === 'number' && l.filePanelWidthRatio > 0 && l.filePanelWidthRatio < 1) {
        ratio = l.filePanelWidthRatio
      } else if (typeof l?.filePanelWidth === 'number' && l.filePanelWidth >= MIN_WIDTH) {
        // Legacy migration: derive ratio from absolute width vs current
        // sideArea. Subsequent saves will fill in `filePanelWidthRatio`.
        ratio = l.filePanelWidth / Math.max(1, computeSideArea(window.innerWidth))
      }
      if (ratio !== null) {
        intendedRatioRef.current = Math.min(0.7, Math.max(0.1, ratio))
      }
      onWinResize()
    }).catch(() => { onWinResize() })

    window.addEventListener('resize', onWinResize)
    return () => window.removeEventListener('resize', onWinResize)
  }, [])

  // Drag-resize (left edge of the column).
  const widthStart = useRef<{ x: number; w: number } | null>(null)
  function onWidthDown(e: PointerEvent): void {
    e.preventDefault()
    widthStart.current = { x: e.clientX, w: filePanelWidth.value }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }
  function onWidthMove(e: PointerEvent): void {
    if (!widthStart.current) return
    const delta = widthStart.current.x - e.clientX
    const right = rightPanelVisible.value && !customizing ? rightPanelWidth.value : 0
    const dynamicMax = Math.max(MIN_WIDTH, window.innerWidth - getEffectiveSidebarWidth() - right - CHAT_MIN_WIDTH)
    const next = Math.max(MIN_WIDTH, Math.min(dynamicMax, widthStart.current.w + delta))
    filePanelWidth.value = next
    intendedRatioRef.current = next / Math.max(1, computeSideArea(window.innerWidth))
    persistWidth(next)
  }
  function onWidthUp(e: PointerEvent): void {
    if (!widthStart.current) return
    ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    widthStart.current = null
  }

  // Drag-resize the gap between the file viewer (top) and the tooling
  // pane (bottom). Same idiom as RightPanel's split.
  const splitStart = useRef<{ y: number; ratio: number; h: number } | null>(null)
  const columnRef = useRef<HTMLDivElement | null>(null)
  function onSplitDown(e: PointerEvent): void {
    e.preventDefault()
    const h = columnRef.current?.getBoundingClientRect().height ?? 0
    splitStart.current = { y: e.clientY, ratio: filePanelSplit.value, h }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }
  function onSplitMove(e: PointerEvent): void {
    if (!splitStart.current) return
    const { y, ratio, h } = splitStart.current
    if (h <= 0) return
    const delta = (e.clientY - y) / h
    const next = Math.max(MIN_SPLIT, Math.min(MAX_SPLIT, ratio + delta))
    filePanelSplit.value = next
  }
  function onSplitUp(e: PointerEvent): void {
    if (!splitStart.current) return
    ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    splitStart.current = null
    try { bridge.setLayout({ filePanelSplit: filePanelSplit.value }) } catch { /* noop */ }
  }

  if (!filePanelVisible.value) return null
  if (customizing) return null

  const bottomOpen = filePanelBottomVisible.value
  const split = filePanelSplit.value
  const topPct = bottomOpen ? `${(split * 100).toFixed(2)}%` : '100%'
  const bottomPct = bottomOpen ? `${((1 - split) * 100).toFixed(2)}%` : '0%'

  // Right margin: 10px when followed by the right panel (so the gap to
  // its left edge stays at the same 10px chat→file uses), 20px when this
  // is the rightmost column (so it doesn't hug the window edge). Mirrors
  // MainPanel's logic — every column owns its own right margin and the
  // pattern stays consistent across the layout.
  const rightOpen = rightPanelVisible.value && !customizing
  const marginRight = rightOpen ? '10px' : 'var(--space-5)'

  return (
    <div
      ref={columnRef}
      class="file-panel-column"
      style={`
        width:${filePanelWidth.value}px;
        flex-shrink:0;
        display:flex;
        flex-direction:column;
        margin:0 ${marginRight} var(--space-5) 0;
        position:relative;
        min-width:${MIN_WIDTH}px;
      `}
    >
      <div
        class="file-panel-resize-handle"
        data-tooltip="Drag to resize"
        onPointerDown={onWidthDown}
        onPointerMove={onWidthMove}
        onPointerUp={onWidthUp}
        onPointerCancel={onWidthUp}
      >
        <span class="file-panel-resize-handle-bar" aria-hidden="true" />
      </div>

      <aside
        class="file-panel-card"
        style={`
          height:${topPct};
          min-height:0;
          display:flex;
          flex-direction:column;
          overflow:hidden;
        `}
      >
        <FileViewerTabs />
        <FileEditor />
      </aside>

      {bottomOpen && (
        <>
          <div
            onPointerDown={onSplitDown}
            onPointerMove={onSplitMove}
            onPointerUp={onSplitUp}
            onPointerCancel={onSplitUp}
            style="flex-shrink:0;height:10px;cursor:row-resize;position:relative;display:flex;align-items:center;justify-content:center"
          >
            <div style="width:32px;height:3px;background:var(--bg-overlay);border-radius:var(--radius-xs);opacity:0.7" />
          </div>
          <aside
            class="file-panel-card"
            style={`
              height:${bottomPct};
              min-height:0;
              display:flex;
              flex-direction:column;
              overflow:hidden;
            `}
          >
            {/* Per-tab terminals — render every open tab's terminal at
                once and toggle visibility, so each chat keeps its own
                live PTY across tab switches. New tabs spin up a fresh
                xterm/PTY pair on first render; closed tabs unmount and
                their PTY is killed by the cleanup effect inside the
                FilePanelTerminal component. */}
            {tabs.value.map((t) => (
              <FilePanelTerminal
                key={t.id}
                tabId={t.id}
                cwd={t.cwd}
                visible={t.id === activeTabId.value}
              />
            ))}
          </aside>
        </>
      )}
    </div>
  )
}

function FilePanelHeader({ title, icon }: { title: string; icon: string }): JSX.Element {
  return (
    <div style="padding:8px 12px;border-bottom:1px solid var(--bg-surface);display:flex;align-items:center;gap:8px;font-size:11px;font-weight:600;color:var(--text-secondary);text-transform:lowercase;letter-spacing:0.04em;flex-shrink:0">
      <i class={`fas ${icon}`} style="font-size:11px;color:var(--accent-primary)" />
      {title}
    </div>
  )
}

function FileViewerStub(): JSX.Element {
  return (
    <div style="flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:24px;color:var(--text-disabled);font-size:11px;text-align:center;line-height:1.5">
      File viewer placeholder.<br />
      Selected files and details will render here.
    </div>
  )
}

