import { useEffect } from 'preact/hooks'
import type { Terminal } from '@xterm/xterm'
import type { FitAddon } from '@xterm/addon-fit'
import { useBridge } from './useBridge'

interface UseTerminalResizeArgs {
  containerRef: { current: HTMLDivElement | null }
  terminalRef: { current: Terminal | null }
  fitAddonRef: { current: FitAddon | null }
  tabId: string
}

export function useTerminalResize({
  containerRef,
  terminalRef,
  fitAddonRef,
  tabId,
}: UseTerminalResizeArgs): void {
  const bridge = useBridge()

  useEffect(() => {
    let resizeTimer: ReturnType<typeof setTimeout> | null = null

    // A hidden container (display:none, e.g. an inactive tab or a TerminalPanel
    // that isn't the active tool) reports 0×0. Fitting to that clamps xterm to
    // ~2 cols and — critically — sends `resize(tabId, 2, rows)` to the PTY,
    // which fights the VISIBLE terminal's wide resize on the SAME tabId (two
    // iframes render the same session's terminal). The CLI then oscillates
    // between the two widths. Skip fit+resize whenever the container is hidden.
    const isVisible = (): boolean => {
      const c = containerRef.current
      return !!c && c.clientWidth > 0 && c.clientHeight > 0
    }

    const observer = new ResizeObserver(() => {
      if (resizeTimer) clearTimeout(resizeTimer)
      resizeTimer = setTimeout(() => {
        resizeTimer = null
        const terminal = terminalRef.current
        const fitAddon = fitAddonRef.current
        if (!terminal || !fitAddon || !isVisible()) return
        fitAddon.fit()
        bridge.resize(tabId, terminal.cols, terminal.rows)
      }, 150)
    })

    if (containerRef.current) {
      observer.observe(containerRef.current)
    }

    // Periodic SIGWINCH to fix TUI corruption (every 5s, lightweight)
    const sigwinchInterval = setInterval(() => {
      const terminal = terminalRef.current
      if (terminal && isVisible()) {
        bridge.resize(tabId, terminal.cols, terminal.rows)
      }
    }, 5000)

    // Redraw nudges. A resumed / attached session does NOT replay raw PTY output
    // (only JSONL replays), so a freshly-mounted terminal stays blank until the
    // CLI next paints — e.g. opening the Bridge Console on an idle session, or a
    // pinned console surviving a reconnect. Jiggle the PTY size (cols-1 → cols)
    // to force a SIGWINCH; the CLI repaints its CURRENT screen into our terminal.
    // Same-size resizes are a server-side no-op, so the size must actually change.
    // A few spaced attempts so at least one lands AFTER the session connects
    // (the terminal often mounts while the tab is still "connecting").
    const nudge = (): void => {
      const terminal = terminalRef.current
      if (!terminal || !isVisible() || terminal.cols <= 2) return
      const { cols, rows } = terminal
      bridge.resize(tabId, cols - 1, rows)
      setTimeout(() => bridge.resize(tabId, cols, rows), 150)
    }
    // The 9000ms attempt covers a reconnect's fresh CLI cold-spawn (the PTY is
    // respawned + `--resume`d, which can take several seconds to first paint) so
    // the repaint lands BEFORE the "Connecting…" overlay's fallback (10s) gives
    // up into an empty console.
    const nudgeTimers = [700, 3000, 6000, 9000].map(d => setTimeout(nudge, d))

    return () => {
      observer.disconnect()
      clearInterval(sigwinchInterval)
      nudgeTimers.forEach(clearTimeout)
      if (resizeTimer) clearTimeout(resizeTimer)
    }
  }, [tabId])
}
