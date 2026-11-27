import type { JSX } from 'preact'
import { useEffect } from 'preact/hooks'
import { useTerminal } from '../../hooks/useTerminal'
import { useTerminalResize } from '../../hooks/useTerminalResize'
import styles from './TerminalPanel.module.css'
import { activeTabId, tabs } from '../../signals/tabs'
import { snd, sub } from '../../lib/bridge-transport'
import { isBrowserHost } from '../../lib/webview-api'
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

  useEffect(() => {
    // Only the standalone native Console hosts the explicit permission action.
    if (!fill || !active || isBrowserHost() || tab?.engine !== 'claude' || !tab.sessionId || !terminalRef.current) return
    const sessionId = tab.sessionId
    const current = (id: unknown, runtime: unknown) => {
      const live = tabs.value.find(t => t.id === id)
      return id === tabId && runtime === sessionId && activeTabId.value === tabId && live?.engine === 'claude' && live.sessionId === runtime
    }
    const ready = (id: unknown, runtime: unknown) => { if (current(id, runtime)) snd('permission-console:ready', tabId, sessionId) }
    const offered = sub('permission-console:requested', ready)
    const focus = sub('permission-console:focus', (id, runtime) => { if (current(id, runtime)) terminalRef.current?.focus() })
    ready(tabId, sessionId)
    return () => { offered(); focus() }
  }, [active, fill, tabId, tab?.engine, tab?.sessionId])

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
