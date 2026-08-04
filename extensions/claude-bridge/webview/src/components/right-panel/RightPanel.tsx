import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { rightPanelVisible, rightPanelWidth, rightPanelSplit, rightPanelBottomTab, activeCustomizePanel, sidebarWidth, filePanelVisible, filePanelWidth, getEffectiveSidebarWidth } from '../../signals/ui'
import { activePlan } from '../../signals/plan'
import { activeTodos } from '../../signals/todos'
import { PlanList } from './PlanList'
import { TodoList } from './TodoList'
import { FilesList } from './FilesList'
import { CardTabBar } from './CardTabBar'

// Right side-panel (Plans / Todos / Files) width — must stay >=300 so
// long task / file names don't truncate. No hard upper cap any more —
// the only ceiling is the dynamic one (window.innerWidth - CHAT_MIN_WIDTH)
// so the chat column stays readable. Effective max therefore scales with
// the user's monitor width.
const MIN_WIDTH = 300
const MIN_SPLIT = 0.15
const MAX_SPLIT = 0.85
/** Hard floor for the chat column to remain visible. resize calls clamp
 *  rightPanelWidth so the chat never drops below this. */
const CHAT_MIN_WIDTH = 550
/** Floor for the sidebar's reserved slot — must match Sidebar.MIN_WIDTH
 *  so the right panel never assumes the sidebar can shrink below it. */
const SIDEBAR_MIN_WIDTH = 200

/** Shared sizing base for all side panels — identical to
 *  Sidebar.computeSideArea. Deliberately independent of sibling widths
 *  so window shrink/expand cycles don't drift: clamped siblings would
 *  otherwise feed an inflated sideArea on the way back up. */
function computeSideArea(viewportW: number): number {
  return Math.max(MIN_WIDTH, viewportW - CHAT_MIN_WIDTH)
}

export function RightPanel(): JSX.Element | null {
  // Hide whenever the user is in any Customize sub-panel — the right
  // column is irrelevant outside the chat and was eating valuable width
  // from the settings/skills/agents/etc. content area.
  const customizing = activeCustomizePanel.value !== null && activeCustomizePanel.value !== 'landing'

  // Symmetric resize: keep the panel's INTENDED share of the side-panels
  // area stable when the window resizes. intendedRatio is set on init
  // and updated on user drag — never recomputed from the clamped value
  // — so shrink → expand round-trips back to the original size.
  const widthRef = useRef<number>(rightPanelWidth.value)
  widthRef.current = rightPanelWidth.value
  const intendedRatioRef = useRef<number>(
    rightPanelWidth.value / Math.max(1, Math.max(MIN_WIDTH, window.innerWidth - SIDEBAR_MIN_WIDTH - CHAT_MIN_WIDTH)),
  )
  useEffect(() => {
    const onWinResize = (): void => {
      const newW = window.innerWidth
      if (newW === 0) return
      const newSideArea = computeSideArea(newW)
      const cust = activeCustomizePanel.value !== null && activeCustomizePanel.value !== 'landing'
      const file = filePanelVisible.value && !cust ? filePanelWidth.value : 0
      const dynamicMax = Math.max(MIN_WIDTH, newW - getEffectiveSidebarWidth() - file - CHAT_MIN_WIDTH)
      const next = Math.max(MIN_WIDTH, Math.min(dynamicMax, Math.round(intendedRatioRef.current * newSideArea)))
      if (next === widthRef.current) return
      widthRef.current = next
      rightPanelWidth.value = next
      // No localStorage write here — auto-resize-clamped values would
      // overwrite the user's last drag intent across an app restart.
    }

    // Hydrate ratio from storage. Prefer the new ratio key; fall
    // back to legacy absolute px (migrated to ratio against current
    // viewport). Then apply against current viewport — Electron doesn't
    // fire 'resize' on initial paint, so without this manual call a
    // session saved on a maximised window would render at the saved px
    // on a smaller windowed reopen.
    const savedRatioRaw = storage.getItem('right-panel-width-ratio')
    const savedRatio = savedRatioRaw ? Number(savedRatioRaw) : NaN
    const savedPxRaw = storage.getItem('right-panel-width')
    const savedPx = savedPxRaw ? Number(savedPxRaw) : NaN
    let ratio: number | null = null
    if (Number.isFinite(savedRatio) && savedRatio > 0 && savedRatio < 1) {
      ratio = savedRatio
    } else if (Number.isFinite(savedPx) && savedPx >= MIN_WIDTH) {
      ratio = savedPx / Math.max(1, computeSideArea(window.innerWidth))
    }
    if (ratio !== null) {
      intendedRatioRef.current = Math.min(0.7, Math.max(0.1, ratio))
    }
    onWinResize()

    window.addEventListener('resize', onWinResize)
    return () => window.removeEventListener('resize', onWinResize)
  }, [])

  // Drag-to-resize the left edge of the column. Lives on the outer
  // wrapper so the user can grab anywhere along the left margin of
  // either card and resize the whole column at once. When the file
  // panel sits between chat and this panel, the handle shares space
  // with the file panel directly (its immediate left neighbour) — the
  // chat column shouldn't budge while the user adjusts the boundary
  // they're actually grabbing. Without `fileW0` the file panel kept
  // its size and chat absorbed the delta, which felt wrong.
  const widthStart = useRef<{ x: number; w: number; fileW0: number } | null>(null)
  function onWidthDown(e: PointerEvent): void {
    e.preventDefault()
    widthStart.current = { x: e.clientX, w: rightPanelWidth.value, fileW0: filePanelWidth.value }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }
  function onWidthMove(e: PointerEvent): void {
    if (!widthStart.current) return
    const delta = widthStart.current.x - e.clientX
    const customizing = activeCustomizePanel.value !== null && activeCustomizePanel.value !== 'landing'
    const fileOpen = filePanelVisible.value && !customizing
    if (fileOpen) {
      // File panel is the immediate left neighbour. Trade pixels 1:1
      // with it (clamped at file MIN_WIDTH=300). Chat stays put.
      const FILE_MIN = 300
      const maxRightGain = Math.max(0, widthStart.current.fileW0 - FILE_MIN)
      const dynamicMax = widthStart.current.w + maxRightGain
      const nextRight = Math.max(MIN_WIDTH, Math.min(dynamicMax, widthStart.current.w + delta))
      const nextFile = Math.max(FILE_MIN, widthStart.current.fileW0 + (widthStart.current.w - nextRight))
      rightPanelWidth.value = nextRight
      filePanelWidth.value = nextFile
      // Keep ratios anchored to user intent for subsequent window resizes.
      intendedRatioRef.current = nextRight / Math.max(1, computeSideArea(window.innerWidth))
    } else {
      // No file panel — original behaviour (chat absorbs the delta).
      const dynamicMax = Math.max(MIN_WIDTH, window.innerWidth - getEffectiveSidebarWidth() - CHAT_MIN_WIDTH)
      const next = Math.max(MIN_WIDTH, Math.min(dynamicMax, widthStart.current.w + delta))
      rightPanelWidth.value = next
      intendedRatioRef.current = next / Math.max(1, computeSideArea(window.innerWidth))
    }
  }
  function onWidthUp(e: PointerEvent): void {
    if (!widthStart.current) return
    ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    widthStart.current = null
    // Persist BOTH absolute width (legacy) and ratio (canonical — survives
    // window resize between sessions). Loaders prefer ratio.
    storage.setItem('right-panel-width', String(rightPanelWidth.value))
    storage.setItem('right-panel-width-ratio', String(intendedRatioRef.current))
  }

  // Drag-to-resize the gap between Plan card (top) and Files card
  // (bottom). The split is now BETWEEN two distinct cards rather than
  // inside one — handle still lives where the cards touch but renders
  // as a thin neutral strip outside the card backgrounds so each card
  // keeps its self-contained chrome.
  const splitStart = useRef<{ y: number; ratio: number; h: number } | null>(null)
  const columnRef = useRef<HTMLDivElement | null>(null)
  function onSplitDown(e: PointerEvent): void {
    e.preventDefault()
    const h = columnRef.current?.getBoundingClientRect().height ?? 0
    splitStart.current = { y: e.clientY, ratio: rightPanelSplit.value, h }
    ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
  }
  function onSplitMove(e: PointerEvent): void {
    if (!splitStart.current) return
    const { y, ratio, h } = splitStart.current
    if (h <= 0) return
    const delta = (e.clientY - y) / h
    const next = Math.max(MIN_SPLIT, Math.min(MAX_SPLIT, ratio + delta))
    rightPanelSplit.value = next
  }
  function onSplitUp(e: PointerEvent): void {
    if (!splitStart.current) return
    ;(e.target as HTMLElement).releasePointerCapture(e.pointerId)
    splitStart.current = null
    storage.setItem('right-panel-split', String(rightPanelSplit.value))
  }

  if (!rightPanelVisible.value) return null
  if (customizing) return null

  const split = rightPanelSplit.value
  const topPct = `${(split * 100).toFixed(2)}%`
  const bottomPct = `${((1 - split) * 100).toFixed(2)}%`

  // Outer column flex-stack: card 1 + splitter + card 2. Each card has
  // its own border-radius/background so they read as TWO separate
  // surfaces, like every other panel in the app.
  return (
    <div
      ref={columnRef}
      class="right-panel-column"
      style={`
        width:${rightPanelWidth.value}px;
        flex-shrink:0;
        display:flex;
        flex-direction:column;
        margin:0 var(--space-5) var(--space-5) 0;
        position:relative;
        min-width:${MIN_WIDTH}px;
      `}
    >
      {/* Left-edge resize handle for column width — same UX as the
          sidebar resize handle (data-tooltip + on-hover highlight) so the
          two grippers feel like the same control. The visible bar is a
          vertical gradient (transparent → accent → transparent, top-down)
          rendered in the inner ::after via inline background. */}
      <div
        class="right-panel-resize-handle"
        data-tooltip="Drag to resize"
        onPointerDown={onWidthDown}
        onPointerMove={onWidthMove}
        onPointerUp={onWidthUp}
        onPointerCancel={onWidthUp}
      >
        <span class="right-panel-resize-handle-bar" aria-hidden="true" />
      </div>

      <aside
        class="right-panel-card"
        style={`
          height:${topPct};
          min-height:0;
          display:flex;
          flex-direction:column;
          overflow:hidden;
        `}
      >
        <FilesList />
      </aside>

      {/* Drag-handle strip between the two cards. Transparent base so
          the dotted appWrapper bg shows through and visually separates
          the cards; the small grip bar centred on the strip telegraphs
          that it's draggable. */}
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
        class="right-panel-card"
        style={`
          height:${bottomPct};
          min-height:0;
          display:flex;
          flex-direction:column;
          overflow:hidden;
        `}
      >
        <CardTabBar
          tabs={[
            { id: 'plan', label: 'Plan', icon: 'fa-list-check', accent: 'var(--accent-purple)', count: activePlan.value?.total ?? 0 },
            { id: 'todos', label: 'Todos', icon: 'fa-list-ul', accent: 'var(--accent-primary)', count: activeTodos.value?.total ?? 0 },
          ]}
          active={rightPanelBottomTab.value}
          onSelect={(id) => {
            const next = id as 'plan' | 'todos'
            rightPanelBottomTab.value = next
            storage.setItem('right-panel-bottom-tab', next)
          }}
        />
        {rightPanelBottomTab.value === 'todos' ? <TodoList /> : <PlanList />}
      </aside>
    </div>
  )
}

