import { useEffect, useRef, useState } from 'preact/hooks'
import '@xterm/xterm/css/xterm.css'
import { CONSOLE_OPTIONS, configureConsole, createOsc52ClipboardProvider } from '../../lib/terminal-console'
import '../../lib/terminal-console/console.css'
import styles from './Terminal.module.css'
import { api } from '../../services/api-client'
import { codexAccountEventAt } from '../../signals/server'
import { CodexAccountCard } from '../dashboard/CodexAccountCard'
import {
  quickCommands,
  terminalEngines,
  codexAccountAction,
  type TerminalEngine,
  type QuickCommand,
  type CodexTerminalCommand,
} from './quick-commands'

export function Terminal() {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<import('@xterm/xterm').Terminal | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const fitRef = useRef<import('@xterm/addon-fit').FitAddon | null>(null)
  const [status, setStatus] = useState<'idle' | 'connecting' | 'connected' | 'disconnected' | 'error'>('idle')
  const [errorMsg, setErrorMsg] = useState('')
  const [hasOutput, setHasOutput] = useState(false)
  const suppressOutputRef = useRef(false)
  const pendingCmdRef = useRef<string | null>(null)
  const launchRef = useRef<CodexTerminalCommand | null>(null)
  const [engine, setEngine] = useState<TerminalEngine>('claude')
  const [codexAvailable, setCodexAvailable] = useState(false)
  const [showAccount, setShowAccount] = useState(false)
  const [accountBusy, setAccountBusy] = useState(false)
  const [accountError, setAccountError] = useState('')
  const [usageHint, setUsageHint] = useState(false)

  useEffect(() => {
    let disposed = false
    const refresh = () =>
      api
        .getStatus()
        .then((status) => {
          if (!disposed) setCodexAvailable((status.codex as { available?: boolean } | undefined)?.available === true)
        })
        .catch(() => undefined)
    void refresh()
    const timer = setInterval(() => void refresh(), 30_000)
    return () => {
      disposed = true
      clearInterval(timer)
    }
  }, [])

  function connect() {
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }

    setStatus('connecting')
    setErrorMsg('')

    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const codexCommand = launchRef.current
    const suffix = codexCommand ? `?engine=codex&command=${codexCommand}` : ''
    const ws = new WebSocket(`${proto}//${location.host}/ws/terminal${suffix}`)
    wsRef.current = ws

    ws.onopen = () => {
      if (wsRef.current !== ws) return
      setStatus('connected')
      if (termRef.current && fitRef.current) {
        fitRef.current.fit()
        ws.send(
          JSON.stringify({
            type: 'resize',
            cols: termRef.current.cols,
            rows: termRef.current.rows,
          }),
        )
        // Clear shell banner, then run pending command if any
        if (codexCommand) return // The server launches Codex directly, without shell input.
        suppressOutputRef.current = true
        setTimeout(() => {
          if (ws.readyState !== WebSocket.OPEN) return
          ws.send(JSON.stringify({ type: 'input', data: 'cls\r' }))
          setTimeout(() => {
            termRef.current?.clear()
            suppressOutputRef.current = false
            if (ws.readyState !== WebSocket.OPEN) return
            const cmd = pendingCmdRef.current
            if (cmd) {
              pendingCmdRef.current = null
              ws.send(JSON.stringify({ type: 'input', data: cmd + '\r' }))
            }
          }, 200)
        }, 200)
      }
    }

    // Auto-refresh: SIGWINCH every 1s (same-size resize, no jitter)
    const refreshInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN && !suppressOutputRef.current) {
        ws.send(JSON.stringify({ type: 'refresh' }))
      }
    }, 1000)

    // Client-side write batching — trailing debounce so xterm gets
    // complete TUI redraws in one write() call instead of partial frames
    let writeBuf = ''
    let writeTimer = 0
    function flushWrites() {
      writeTimer = 0
      if (suppressOutputRef.current) {
        writeBuf = ''
        return
      }
      if (writeBuf && termRef.current) {
        termRef.current.write(writeBuf)
        writeBuf = ''
      }
    }

    ws.onmessage = (evt) => {
      if (wsRef.current !== ws) return
      try {
        const msg = JSON.parse(evt.data)
        if (msg.type === 'output') {
          if (!suppressOutputRef.current) setHasOutput(true)
          writeBuf += msg.data
          if (writeTimer) clearTimeout(writeTimer)
          writeTimer = window.setTimeout(flushWrites, 4)
        } else if (msg.type === 'exit') {
          termRef.current?.write(`\r\n\x1b[33m[Process exited with code ${msg.code}]\x1b[0m\r\n`)
          setStatus('disconnected')
        } else if (msg.type === 'error') {
          setErrorMsg(msg.data)
          setStatus('error')
        }
      } catch {}
    }

    ws.onclose = () => {
      clearInterval(refreshInterval)
      if (wsRef.current === ws) setStatus((current) => (current === 'error' ? 'error' : 'disconnected'))
    }

    ws.onerror = () => {
      clearInterval(refreshInterval)
      if (wsRef.current !== ws) return
      setStatus('error')
      setErrorMsg('WebSocket connection failed')
    }
  }

  useEffect(() => {
    let disposed = false

    async function init() {
      const { Terminal } = await import('@xterm/xterm')
      const { FitAddon } = await import('@xterm/addon-fit')
      const { ClipboardAddon } = await import('@xterm/addon-clipboard')

      if (disposed || !containerRef.current) return

      const term = new Terminal({
        ...CONSOLE_OPTIONS,
        cursorBlink: true,
        fontFamily: "'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace",
        fontSize: 14,
        lineHeight: 1.2,
        theme: {
          background: '#0a0c0f',
          foreground: '#e6edf3',
          cursor: '#58a6ff',
          cursorAccent: '#0a0c0f',
          selectionBackground: '#264f78',
          selectionInactiveBackground: '#264f78',
          selectionForeground: '#e6edf3',
          black: '#484f58',
          red: '#ff7b72',
          green: '#3fb950',
          yellow: '#d29922',
          blue: '#58a6ff',
          magenta: '#bc8cff',
          cyan: '#39d2c0',
          white: '#b1bac4',
          brightBlack: '#6e7681',
          brightRed: '#ffa198',
          brightGreen: '#56d364',
          brightYellow: '#e3b341',
          brightBlue: '#79c0ff',
          brightMagenta: '#d2a8ff',
          brightCyan: '#56d4dd',
          brightWhite: '#f0f6fc',
        },
      })

      const fitAddon = new FitAddon()
      term.loadAddon(fitAddon)
      term.loadAddon(new ClipboardAddon(undefined, createOsc52ClipboardProvider(containerRef.current.ownerDocument)))
      term.open(containerRef.current)
      const disposeConsole = configureConsole(term, containerRef.current)
      fitAddon.fit()

      termRef.current = term
      fitRef.current = fitAddon

      // Input → WS
      term.onData((data) => {
        suppressOutputRef.current = false
        const ws = wsRef.current
        if (ws && ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'input', data }))
        }
      })

      // Resize handling — debounced
      let resizeTimer: ReturnType<typeof setTimeout> | null = null
      const resizeObs = new ResizeObserver(() => {
        if (disposed) return
        if (resizeTimer) clearTimeout(resizeTimer)
        resizeTimer = setTimeout(() => {
          resizeTimer = null
          if (disposed) return
          const prevCols = term.cols
          const prevRows = term.rows
          fitAddon.fit()
          if (term.cols !== prevCols || term.rows !== prevRows) {
            const ws = wsRef.current
            if (ws && ws.readyState === WebSocket.OPEN) {
              ws.send(
                JSON.stringify({
                  type: 'resize',
                  cols: term.cols,
                  rows: term.rows,
                }),
              )
            }
          }
        }, 150)
      })
      resizeObs.observe(containerRef.current)

      return () => {
        if (resizeTimer) clearTimeout(resizeTimer)
        resizeObs.disconnect()
        disposeConsole()
      }
    }

    let cleanupResize: (() => void) | undefined
    init().then((fn) => {
      if (disposed) fn?.()
      else cleanupResize = fn
    })

    // Listen for external command events (e.g. from Login button on Account card)
    const handleExternalCmd = (e: Event) => {
      const cmd = (e as CustomEvent).detail?.command
      if (cmd) sendCommand(cmd)
    }
    window.addEventListener('terminal:command', handleExternalCmd)

    return () => {
      disposed = true
      cleanupResize?.()
      window.removeEventListener('terminal:command', handleExternalCmd)
      wsRef.current?.close()
      wsRef.current = null
      termRef.current?.dispose()
      termRef.current = null
      fitRef.current = null
    }
  }, [])

  function handleReconnect() {
    termRef.current?.clear()
    connect()
  }

  /** Force TUI redraw by sending SIGWINCH (resize toggle) to PTY */
  function handleRefresh() {
    const ws = wsRef.current
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'refresh' }))
    }
  }

  function sendCommand(cmd: string) {
    launchRef.current = null
    setUsageHint(false)
    // Always kill old session and start fresh (like Exit + run)
    wsRef.current?.close()
    wsRef.current = null
    termRef.current?.reset()
    setHasOutput(false)
    pendingCmdRef.current = cmd
    setStatus('connecting')
    connect()
  }

  async function runQuickCommand(command: QuickCommand) {
    if ('cmd' in command) {
      sendCommand(command.cmd)
      return
    }
    if ('account' in command) {
      setAccountBusy(true)
      setAccountError('')
      setShowAccount(true)
      try {
        await codexAccountAction(command.account, api)
      } catch (error) {
        setAccountError((error as Error).message)
      } finally {
        setAccountBusy(false)
        codexAccountEventAt.value = Date.now()
      }
      return
    }
    sendExit()
    launchRef.current = command.codex
    setUsageHint(command.codex === 'usage')
    connect()
  }

  function sendExit() {
    // Kill terminal session — close WS which kills PTY on server
    pendingCmdRef.current = null
    launchRef.current = null
    suppressOutputRef.current = false
    wsRef.current?.close()
    wsRef.current = null
    termRef.current?.reset()
    setHasOutput(false)
    setStatus('idle')
  }

  return (
    <div class={styles.container}>
      <div class={styles.toolbar}>
        <div class={styles.toolbarLeft}>
          <span
            class={styles.statusDot}
            data-status={
              status === 'connected'
                ? 'connected'
                : status === 'idle'
                  ? 'idle'
                  : status === 'connecting'
                    ? undefined
                    : 'disconnected'
            }
          />
          <span>
            {status === 'idle' && 'Terminal'}
            {status === 'connecting' && 'Connecting...'}
            {status === 'connected' && 'Terminal'}
            {status === 'disconnected' && 'Disconnected'}
            {status === 'error' && (errorMsg || 'Error')}
          </span>
        </div>
        <div class={styles.toolbarRight}>
          {status === 'connected' && (
            <button class={styles.reconnectBtn} onClick={handleRefresh} title="Force TUI redraw (SIGWINCH)">
              <i class="fa-solid fa-arrows-rotate" />
              Refresh
            </button>
          )}
          {(status === 'disconnected' || status === 'error') && (
            <button class={styles.reconnectBtn} onClick={handleReconnect}>
              <i class="fa-solid fa-rotate" />
              Reconnect
            </button>
          )}
        </div>
      </div>

      {status !== 'connecting' && (
        <div class={styles.quickActions}>
          {codexAvailable && (
            <div role="group" aria-label="Terminal engine" class={styles.engineSwitch}>
              {terminalEngines(codexAvailable).map((value) => (
                <button
                  type="button"
                  class={styles.quickBtn}
                  aria-pressed={engine === value}
                  onClick={() => setEngine(value)}
                >
                  {value === 'claude' ? 'Claude' : 'Codex'}
                </button>
              ))}
            </div>
          )}
          {quickCommands(codexAvailable ? engine : 'claude').map((qc) => (
            <button
              key={qc.label}
              class={styles.quickBtn}
              disabled={accountBusy}
              onClick={() => void runQuickCommand(qc)}
              title={
                'cmd' in qc
                  ? qc.cmd
                  : 'account' in qc
                    ? `Codex ${qc.account} via Bridge account API`
                    : `Codex ${qc.codex}`
              }
            >
              <i class={qc.icon} style={{ color: qc.color }} />
              {qc.label}
            </button>
          ))}
          <button class={styles.quickBtn} onClick={sendExit} title="Ctrl+C x2 — kill current process">
            <i class="fa-solid fa-xmark" style={{ color: 'var(--text-muted)' }} />
            Exit
          </button>
        </div>
      )}

      {usageHint && (
        <div class={styles.notice}>
          Type <code>/status</code> in the Codex TUI to see account usage and limits.
        </div>
      )}
      {showAccount && (
        <div class={styles.accountPanel}>
          <button type="button" class={styles.quickBtn} onClick={() => setShowAccount(false)}>
            Close account
          </button>
          {accountError && <p role="alert">{accountError}</p>}
          <CodexAccountCard />
        </div>
      )}

      <div class={styles.terminalWrapper} ref={containerRef} />

      {(status === 'idle' || (status === 'connected' && !hasOutput)) && (
        <div class={styles.placeholder}>
          <pre
            class={styles.asciiBanner}
          >{`██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗     ██████╗ ██████╗ ██████╗ ███████╗     ██████╗██╗     ██╗
██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝    ██╔════╝██╔═══██╗██╔══██╗██╔════╝    ██╔════╝██║     ██║
██║     ██║     ███████║██║   ██║██║  ██║█████╗      ██║     ██║   ██║██║  ██║█████╗      ██║     ██║     ██║
██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝      ██║     ██║   ██║██║  ██║██╔══╝      ██║     ██║     ██║
╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗    ╚██████╗╚██████╔╝██████╔╝███████╗    ╚██████╗███████╗██║
 ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝     ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝     ╚═════╝╚══════╝╚═╝`}</pre>
          <div class={styles.placeholderHints}>
            <span>Use the quick actions above to get started</span>
          </div>
          <div class={styles.placeholderShortcuts}>
            <div class={styles.shortcutRow}>
              <kbd>Ctrl+C</kbd> Interrupt / Exit
            </div>
            <div class={styles.shortcutRow}>
              <kbd>Ctrl+V</kbd> Paste
            </div>
          </div>
        </div>
      )}

      {status === 'connecting' && (
        <div class={styles.overlay}>
          <div class={styles.overlayMessage}>
            <i class="fa-solid fa-spinner fa-spin" />
            Connecting to terminal...
          </div>
        </div>
      )}
    </div>
  )
}
