import type { JSX } from 'preact'
import { useEffect, useRef, useState } from 'preact/hooks'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import '@xterm/xterm/css/xterm.css'
import { useBridge } from '../../hooks/useBridge'
import { resolvedTheme } from '../../theme/apply-theme'

// FilePanel terminal palette is *always* dark — same Catppuccin-mocha
// values whether the app theme is dark or light. We pin them here
// (rather than reading --term-* CSS vars via `buildTerminalTheme`)
// because in light mode those tokens flip to deep inks meant for
// readability on cream paper, which would tint the cell-grid text
// dark on dark and make the terminal unreadable. Only `background`
// follows the active theme — see `resolveBg()` in the component.
const DARK_TERMINAL_PALETTE: Record<string, string> = {
  foreground: '#cfd4e2',
  cursor: '#f5e0dc',
  selectionBackground: '#515567',
  selectionForeground: '#cfd4e2',
  black: '#515567',
  red: '#f38ba8',
  green: '#a6e3a1',
  yellow: '#f9e2af',
  blue: '#89b4fa',
  magenta: '#f5c2e7',
  cyan: '#94e2d5',
  white: '#afb6ca',
  brightBlack: '#60667b',
  brightRed: '#f38ba8',
  brightGreen: '#a6e3a1',
  brightYellow: '#f9e2af',
  brightBlue: '#89b4fa',
  brightMagenta: '#f5c2e7',
  brightCyan: '#94e2d5',
  brightWhite: '#adb3c7',
}

interface ShellProfile {
  id: string
  label: string
  command: string
  args: string[]
  icon?: string
}

interface FilePanelTerminalProps {
  /** Tab this terminal belongs to. One xterm + PTY are kept alive per
   *  tabId; switching tabs only flips visibility, never tears them down. */
  tabId: string
  /** Initial working directory passed to the first `pty.spawn`. Once the
   *  shell is running it owns its own cwd, so further tab cwd changes
   *  are ignored — the user can `cd` manually. */
  cwd: string
  /** Whether this is the currently active tab's terminal. Hidden via
   *  display:none when false; xterm + PTY stay alive in the background. */
  visible: boolean
}

/** Interactive PTY terminal for the FilePanel bottom pane. Uses xterm.js
 *  on the renderer side and node-pty in the main process. The dropdown
 *  exposes every shell `discoverShells()` returns (Windows: cmd / Power-
 *  Shell / pwsh / Git Bash / each WSL distro; POSIX: /etc/shells + $SHELL).
 *
 *  One instance per tab — the parent renders all of them simultaneously
 *  and toggles visibility, so each chat tab gets its own persistent
 *  terminal that never resets when switching away. */
export function FilePanelTerminal({ tabId, cwd, visible }: FilePanelTerminalProps): JSX.Element {
  void tabId // tabId is the React key — only used for instance identity
  const bridge = useBridge()
  const containerRef = useRef<HTMLDivElement | null>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddon | null>(null)
  const ptyIdRef = useRef<string | null>(null)
  const [shells, setShells] = useState<ShellProfile[]>([])
  const [activeShellId, setActiveShellId] = useState<string | null>(null)
  const [menuOpen, setMenuOpen] = useState(false)
  // Lazy-mount: an inactive tab's terminal pays no cost (no xterm DOM
  // tree, no PTY spawn, no shell-list IPC). Once the user views this
  // tab the first time we mount; thereafter we keep the instance even
  // while the tab is hidden so the user's scrollback / input survives.
  const [mounted, setMounted] = useState<boolean>(visible)
  useEffect(() => {
    if (visible && !mounted) setMounted(true)
  }, [visible, mounted])

  // Lazy-load shell list once — only after first mount.
  useEffect(() => {
    if (!mounted) return
    bridge.shellsList().then((list) => {
      setShells(list)
      // Default = first list entry; user can switch via the dropdown.
      if (list.length > 0 && !activeShellId) setActiveShellId(list[0]!.id)
    }).catch(() => { /* no shells available */ })
  }, [mounted])

  // Initialise xterm once, attach data pumps, kick off the first PTY
  // when both shell list + cwd are known.
  useEffect(() => {
    if (!mounted) return
    if (!containerRef.current) return
    if (termRef.current) return
    // Reuse the project's canonical xterm theme so ANSI colours match
    // the main session terminal exactly; override just `background` per
    // the active app theme — Windows-Terminal-dark `#0b0c0f` on dark,
    // a softer `#262626` on light so the contrast vs. the surrounding
    // cream/paper layout doesn't read as a black hole. The wrapper
    // padding bg uses --term-pane-bg which already follows the theme;
    // xterm's own canvas/webgl renderer ignores parent CSS, so we
    // re-poke `term.options.theme` on every theme change below.
    const resolveBg = (): string => resolvedTheme.value === 'light' ? '#262626' : '#0b0c0f'
    const term = new Terminal({
      fontFamily: "ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace",
      fontSize: 12,
      theme: { ...DARK_TERMINAL_PALETTE, background: resolveBg() },
      cursorBlink: true,
      scrollback: 5000,
    })
    const fit = new FitAddon()
    term.loadAddon(fit)
    term.open(containerRef.current)
    try { fit.fit() } catch { /* container not laid out yet */ }
    termRef.current = term
    fitRef.current = fit

    // Outbound: keystrokes → PTY.
    term.onData((data: string) => {
      if (ptyIdRef.current) bridge.ptyWrite(ptyIdRef.current, data)
    })

    // Windows-Terminal-style Ctrl+C / Ctrl+V — copy when there's a
    // selection, else fall through to the default xterm behaviour (which
    // emits ^C as input, i.e. SIGINT for the shell). Ctrl+V always pastes
    // from the clipboard, matching cmd / PowerShell expectations on Win11.
    function copyText(text: string): void {
      if (navigator.clipboard?.writeText) {
        navigator.clipboard.writeText(text).catch(() => fallbackCopy(text))
      } else {
        fallbackCopy(text)
      }
    }
    function fallbackCopy(text: string): void {
      const ta = document.createElement('textarea')
      ta.value = text
      ta.style.cssText = 'position:fixed;left:-9999px;top:-9999px'
      document.body.appendChild(ta)
      ta.select()
      document.execCommand('copy')
      document.body.removeChild(ta)
    }
    async function pasteFromClipboard(): Promise<void> {
      try {
        const text = await navigator.clipboard.readText()
        if (text && ptyIdRef.current) bridge.ptyWrite(ptyIdRef.current, text)
      } catch { /* clipboard API denied */ }
    }
    term.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== 'keydown') return true
      // Ctrl+Shift+C → always copy selection (Windows Terminal default).
      if (ev.ctrlKey && ev.shiftKey && ev.code === 'KeyC') {
        const sel = term.getSelection()
        if (sel) { copyText(sel); term.clearSelection() }
        return false
      }
      // Ctrl+C → copy if there's a selection, else send ^C (SIGINT).
      if (ev.ctrlKey && !ev.shiftKey && ev.code === 'KeyC') {
        const sel = term.getSelection()
        if (sel) {
          copyText(sel)
          term.clearSelection()
          return false
        }
        return true
      }
      // Ctrl+V (and Ctrl+Shift+V) → paste from clipboard. Block xterm so
      // it doesn't also emit \x16 as input.
      if (ev.ctrlKey && ev.code === 'KeyV') {
        pasteFromClipboard()
        return false
      }
      return true
    })

    // Native paste event on xterm's textarea — covers OS-native paste
    // gestures (middle-click on Linux, paste from menu) that don't go
    // through our key handler above.
    const xtermTextarea = containerRef.current.querySelector('textarea')
    if (xtermTextarea) {
      xtermTextarea.addEventListener('paste', (e) => {
        e.preventDefault()
        e.stopImmediatePropagation()
        const text = (e as ClipboardEvent).clipboardData?.getData('text/plain')
        if (text && ptyIdRef.current) bridge.ptyWrite(ptyIdRef.current, text)
        else pasteFromClipboard().catch(() => {})
      }, true)
    }
    term.onResize(({ cols, rows }) => {
      if (ptyIdRef.current) bridge.ptyResize(ptyIdRef.current, cols, rows)
    })

    // Inbound: PTY data → xterm. Filter by ptyId so a stale spawn from
    // a previous shell doesn't write into the current terminal.
    const offData = bridge.onPtyData(({ ptyId, data }) => {
      if (ptyId !== ptyIdRef.current) return
      term.write(data)
    })
    const offExit = bridge.onPtyExit(({ ptyId }) => {
      if (ptyId !== ptyIdRef.current) return
      ptyIdRef.current = null
      term.write('\r\n\x1b[2m[shell exited]\x1b[0m\r\n')
    })

    // Re-fit when the panel resizes — RO observes the host element.
    const ro = new ResizeObserver(() => {
      try { fit.fit() } catch { /* ignore */ }
    })
    ro.observe(containerRef.current)

    // Live theme tracking — xterm's renderers (webgl/canvas/dom) cache
    // the background and ignore CSS variables, so we re-assign
    // `term.options.theme` on every resolvedTheme.value flip. signals'
    // `subscribe` returns an unsubscribe function we call on cleanup.
    const unsubTheme = resolvedTheme.subscribe(() => {
      try {
        term.options.theme = { ...DARK_TERMINAL_PALETTE, background: resolveBg() }
      } catch { /* terminal already disposed */ }
    })

    return () => {
      offData()
      offExit()
      unsubTheme()
      ro.disconnect()
      if (ptyIdRef.current) bridge.ptyKill(ptyIdRef.current)
      ptyIdRef.current = null
      term.dispose()
      termRef.current = null
      fitRef.current = null
    }
  }, [mounted])

  // (Re)spawn the PTY only when the user picks a different shell — NOT
  // when the active project tab changes. Switching chat tabs should
  // leave the terminal state intact (history, current cwd, partial
  // command). cwd is read fresh at spawn time as the *initial* directory
  // only; once the shell is running it owns its own cwd via `cd`.
  useEffect(() => {
    if (!termRef.current || !activeShellId) return
    const term = termRef.current
    const fit = fitRef.current
    if (ptyIdRef.current) {
      bridge.ptyKill(ptyIdRef.current)
      ptyIdRef.current = null
    }
    term.reset()
    try { fit?.fit() } catch { /* ignore */ }
    // Wait one frame so xterm has measured the container before we
    // ship cols/rows to the PTY — otherwise a freshly-mounted panel
    // sizes at 0×0 and the shell renders a 1-column-wide prompt that
    // looks broken until the user manually drags a divider.
    requestAnimationFrame(() => {
      try { fit?.fit() } catch { /* ignore */ }
      const cols = Math.max(20, term.cols)
      const rows = Math.max(5, term.rows)
      bridge.ptySpawn({ shellId: activeShellId, cwd, cols, rows }).then(({ ptyId }) => {
        ptyIdRef.current = ptyId
        term.focus()
      }).catch((err: unknown) => {
        term.write(`\r\n\x1b[31m[failed to spawn shell: ${String(err)}]\x1b[0m\r\n`)
      })
    })
  }, [activeShellId])

  // When this terminal becomes visible (parent flipped to its tab), the
  // xterm container has been display:none — its measured cols/rows are
  // stale. Re-fit and forward the new size to the PTY so the prompt
  // doesn't render in the old (often 0-column) geometry.
  useEffect(() => {
    if (!visible) return
    requestAnimationFrame(() => {
      try { fitRef.current?.fit() } catch { /* ignore */ }
      const term = termRef.current
      if (term && ptyIdRef.current) {
        bridge.ptyResize(ptyIdRef.current, Math.max(20, term.cols), Math.max(5, term.rows))
      }
      term?.focus()
    })
  }, [visible])

  const activeProfile = shells.find(s => s.id === activeShellId) ?? null

  // Inactive tabs that have never been visible render an empty
  // placeholder — no xterm DOM, no PTY. Once the user clicks the tab
  // for the first time, `visible` flips and `mounted` flips with it,
  // and the real terminal mounts. This prevents corp-Windows machines
  // from spawning N PowerShell instances at app start (each can take
  // 30+s under AppLocker / AV scanning).
  if (!mounted) {
    return <div style="display:none" data-pty-tab-id={tabId} />
  }

  return (
    <div style={`flex:1;min-height:0;display:${visible ? 'flex' : 'none'};flex-direction:column`}>
      <div style="flex-shrink:0;display:flex;align-items:center;gap:6px;padding:7px 4px 3px;border-bottom:1px solid var(--bg-surface);position:relative">
        <button
          type="button"
          onClick={() => setMenuOpen((v) => !v)}
          style="display:inline-flex;align-items:center;gap:6px;padding:2px 8px;border:none;border-radius:var(--radius-sm);background:transparent;color:var(--text-secondary);font-size:11px;font-weight:600;cursor:pointer"
        >
          <i class={activeProfile?.icon || 'fas fa-terminal'} style="font-size:10px;color:var(--accent-primary)" />
          <span>{activeProfile?.label || 'Shell'}</span>
          <i class="fas fa-chevron-down" style="font-size:9px;color:var(--text-muted)" />
        </button>
        {menuOpen && (
          <div
            style="position:absolute;top:100%;left:8px;z-index:50;min-width:200px;background:var(--bg-base);border:1px solid var(--bg-surface);border-radius:var(--radius-md);padding:4px;box-shadow:var(--shadow-dropdown);display:flex;flex-direction:column;gap:1px"
            onMouseLeave={() => setMenuOpen(false)}
          >
            {shells.length === 0 ? (
              <div style="padding:6px 10px;font-size:11px;color:var(--text-muted)">No shells discovered</div>
            ) : (
              shells.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => { setActiveShellId(s.id); setMenuOpen(false) }}
                  style={`display:inline-flex;align-items:center;gap:8px;padding:6px 10px;border:none;border-radius:var(--radius-sm);background:${s.id === activeShellId ? 'color-mix(in srgb, var(--accent-primary) 12%, transparent)' : 'transparent'};color:${s.id === activeShellId ? 'var(--accent-primary)' : 'var(--text-primary)'};font-size:11px;cursor:pointer;text-align:left`}
                >
                  <i class={s.icon || 'fas fa-terminal'} style="font-size:11px;width:14px;text-align:center" />
                  {s.label}
                </button>
              ))
            )}
          </div>
        )}
      </div>
      <div
        ref={containerRef}
        onClick={() => termRef.current?.focus()}
        style="flex:1;min-height:0;padding:5px;background:var(--term-pane-bg);border-radius:0 0 var(--radius-md) var(--radius-md);overflow:hidden"
      />
    </div>
  )
}
