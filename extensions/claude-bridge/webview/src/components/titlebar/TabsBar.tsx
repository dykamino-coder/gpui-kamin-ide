import type { JSX } from 'preact'
import { useComputed } from '@preact/signals'
import { useEffect, useRef, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { tabs, activeTabId, switchTabLocal, pinnedTabs, topTabsOrder, ghostPinnedTabs } from '../../signals/tabs'
import { savedSessionsByConvId } from '../../signals/saved-sessions'
import { tabActivity, sidebarMode, pinnedTitleColors } from '../../signals/ui'
import { togglePinTab, moveTopTab } from '../../lib/tabs-persist'
import { getAgentColor, resolvePinnedColor } from '../../utils/agent-color'
import { useTabDrag } from './useTabDrag'
import { buildActiveTabs } from './tabs-list'
import { resumeGhostTab } from './resume-ghost-tab'
import { TabContextMenu } from './TabContextMenu'
import { TabChip } from './TabChip'
import { NewSessionModal } from './NewSessionModal'
import styles from './TabsBar.module.css'

/**
 * Chrome-style tab strip. Tabs share the available width equally and shrink
 * together as more sessions open. When even the minimum per-tab width can't
 * fit the row, left/right arrow buttons appear and step the strip by one tab
 * at a time — we deliberately avoid a free-scroll to keep the UX mouse-wheel
 * neutral (no accidental scrolls from trackpad momentum).
 */
export function TabsBar(): JSX.Element | null {
  const bridge = useBridge()

  const activeTabs = useComputed(() => buildActiveTabs(
    tabs.value,
    pinnedTabs.value,
    topTabsOrder.value,
    ghostPinnedTabs.value,
    savedSessionsByConvId.value,
  ))

  const stripRef = useRef<HTMLDivElement | null>(null)
  const [overflow, setOverflow] = useState(false)
  const [canLeft, setCanLeft] = useState(false)
  const [canRight, setCanRight] = useState(false)
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; tabId: string } | null>(null)
  const [newPickerOpen, setNewPickerOpen] = useState(false)

  // Pointer-based drag — extracted into useTabDrag so the visual component
  // shrinks below the god-file threshold. Bypasses HTML5 DnD because
  // document-level listeners from useDragDrop swallow drop events for our
  // tab payload type.
  const drag = useTabDrag(stripRef, (srcId, dstId, after) => moveTopTab(srcId, dstId, after))
  const { draggingId, dndOverId, dndOverSide } = drag
  const handlePointerDown = drag.handlers.onPointerDown
  const handlePointerMove = drag.handlers.onPointerMove
  const handlePointerUp = drag.handlers.onPointerUp
  const handlePointerCancel = drag.handlers.onPointerCancel

  function recompute(): void {
    const el = stripRef.current
    if (!el) return
    const over = el.scrollWidth > el.clientWidth + 1
    setOverflow(over)
    setCanLeft(el.scrollLeft > 1)
    setCanRight(over && (el.scrollLeft + el.clientWidth < el.scrollWidth - 1))
  }

  useEffect(() => {
    recompute()
    const el = stripRef.current
    if (!el) return
    const ro = new ResizeObserver(() => recompute())
    ro.observe(el)
    for (const child of Array.from(el.children)) ro.observe(child as Element)
    return () => ro.disconnect()
  }, [activeTabs.value.length])

  useEffect(() => { recompute() }, [activeTabId.value])

  if (activeTabs.value.length === 0) return null
  if (sidebarMode.value === 'customize') return null

  async function handleActivate(tabId: string): Promise<void> {
    if (tabId.startsWith('ghost:')) {
      const convId = tabId.slice('ghost:'.length)
      const ghost = ghostPinnedTabs.value.find(g => (g.conversationId || g.id) === convId)
      if (!ghost) return
      await resumeGhostTab(bridge, ghost, savedSessionsByConvId.value)
      return
    }
    switchTabLocal(tabId) // optimistic + record intent so a stale tab:switched can't overtake
    bridge?.switchTab?.(tabId)
    requestAnimationFrame(() => {
      const el = stripRef.current
      if (!el) return
      const active = el.querySelector<HTMLElement>(`[data-tab-id="${tabId}"]`)
      if (active) {
        const left = active.offsetLeft
        const right = left + active.offsetWidth
        if (left < el.scrollLeft) el.scrollTo({ left, behavior: 'smooth' })
        else if (right > el.scrollLeft + el.clientWidth) el.scrollTo({ left: right - el.clientWidth, behavior: 'smooth' })
      }
    })
  }

  function handleClose(e: MouseEvent, tabId: string): void {
    e.stopPropagation()
    if (tabId.startsWith('ghost:')) {
      // Unpinning a ghost removes it from the persisted state.
      const convId = tabId.slice('ghost:'.length)
      const ghost = ghostPinnedTabs.value.find(g => (g.conversationId || g.id) === convId)
      if (ghost) ghostPinnedTabs.value = ghostPinnedTabs.value.filter(g => g !== ghost)
      return
    }
    bridge?.closeTab?.(tabId)
  }

  function handleContextMenu(e: MouseEvent, tabId: string): void {
    e.preventDefault()
    e.stopPropagation()
    setCtxMenu({ x: e.clientX, y: e.clientY, tabId })
  }

  function closeOthers(keepId: string): void {
    setCtxMenu(null)
    for (const t of activeTabs.value) {
      if (t.id === keepId) continue
      if (t.id.startsWith('ghost:')) continue
      bridge?.closeTab?.(t.id)
    }
  }

  function closeAll(): void {
    setCtxMenu(null)
    for (const t of activeTabs.value) {
      if (t.id.startsWith('ghost:')) continue
      bridge?.closeTab?.(t.id)
    }
  }

  function handleTogglePin(e: MouseEvent, tabId: string): void {
    e.stopPropagation()
    if (tabId.startsWith('ghost:')) {
      // Unpin a ghost → drop it entirely (no live tab to flip).
      const convId = tabId.slice('ghost:'.length)
      ghostPinnedTabs.value = ghostPinnedTabs.value.filter(g => (g.conversationId || g.id) !== convId)
      return
    }
    togglePinTab(tabId)
  }

  function stepStrip(dir: -1 | 1): void {
    const el = stripRef.current
    if (!el) return
    const children = Array.from(el.children) as HTMLElement[]
    if (children.length === 0) return
    const avg = children.reduce((s, c) => s + c.offsetWidth, 0) / children.length
    el.scrollBy({ left: dir * Math.max(avg, 80), behavior: 'smooth' })
    setTimeout(recompute, 200)
  }

  return (
    <div class={styles.bar}>
      {overflow && (
        <button
          class={`${styles.arrow} ${!canLeft ? styles.arrowDisabled : ''}`}
          onClick={() => stepStrip(-1)}
          disabled={!canLeft}
          aria-label="Scroll tabs left"
          title="Previous tab"
        >
          <i class="fas fa-chevron-left" />
        </button>
      )}
      <div
        class={styles.strip}
        ref={stripRef}
        onScroll={recompute}
        onPointerMove={handlePointerMove}
        onPointerUp={handlePointerUp}
        onPointerCancel={handlePointerCancel}
      >
        {activeTabs.value.map((t) => {
          const isGhost = t.id.startsWith('ghost:')
          const isSleeping = isGhost || t.status === 'disconnected' || t.status === 'error'
          const title = t.sessionTitle?.trim() || t.folderName || 'session'
          const pinnedColor = resolvePinnedColor(t.sessionTitle ? pinnedTitleColors.value[t.sessionTitle] : undefined) || undefined
          const realColor = pinnedColor || getAgentColor(t.sessionTitle) || null
          const color = realColor || 'var(--accent-primary)'
          const isActive = t.id === activeTabId.value
          const isPinned = isGhost || pinnedTabs.value.has(t.id)
          const working = tabActivity.value.get(t.id)?.isWorking === true
          const tooltip = `${title}\n${t.folderName} · ${t.cwd}${isSleeping ? '\n(click to reconnect)' : isPinned ? '\n(pinned)' : ''}`
          const isDrag = draggingId === t.id
          const isOver = dndOverId === t.id && draggingId !== null && draggingId !== t.id
          return (
            <TabChip
              key={t.id}
              tab={t}
              isActive={isActive}
              isPinned={isPinned}
              isSleeping={isSleeping}
              isTinted={!!realColor}
              isDragging={isDrag}
              isOverLeft={isOver && dndOverSide === 'left'}
              isOverRight={isOver && dndOverSide === 'right'}
              working={working}
              color={color}
              title={title}
              tooltip={tooltip}
              onActivate={() => handleActivate(t.id)}
              onPointerDown={(e) => handlePointerDown(e, t.id)}
              onContextMenu={(e) => handleContextMenu(e, t.id)}
              onTogglePin={(e) => handleTogglePin(e, t.id)}
              onClose={(e) => handleClose(e, t.id)}
            />
          )
        })}
        {/* Chrome-style "new tab" affordance — lives INSIDE the strip
            so it sits right after the last tab regardless of how the
            strip's flex space distributes. Outside the strip the
            `flex:1` strip pushed the `+` to the far edge of the bar
            with empty space between, which the user read as
            disconnected from the tab row. */}
        <button
          type="button"
          onClick={() => setNewPickerOpen(true)}
          data-tooltip="New session…"
          aria-label="New session"
          class={styles.newTabButton}
        >
          <i class="fas fa-plus" />
        </button>
      </div>
      {overflow && (
        <button
          class={`${styles.arrow} ${!canRight ? styles.arrowDisabled : ''}`}
          onClick={() => stepStrip(1)}
          disabled={!canRight}
          aria-label="Scroll tabs right"
          title="Next tab"
        >
          <i class="fas fa-chevron-right" />
        </button>
      )}
      {newPickerOpen && <NewSessionModal onClose={() => setNewPickerOpen(false)} />}
      {ctxMenu && (
        <TabContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          tabId={ctxMenu.tabId}
          activeTabs={activeTabs.value}
          onClose={() => setCtxMenu(null)}
          onCloseTab={(id) => bridge?.closeTab?.(id)}
          onCloseOthers={closeOthers}
          onCloseAll={closeAll}
        />
      )}
    </div>
  )
}
