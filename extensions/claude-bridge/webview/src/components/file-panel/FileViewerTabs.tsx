import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import {
  activeOpenFiles,
  activeOpenFilePath,
  closeAllInTab,
  closeFileInTab,
  closeOthersInTab,
  closeToRightInTab,
  setActiveFile,
} from '../../signals/file-viewer'
import tabStyles from '../titlebar/TabsBar.module.css'

function basename(p: string): string {
  return p.replace(/^.*[/\\]/, '')
}

/** Tab strip showing every file open in the active chat tab. Click →
 *  switch active file. Middle-click / × → close. Mirrors the chat
 *  TabsBar UX: tabs share row width and shrink before chevron buttons
 *  appear when the row overflows. */
export function FileViewerTabs(): JSX.Element {
  const files = activeOpenFiles.value
  const active = activeOpenFilePath.value
  const stripRef = useRef<HTMLDivElement | null>(null)
  const [overflow, setOverflow] = useState(false)
  const [canLeft, setCanLeft] = useState(false)
  const [canRight, setCanRight] = useState(false)
  const [ctxMenu, setCtxMenu] = useState<{ x: number; y: number; path: string } | null>(null)

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
    const onScroll = (): void => recompute()
    el.addEventListener('scroll', onScroll, { passive: true })
    return () => {
      ro.disconnect()
      el.removeEventListener('scroll', onScroll)
    }
  }, [files.length])

  function step(dir: 'left' | 'right'): void {
    const el = stripRef.current
    if (!el) return
    const children = Array.from(el.children) as HTMLElement[]
    const viewportLeft = el.scrollLeft
    const viewportRight = el.scrollLeft + el.clientWidth
    if (dir === 'right') {
      const next = children.find(c => c.offsetLeft + c.offsetWidth > viewportRight + 1)
      if (next) el.scrollTo({ left: Math.max(0, next.offsetLeft - 8), behavior: 'smooth' })
    } else {
      const prev = [...children].reverse().find(c => c.offsetLeft < viewportLeft - 1)
      if (prev) el.scrollTo({ left: Math.max(0, prev.offsetLeft - 8), behavior: 'smooth' })
    }
  }

  if (files.length === 0) return <div style="height:34px;flex-shrink:0" />

  return (
    <div style="z-index:5;padding:6px 8px;display:flex;align-items:center;gap:4px;flex-shrink:0">
      {overflow && (
        <button
          type="button"
          onClick={() => step('left')}
          disabled={!canLeft}
          aria-label="Scroll files left"
          style={`flex-shrink:0;width:22px;height:22px;display:inline-flex;align-items:center;justify-content:center;border:none;background:transparent;color:${canLeft ? 'var(--text-secondary)' : 'var(--text-disabled)'};cursor:${canLeft ? 'pointer' : 'default'};border-radius:var(--radius-sm)`}
        >
          <i class="fas fa-chevron-left" style="font-size:10px" />
        </button>
      )}
      <div
        ref={stripRef}
        style="display:flex;align-items:center;gap:4px;flex:1;min-width:0;overflow-x:auto;scrollbar-width:none"
        class="conv-seg-strip"
      >
        {files.map(f => (
          <FileTab
            key={f.path}
            path={f.path}
            dirty={f.dirty}
            active={f.path === active}
            onActivate={() => {
              const tab = activeTabId.value
              if (tab) setActiveFile(tab, f.path)
            }}
            onClose={() => {
              const tab = activeTabId.value
              if (tab) closeFileInTab(tab, f.path)
            }}
            onContextMenu={(e) => {
              e.preventDefault()
              setCtxMenu({ x: e.clientX, y: e.clientY, path: f.path })
            }}
          />
        ))}
      </div>
      {overflow && (
        <button
          type="button"
          onClick={() => step('right')}
          disabled={!canRight}
          aria-label="Scroll files right"
          style={`flex-shrink:0;width:22px;height:22px;display:inline-flex;align-items:center;justify-content:center;border:none;background:transparent;color:${canRight ? 'var(--text-secondary)' : 'var(--text-disabled)'};cursor:${canRight ? 'pointer' : 'default'};border-radius:var(--radius-sm)`}
        >
          <i class="fas fa-chevron-right" style="font-size:10px" />
        </button>
      )}
      {ctxMenu && (
        <FileTabContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          path={ctxMenu.path}
          allFiles={files.map(f => f.path)}
          onClose={() => setCtxMenu(null)}
        />
      )}
    </div>
  )
}

/** Right-click context menu for an individual file tab. Mirrors the
 *  chat TabsBar context menu (Close / Close others / Close all) and
 *  adds the "Close to the right" command — handy when a chat opened
 *  five files in a row and the user wants to keep just the lefthand
 *  cluster. Uses the same `.ctx*` styles from TabsBar.module.css so
 *  the visual is identical to chat-tab right-clicks. */
function FileTabContextMenu({ x, y, path, allFiles, onClose }: {
  x: number
  y: number
  path: string
  allFiles: string[]
  onClose: () => void
}): JSX.Element {
  useEffect(() => {
    function onDown(): void { onClose() }
    function onKey(e: KeyboardEvent): void { if (e.key === 'Escape') onClose() }
    window.addEventListener('mousedown', onDown)
    window.addEventListener('keydown', onKey)
    return () => {
      window.removeEventListener('mousedown', onDown)
      window.removeEventListener('keydown', onKey)
    }
  }, [onClose])

  const idx = allFiles.indexOf(path)
  const othersCount = allFiles.length - 1
  const rightCount = idx >= 0 ? allFiles.length - idx - 1 : 0

  return (
    <div
      class={tabStyles.ctxMenu}
      style={`left:${x}px;top:${y}px`}
      onMouseDown={(e: MouseEvent) => e.stopPropagation()}
    >
      <button
        type="button"
        class={tabStyles.ctxItem}
        onClick={() => {
          const tab = activeTabId.value
          if (tab) closeFileInTab(tab, path)
          onClose()
        }}
      >
        <i class="fas fa-xmark" /> Close
      </button>
      <button
        type="button"
        class={tabStyles.ctxItem}
        onClick={() => {
          const tab = activeTabId.value
          if (tab) closeOthersInTab(tab, path)
          onClose()
        }}
        disabled={othersCount === 0}
      >
        <i class="fas fa-rectangle-xmark" /> Close others
      </button>
      <button
        type="button"
        class={tabStyles.ctxItem}
        onClick={() => {
          const tab = activeTabId.value
          if (tab) closeToRightInTab(tab, path)
          onClose()
        }}
        disabled={rightCount === 0}
      >
        <i class="fas fa-angles-right" /> Close to the right
      </button>
      <button
        type="button"
        class={`${tabStyles.ctxItem} ${tabStyles.ctxDanger}`}
        onClick={() => {
          const tab = activeTabId.value
          if (tab) closeAllInTab(tab)
          onClose()
        }}
        disabled={allFiles.length === 0}
      >
        <i class="fas fa-trash" /> Close all
      </button>
    </div>
  )
}

function FileTab({ path, dirty, active, onActivate, onClose, onContextMenu }: {
  path: string
  dirty: boolean
  active: boolean
  onActivate: () => void
  onClose: () => void
  onContextMenu: (e: MouseEvent) => void
}): JSX.Element {
  return (
    <div
      onClick={onActivate}
      onAuxClick={(e: any) => { if (e.button === 1) { e.preventDefault(); onClose() } }}
      onContextMenu={onContextMenu}
      data-tooltip={path}
      style={`
        display:inline-flex;align-items:center;gap:6px;
        padding:4px 6px 4px 10px;border-radius:var(--radius-md);cursor:pointer;
        background:${active ? 'color-mix(in srgb, var(--accent-primary) 12%, transparent)' : 'transparent'};
        color:${active ? 'var(--accent-primary)' : 'var(--text-secondary)'};
        font-size:11px;font-weight:500;letter-spacing:0.02em;
        height:24px;flex-shrink:1;min-width:60px;max-width:220px;
        white-space:nowrap;overflow:hidden;
      `}
    >
      <span style="overflow:hidden;text-overflow:ellipsis;min-width:0">{basename(path)}</span>
      <button
        type="button"
        onClick={(e: MouseEvent) => { e.stopPropagation(); onClose() }}
        aria-label="Close file"
        title={dirty ? 'Unsaved changes' : 'Close'}
        style={`flex-shrink:0;width:14px;height:14px;display:inline-flex;align-items:center;justify-content:center;border:none;background:transparent;color:${active ? 'var(--accent-primary)' : 'var(--text-muted)'};cursor:pointer;border-radius:var(--radius-xs);font-size:9px`}
      >
        <i class={dirty ? 'fas fa-circle' : 'fas fa-xmark'} style={`font-size:${dirty ? '6px' : '9px'}`} />
      </button>
    </div>
  )
}
