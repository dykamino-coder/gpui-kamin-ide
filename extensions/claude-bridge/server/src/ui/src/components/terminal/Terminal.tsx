import { useEffect, useRef, useState } from 'preact/hooks'
import '@xterm/xterm/css/xterm.css'
import styles from './Terminal.module.css'

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
  const intentionalCloseRef = useRef(false)

  function connect() {
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }

    setStatus('connecting')
    setErrorMsg('')

    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${proto}//${location.host}/ws/terminal`)
    wsRef.current = ws

    ws.onopen = () => {
      setStatus('connected')
      if (termRef.current && fitRef.current) {
        fitRef.current.fit()
        ws.send(JSON.stringify({
          type: 'resize',
          cols: termRef.current.cols,
          rows: termRef.current.rows,
        }))
        // Clear shell banner, then run pending command if any
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
      if (suppressOutputRef.current) { writeBuf = ''; return }
      if (writeBuf && termRef.current) {
        termRef.current.write(writeBuf)
        writeBuf = ''
      }
    }

    ws.onmessage = (evt) => {
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
      if (!intentionalCloseRef.current) setStatus('disconnected')
      intentionalCloseRef.current = false
    }

    ws.onerror = () => {
      clearInterval(refreshInterval)
      setStatus('error')
      setErrorMsg('WebSocket connection failed')
    }
  }

  useEffect(() => {
    let disposed = false

    async function init() {
      const { Terminal } = await import('@xterm/xterm')
      const { FitAddon } = await import('@xterm/addon-fit')

      if (disposed || !containerRef.current) return

      const term = new Terminal({
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
      term.open(containerRef.current)
      fitAddon.fit()

      termRef.current = term
      fitRef.current = fitAddon

      // ── Clipboard: paste via native paste event on xterm's textarea ──
      // This works on all contexts (HTTP/HTTPS) — no permissions needed.
      // xterm has its own paste listener that calls stopPropagation,
      // so we must intercept in capture phase with stopImmediatePropagation.
      const xtermTextarea = containerRef.current.querySelector('textarea')
      if (xtermTextarea) {
        xtermTextarea.addEventListener('paste', (e) => {
          e.preventDefault()
          e.stopImmediatePropagation()
          const text = (e as ClipboardEvent).clipboardData?.getData('text/plain')
          if (text) {
            const ws = wsRef.current
            if (ws && ws.readyState === WebSocket.OPEN) {
              ws.send(JSON.stringify({ type: 'input', data: text }))
            }
          }
        }, true) // capture phase — runs before xterm's handler
      }

      // ── Clipboard: copy via hidden textarea fallback ──
      function copyText(text: string) {
        // Try modern API first
        if (navigator.clipboard?.writeText) {
          navigator.clipboard.writeText(text).catch(() => fallbackCopy(text))
        } else {
          fallbackCopy(text)
        }
      }
      function fallbackCopy(text: string) {
        const ta = document.createElement('textarea')
        ta.value = text
        ta.style.cssText = 'position:fixed;left:-9999px;top:-9999px'
        document.body.appendChild(ta)
        ta.select()
        document.execCommand('copy')
        document.body.removeChild(ta)
      }

      // ── Key handler: Ctrl+C/X/V ──
      term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
        if (ev.type !== 'keydown') return true

        // Ctrl+C / Ctrl+Shift+C: copy selection (or SIGINT if no selection)
        if (ev.ctrlKey && (ev.key === 'c' || ev.key === 'C')) {
          const sel = term.getSelection()
          if (sel) {
            copyText(sel)
            term.clearSelection()
            return false
          }
          // No selection: Ctrl+C → SIGINT, Ctrl+Shift+C → ignore
          return ev.key === 'c'
        }

        // Ctrl+X: copy selection
        if (ev.ctrlKey && ev.key === 'x') {
          const sel = term.getSelection()
          if (sel) {
            copyText(sel)
            term.clearSelection()
            return false
          }
          return true
        }

        // Ctrl+V / Ctrl+Shift+V: stop xterm from handling (it sends raw \x16),
        // let the browser trigger native paste event on the focused textarea.
        // Our capture-phase paste listener above will intercept and send to PTY.
        if (ev.ctrlKey && (ev.key === 'v' || ev.key === 'V')) {
          // Try clipboard API as primary (works in secure contexts)
          if (navigator.clipboard?.readText) {
            navigator.clipboard.readText().then((text) => {
              if (text) {
                const ws = wsRef.current
                if (ws && ws.readyState === WebSocket.OPEN) {
                  ws.send(JSON.stringify({ type: 'input', data: text }))
                }
              }
            }).catch(() => {})
          }
          // return false → xterm skips processing, browser fires native paste event
          // on the textarea → our capture-phase paste handler sends to PTY (HTTP fallback)
          return false
        }

        return true
      })

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
              ws.send(JSON.stringify({
                type: 'resize',
                cols: term.cols,
                rows: term.rows,
              }))
            }
          }
        }, 150)
      })
      resizeObs.observe(containerRef.current)

      return () => {
        resizeObs.disconnect()
      }
    }

    let cleanupResize: (() => void) | undefined
    init().then((fn) => { cleanupResize = fn })

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
    // Always kill old session and start fresh (like Exit + run)
    intentionalCloseRef.current = true
    wsRef.current?.close()
    wsRef.current = null
    termRef.current?.reset()
    setHasOutput(false)
    pendingCmdRef.current = cmd
    setStatus('connecting')
    connect()
  }

  const quickCommands = [
    { label: 'Claude', icon: 'fa-solid fa-rocket', cmd: 'claude --dangerously-skip-permissions', color: 'var(--accent-blue)' },
    { label: 'Resume', icon: 'fa-solid fa-rotate-left', cmd: 'claude --dangerously-skip-permissions --resume', color: 'var(--accent-purple)' },
    { label: 'Auth Status', icon: 'fa-solid fa-circle-check', cmd: 'claude --dangerously-skip-permissions auth status', color: 'var(--accent-green)' },
    { label: 'Usage', icon: 'fa-solid fa-chart-pie', cmd: 'claude --dangerously-skip-permissions /usage', color: 'var(--accent-yellow)' },
    { label: 'Login', icon: 'fa-solid fa-right-to-bracket', cmd: 'claude --dangerously-skip-permissions /login', color: 'var(--accent-cyan)' },
    { label: 'Logout', icon: 'fa-solid fa-right-from-bracket', cmd: 'claude --dangerously-skip-permissions /logout', color: 'var(--accent-red)' },
  ]

  function sendExit() {
    // Kill terminal session — close WS which kills PTY on server
    intentionalCloseRef.current = true
    pendingCmdRef.current = null
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
          <span class={styles.statusDot} data-status={status === 'connected' ? 'connected' : status === 'idle' ? 'idle' : status === 'connecting' ? undefined : 'disconnected'} />
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

      {(status === 'idle' || status === 'connected') && (
        <div class={styles.quickActions}>
          {quickCommands.map((qc) => (
            <button
              key={qc.label}
              class={styles.quickBtn}
              onClick={() => sendCommand(qc.cmd)}
              title={qc.cmd}
            >
              <i class={qc.icon} style={{ color: qc.color }} />
              {qc.label}
            </button>
          ))}
          <button
            class={styles.quickBtn}
            onClick={sendExit}
            title="Ctrl+C x2 — kill current process"
          >
            <i class="fa-solid fa-xmark" style={{ color: 'var(--text-muted)' }} />
            Exit
          </button>
        </div>
      )}

      <div class={styles.terminalWrapper} ref={containerRef} />

      {(status === 'idle' || (status === 'connected' && !hasOutput)) && (
        <div class={styles.placeholder}>
          <pre class={styles.asciiBanner}>{`██████╗██╗      █████╗ ██╗   ██╗██████╗ ███████╗     ██████╗ ██████╗ ██████╗ ███████╗     ██████╗██╗     ██╗
██╔════╝██║     ██╔══██╗██║   ██║██╔══██╗██╔════╝    ██╔════╝██╔═══██╗██╔══██╗██╔════╝    ██╔════╝██║     ██║
██║     ██║     ███████║██║   ██║██║  ██║█████╗      ██║     ██║   ██║██║  ██║█████╗      ██║     ██║     ██║
██║     ██║     ██╔══██║██║   ██║██║  ██║██╔══╝      ██║     ██║   ██║██║  ██║██╔══╝      ██║     ██║     ██║
╚██████╗███████╗██║  ██║╚██████╔╝██████╔╝███████╗    ╚██████╗╚██████╔╝██████╔╝███████╗    ╚██████╗███████╗██║
 ╚═════╝╚══════╝╚═╝  ╚═╝ ╚═════╝ ╚═════╝ ╚══════╝     ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝     ╚═════╝╚══════╝╚═╝`}</pre>
          <div class={styles.placeholderHints}>
            <span>Use the quick actions above to get started</span>
          </div>
          <div class={styles.placeholderShortcuts}>
            <div class={styles.shortcutRow}><kbd>Ctrl+C</kbd> Interrupt / Exit</div>
            <div class={styles.shortcutRow}><kbd>Ctrl+V</kbd> Paste</div>
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
