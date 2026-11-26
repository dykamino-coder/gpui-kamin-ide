import type { JSX } from 'preact'
import { useEffect } from 'preact/hooks'
import { useTerminal } from '../../hooks/useTerminal'
import { useTerminalResize } from '../../hooks/useTerminalResize'
import styles from './TerminalPanel.module.css'
import { tabs } from '../../signals/tabs'
import { SESSION_LIMIT_NOTICE, waitingForSessionSlot } from '../../lib/session-admission'
import { LoadingCue } from '../common/LoadingCue'

interface TerminalContainerProps {
  tabId: string
  active: boolean
  /** Inset the xterm host like the native terminal `.session`; FitAddon
   *  includes the normal-screen scrollbar inside these symmetric bounds. */
  fill?: boolean
}

export function TerminalContainer({ tabId, active, fill = false }: TerminalContainerProps): JSX.Element {
  const { containerRef, terminalRef, fitAddonRef, hasOutput } = useTerminal(tabId)

  const tab = tabs.value.find(item => item.id === tabId)
  const atSessionLimit = waitingForSessionSlot(tab)
  const waitingForOutput = tab?.engine === 'codex' && tab.status === 'connected'

  useTerminalResize({ containerRef, terminalRef, fitAddonRef, tabId })

  // Repaint immediately when this terminal becomes active (session switch). While
  // display:none, xterm skips rendering and its size is stale; without this the
  // terminal shows blank/old until the resize-observer's 150ms debounce catches
  // up — the "console doesn't show right away on switch-back" lag. Fit + refresh
  // + scroll-to-bottom on the next frame paints the current screen at once.
  useEffect(() => {
    if (!active) return
    const raf = requestAnimationFrame(() => {
      const term = terminalRef.current
      const fit = fitAddonRef.current
      if (!term || !fit || !containerRef.current || containerRef.current.clientWidth === 0) return
      try {
        fit.fit()
        term.refresh(0, term.rows - 1)
        term.scrollToBottom()
      } catch { /* terminal disposed mid-switch */ }
    })
    return () => cancelAnimationFrame(raf)
  }, [active])

  return (
    <div
      class={styles.terminalArea}
      style={active ? undefined : 'display:none'}
    >
      <div
        ref={containerRef as any}
        class={styles.xtermContainer}
        style={fill ? 'inset:8px 14px 10px 14px' : undefined}
      />
      {(atSessionLimit || !hasOutput) && (
        <div class={styles.connecting} role="status">
          {atSessionLimit ? (
            <><strong>Session limit reached</strong><span>{SESSION_LIMIT_NOTICE}</span></>
          ) : waitingForOutput ? (
            <span>Codex is connected. Waiting for console output.</span>
          ) : <LoadingCue compact title="Connecting to session…" />}
        </div>
      )}
    </div>
  )
}
