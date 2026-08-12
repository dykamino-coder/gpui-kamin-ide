import type { JSX } from 'preact'
import { useState, useEffect, useRef } from 'preact/hooks'
import { sidebarMode, updateInfo, rightPanelVisible, rightPanelWidth, activeCustomizePanel, sidebarWidth, filePanelVisible, filePanelWidth, sidebarVisible } from '../../signals/ui'
import { SessionsMode } from './sessions/SessionsMode'
import { CustomizeMode } from './customize/CustomizeMode'
import { useBridge } from '../../hooks/useBridge'
import styles from './Sidebar.module.css'

const MIN_WIDTH = 200
const MAX_WIDTH = 520
const DEFAULT_WIDTH = 270
const CHAT_MIN_WIDTH = 550

/** Shared sizing base for all three side panels (sidebar / file / right).
 *  Deliberately independent of the OTHER panels' current widths — using
 *  `viewport - chat-min` gives every panel the same monotonic reference,
 *  so each one's `intendedRatio × sideArea` scales linearly with the
 *  window. If we subtracted siblings here, a window-shrink would clamp
 *  the others at their MIN_WIDTH, inflate this panel's sideArea relative
 *  to the original, and the next window-grow tick would compute a wrong
 *  width because the sibling widths feeding sideArea hadn't recovered
 *  yet — manifesting as the sidebar drifting on shrink/expand cycles. */
function computeSideArea(viewportW: number): number {
  return Math.max(MIN_WIDTH, viewportW - CHAT_MIN_WIDTH)
}

export function Sidebar(): JSX.Element | null {
  const mode = sidebarMode.value
  const bridge = useBridge()
  const [version, setVersion] = useState('')
  const [width, setWidth] = useState(DEFAULT_WIDTH)
  const widthRef = useRef(DEFAULT_WIDTH)
  // Symmetric resize: store the user's INTENDED ratio (sidebar / sideArea)
  // and reapply it to every resize tick. Without this, shrinking clamps
  // widthRef to MIN_WIDTH which then becomes the new ratio basis on
  // expand — so widening doesn't return to the original size. With a
  // stable intendedRatio, shrink → expand round-trips back exactly.
  const intendedRatioRef = useRef<number>(DEFAULT_WIDTH / Math.max(1, computeSideArea(window.innerWidth)))
  const persistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const [dragging, setDragging] = useState(false)

  useEffect(() => {
    sidebarWidth.value = widthRef.current
    try { setVersion(bridge.getVersion()) } catch { /* not available */ }

    // Apply intendedRatio against the current side area. Always called on
    // every window resize, plus once after layout hydration so the panel
    // adapts to the actual viewport at launch (the host does not fire
    // 'resize' on initial paint, so without this manual call a session
    // saved on a maximised window would render at the saved px on a
    // smaller windowed reopen).
    const onWinResize = (): void => {
      const newW = window.innerWidth
      if (newW === 0) return
      const newSideArea = computeSideArea(newW)
      const cap = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, newSideArea - MIN_WIDTH))
      const next = Math.max(MIN_WIDTH, Math.min(cap, Math.round(intendedRatioRef.current * newSideArea)))
      if (next === widthRef.current) return
      widthRef.current = next
      setWidth(next)
      sidebarWidth.value = next
    }

    bridge.getLayout().then((l) => {
      let ratio: number | null = null
      if (typeof l?.sidebarWidthRatio === 'number' && l.sidebarWidthRatio > 0 && l.sidebarWidthRatio < 1) {
        ratio = l.sidebarWidthRatio
      } else if (typeof l?.sidebarWidth === 'number' && l.sidebarWidth >= MIN_WIDTH && l.sidebarWidth <= MAX_WIDTH) {
        // Legacy migration: old builds stored absolute px. Convert to a
        // ratio relative to the *current* sideArea so subsequent resizes
        // scale proportionally; new writes will replace the legacy field.
        ratio = l.sidebarWidth / Math.max(1, computeSideArea(window.innerWidth))
      }
      if (ratio !== null) {
        // Hard sanity clamp: a stored ratio above ~0.6 of the side area
        // means the sidebar would dominate at any window size — almost
        // certainly poisoned data from a pre-fix auto-resize tick. Cap.
        intendedRatioRef.current = Math.min(0.6, Math.max(0.05, ratio))
      }
      onWinResize()
    }).catch(() => { onWinResize() })

    window.addEventListener('resize', onWinResize)
    return () => window.removeEventListener('resize', onWinResize)
  }, [])

  function applyAndPersist(w: number): void {
    sidebarWidth.value = w
    if (persistTimerRef.current) clearTimeout(persistTimerRef.current)
    persistTimerRef.current = setTimeout(() => {
      // Persist BOTH the absolute width (legacy field, still readable by
      // older builds) and the ratio (canonical — survives window-size
      // changes between sessions). Loaders prefer ratio.
      const ratio = intendedRatioRef.current
      try { bridge.setLayout({ sidebarWidth: w, sidebarWidthRatio: ratio }) } catch {}
    }, 250)
  }

  const handleMouseDown = (e: MouseEvent): void => {
    e.preventDefault()
    e.stopPropagation()
    setDragging(true)

    const sidebarEl = (e.currentTarget as HTMLElement).parentElement
    const leftX = sidebarEl ? sidebarEl.getBoundingClientRect().left : 0

    const onMove = (ev: MouseEvent) => {
      const raw = ev.clientX - leftX
      const next = Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, raw))
      if (next === widthRef.current) return
      widthRef.current = next
      // User-driven resize re-anchors the intended ratio so subsequent
      // window resizes preserve THIS proportion, not the load-time one.
      intendedRatioRef.current = next / Math.max(1, computeSideArea(window.innerWidth))
      setWidth(next)
      applyAndPersist(next)
    }
    const onUp = () => {
      setDragging(false)
      window.removeEventListener('mousemove', onMove, true)
      window.removeEventListener('mouseup', onUp, true)
      document.body.style.userSelect = ''
      document.body.style.cursor = ''
    }

    window.addEventListener('mousemove', onMove, true)
    window.addEventListener('mouseup', onUp, true)
    document.body.style.userSelect = 'none'
    document.body.style.cursor = 'col-resize'
  }

  // When the user collapses the sidebar via the titlebar toggle we
  // render nothing — but we still expose the signal value so the
  // Titlebar's left cluster can shrink to zero in lockstep.
  // Exception: Customize mode always shows the sidebar (it IS the
  // settings nav), so the user's hide preference is overridden while
  // they're in customize and restored when they leave.
  const effectivelyVisible = sidebarVisible.value || sidebarMode.value === 'customize'
  if (!effectivelyVisible) return <div style="display:none" data-sidebar-collapsed="1" />

  return (
    <div
      class={`sidebar ${styles.sidebar}`}
      style={`width:${width}px;min-width:${MIN_WIDTH}px;max-width:${MAX_WIDTH}px;flex-shrink:0`}
    >
      {mode === 'sessions' ? <SessionsMode /> : <CustomizeMode />}
      {/* Version chip moved to the titlebar (TitlebarVersion). */}
      <div
        class={`${styles.resizeHandle} ${dragging ? styles.resizeHandleActive : ''}`}
        data-tooltip="Drag to resize"
        onMouseDown={handleMouseDown}
      >
        <span class={styles.resizeHandleBar} aria-hidden="true" />
      </div>
    </div>
  )
}

function VersionFooter({ version, bridge }: { version: string; bridge: ReturnType<typeof useBridge> }): JSX.Element | null {
  if (!version) return null
  const info = updateInfo.value
  if (!info) {
    return (
      <div style="padding:6px 12px;font-size:10px;color:var(--text-disabled);text-align:center;flex-shrink:0">
        v{version}
      </div>
    )
  }
  const busy = info.state === 'downloading'
  const ready = info.state === 'ready'
  const errored = info.state === 'error'
  const label = ready
    ? `Restart to install ${info.serverVersion}`
    : busy
      ? `Downloading ${info.serverVersion}…`
      : errored
        ? `Update failed — retry ${info.serverVersion}`
        : `Update to ${info.serverVersion}`
  const bg = ready ? 'var(--accent-green)' : errored ? 'var(--accent-red)' : 'var(--accent-primary)'
  async function onClick(): Promise<void> {
    if (ready) {
      await bridge.quitAndInstallUpdate()
    } else if (!busy) {
      await bridge.applyUpdate()
    }
  }
  return (
    <div style="padding:6px 12px;flex-shrink:0">
      <button
        type="button"
        onClick={onClick}
        disabled={busy}
        data-tooltip={errored && info.error ? info.error : undefined}
        style={`width:100%;padding:6px 10px;border:none;border-radius:var(--radius-sm);background:${bg};color:var(--bg-primary);font-size:11px;font-weight:600;cursor:${busy ? 'default' : 'pointer'};display:flex;align-items:center;justify-content:center;gap:6px;opacity:${busy ? 0.7 : 1}`}
      >
        <i class={ready ? 'fas fa-rotate-right' : busy ? 'fas fa-spinner fa-spin' : errored ? 'fas fa-triangle-exclamation' : 'fas fa-arrow-up'} style="font-size:10px" />
        {label}
      </button>
      <div style="margin-top:4px;text-align:center;font-size:9px;color:var(--text-disabled)">
        current v{version}
      </div>
    </div>
  )
}
