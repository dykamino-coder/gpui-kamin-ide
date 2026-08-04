import { storage } from "@bridge/storage"
import type { JSX } from 'preact'
import { useRef, useState, useCallback } from 'preact/hooks'
import { activeTabId, tabs } from '../../signals/tabs'
import { TerminalContainer } from './TerminalContainer'
import { TerminalResizeHandle } from './TerminalResizeHandle'
import styles from './TerminalPanel.module.css'

const STORAGE_KEY = 'terminal-panel-width'
const DEFAULT_WIDTH = 500
const MIN_WIDTH = 200

function readSavedWidth(): number {
  const raw = storage.getItem(STORAGE_KEY)
  if (!raw) return DEFAULT_WIDTH
  const n = Number(raw)
  return Number.isFinite(n) && n >= MIN_WIDTH ? n : DEFAULT_WIDTH
}

interface TerminalPanelProps {
  visible: boolean
  showResizeHandle?: boolean
  /** Fill the parent edge-to-edge (Bridge Console tool): full width, no 800px
   *  cap, no internal "CONSOLE" header, transparent panel so the host's editor-
   *  coloured backdrop shows through. */
  fill?: boolean
}

export function TerminalPanel({ visible, showResizeHandle = true, fill = false }: TerminalPanelProps): JSX.Element {
  const panelRef = useRef<HTMLDivElement | null>(null)
  const currentTabId = activeTabId.value
  const allTabs = tabs.value
  // Width lives in state so it survives re-renders. The previous version
  // wrote `panel.style.width` imperatively but ALSO re-applied
  // `style="width:500px"` from JSX every render — any signal update mid-
  // drag (active tab title, plan, etc.) reset the width back to the
  // default and stopped propagating size changes to xterm.
  const [width, setWidth] = useState<number>(readSavedWidth)

  const handleResize = useCallback((delta: number) => {
    const panel = panelRef.current
    if (!panel) return
    const parentRect = panel.parentElement?.getBoundingClientRect()
    if (!parentRect) return
    const currentWidth = panel.getBoundingClientRect().width
    const next = currentWidth - delta
    const maxWidth = parentRect.width * 0.6
    if (next >= MIN_WIDTH && next <= maxWidth) {
      setWidth(next)
      storage.setItem(STORAGE_KEY, String(next))
    }
  }, [])

  const flexCenter = visible && !showResizeHandle
  const panelStyle = fill
    ? 'width:100%;flex:1;max-width:none;margin:0;background:transparent'
    : flexCenter
      ? 'width:100%;max-width:800px;flex:1;margin:0 auto'
      : `width:${width}px`

  return (
    <>
      {visible && showResizeHandle && <TerminalResizeHandle onResize={handleResize} />}
      <div
        ref={panelRef}
        class={`${styles.panel} ${visible ? styles.visible : ''}`}
        style={panelStyle}
      >
        {allTabs.map(tab => (
          <TerminalContainer
            key={tab.id}
            tabId={tab.id}
            active={tab.id === currentTabId}
            fill={fill}
          />
        ))}
      </div>
    </>
  )
}

export { TerminalResizeHandle }
