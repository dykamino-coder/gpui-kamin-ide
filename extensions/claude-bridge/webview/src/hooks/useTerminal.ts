import { useEffect, useRef, useState } from 'preact/hooks'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { SerializeAddon } from '@xterm/addon-serialize'
import { ClipboardAddon } from '@xterm/addon-clipboard'
import { CONSOLE_OPTIONS, configureConsole, createOsc52ClipboardProvider } from '../../../server/src/ui/src/lib/terminal-console'
import '../../../server/src/ui/src/lib/terminal-console/console.css'
import { buildTerminalTheme } from '../theme/terminal-theme'
import { resolvedTheme, themeNonce } from '../theme/apply-theme'
import { useBridge } from './useBridge'
import { terminalRegistry } from '../signals/terminal-registry'
import { terminalSnapshots } from '../signals/terminal-snapshots'
import { terminalAtBottom } from '../signals/ui'
import { activeTabId } from '../signals/tabs'
import { tabPromptVisible, tabPromptReady } from '../signals/connection'
import { localQueue } from '../signals/queue'
import { enableTerminalRenderer } from './terminal-renderer'

/** True once the terminal buffer holds ANY non-blank line — used to keep the
 *  "Connecting…" overlay up until the console actually has something to show
 *  (an empty black panel reads as "frozen"). Cheap: scans only until the first
 *  non-empty line, and it's only called before the first real content lands. */
function terminalHasVisibleContent(term: Terminal): boolean {
  const buf = term.buffer.active
  const len = buf.length
  for (let i = 0; i < len; i++) {
    const line = buf.getLine(i)
    if (line && line.translateToString(true).trim() !== '') return true
  }
  return false
}

export interface TerminalRefs {
  containerRef: { current: HTMLDivElement | null }
  terminalRef: { current: Terminal | null }
  fitAddonRef: { current: FitAddon | null }
  /** False until the terminal has received its first content (a restored
   *  snapshot or the first PTY stream chunk) — drives a "Connecting…" overlay so
   *  a fresh session's attach window isn't just a blank black surface. */
  hasOutput: boolean
}

export function useTerminal(tabId: string): TerminalRefs {
  const bridge = useBridge()
  // Preact useRef with explicit null initial value — cast to avoid undefined in type
  const containerRef = useRef<HTMLDivElement>(null) as { current: HTMLDivElement | null }
  const terminalRef = useRef<Terminal>(null) as { current: Terminal | null }
  const fitAddonRef = useRef<FitAddon>(null) as { current: FitAddon | null }
  const [hasOutput, setHasOutput] = useState(false)

  useEffect(() => {
    const container = containerRef.current
    if (!container || terminalRef.current) return

    const terminal = new Terminal({
      // Match KaminIDE's native terminal (TerminalSession): --font-mono @ 13px.
      fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim()
        || "'Cascadia Code', 'Fira Code', 'JetBrains Mono', 'Consolas', monospace",
      fontSize: 13,
      lineHeight: 1.2,
      cursorBlink: true,
      cursorStyle: 'block',
      theme: buildTerminalTheme(),
      ...CONSOLE_OPTIONS,
    })

    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.loadAddon(new WebLinksAddon())
    terminal.loadAddon(new ClipboardAddon(undefined, createOsc52ClipboardProvider(container.ownerDocument, {
      writeText: text => bridge.writeClipboard(text),
    })))

    // A document-local remount cache, not durable CEF/app persistence.
    const serializeAddon = new SerializeAddon()
    terminal.loadAddon(serializeAddon)
    terminal.open(container)
    const disposeConsole = configureConsole(terminal, container, {
      writeText: text => bridge.writeClipboard(text),
      readText: () => bridge.readClipboard(),
      handleKey: (ev: KeyboardEvent): boolean => {
        // CEF keydown uses the initial layout; printable input arrives through
        // keypress with the Unicode character supplied by the native shell.
        if (ev.key.length === 1 && !ev.ctrlKey && !ev.altKey && !ev.metaKey) return false

        // Escape → block xterm from forwarding \x1b to the CLI ONLY when the
        // bridge is actually going to cancel (active turn or queued messages).
        // In that case useGlobalEscapeCancel sends \x03 + restores the local
        // queue, so we mustn't also let CLI's Esc handler run
        // `popAllCommandsFromQueue` (it would rehydrate ITS input first).
        //
        // When the CLI is IDLE with no queue, useGlobalEscapeCancel is a no-op,
        // so blocking \x1b here would swallow Esc entirely — breaking every
        // native CLI use of Escape (close a permission/model menu, dismiss the
        // `/` autocomplete, clear the prompt line). Forward it instead.
        if (ev.code === 'Escape' && !ev.ctrlKey && !ev.shiftKey && !ev.altKey && !ev.metaKey) {
          const tabId = activeTabId.value
          const queued = tabId ? (localQueue.value.get(tabId) ?? []) : []
          const isBusy = tabId ? !(tabPromptReady.value.get(tabId) ?? true) : false
          const bridgeWillCancel = !!tabId && (isBusy || queued.length > 0)
          return bridgeWillCancel ? false : true
        }

        return true
      },
    })
    fitAddon.fit()

    // Clear the "Connecting…" overlay only once the terminal has REAL visible
    // content (restored snapshot or actual PTY output). Gate on visible text —
    // NOT on any write (a bare cursor-move/clear escape carries no text) and NOT
    // on a timer: an empty black console reads as "frozen", so the spinner must
    // stay up until something actually paints (a spinning overlay looks like
    // loading; a blank panel looks hung). onWriteParsed is a read-only observer.
    const firstOutput = terminal.onWriteParsed(() => {
      if (!terminalHasVisibleContent(terminal)) return
      setHasOutput(true)
      firstOutput.dispose()
    })

    // Restore a prior mount in this document before attaching PTY output.
    // Document recreation/restart starts fresh; reconnect clears this cache.
    try {
      const stored = terminalSnapshots.get(tabId)
      // Restore application mouse modes together with the screen contents.
      if (stored) terminal.write(stored)
    } catch { /* malformed historical snapshot */ }

    const disposeRenderer = enableTerminalRenderer(terminal)

    // Forward terminal input to server
    terminal.onData((data: string) => {
      bridge.sendInput(tabId, data)
    })

    // Forward terminal resize to server
    terminal.onResize(({ cols, rows }) => {
      bridge.resize(tabId, cols, rows)
    })

    // Track scroll position for the "Scroll down" pill above the input bar.
    // Only meaningful for the active tab — inactive tabs stay true.
    function checkAtBottom(): void {
      if (activeTabId.value !== tabId) return
      const buf = terminal.buffer.active
      terminalAtBottom.value = buf.viewportY >= buf.baseY
    }
    terminal.onScroll(checkAtBottom)
    // xterm's .onScroll can miss user-driven mousewheel scroll in some setups —
    // listen on the native viewport element as a belt-and-suspenders fallback.
    const viewportEl = container.querySelector('.xterm-viewport') as HTMLElement | null
    if (viewportEl) {
      viewportEl.addEventListener('scroll', checkAtBottom, { passive: true })
    }

    // Detect whether the CLI `>` / `›` prompt line is actually rendered in the
    // viewport. The server-side promptReady flip can arrive before xterm has
    // redrawn, so pressing Enter at that moment sends into the void. We check
    // the last few lines of the visible buffer on every render; debounced at
    // 50ms to avoid thrashing on streaming output.
    let promptVisibleTimer: ReturnType<typeof setTimeout> | null = null
    let lastPromptVisible: boolean | undefined
    function checkPromptVisible(): void {
      const buf = terminal.buffer.active
      const viewportTop = buf.viewportY
      const rows = terminal.rows
      let found = false
      // Scan from bottom up, last ~3 rows is enough — CLI prompt sits on the
      // bottom-most non-empty line.
      for (let i = rows - 1; i >= Math.max(0, rows - 4); i--) {
        const line = buf.getLine(viewportTop + i)
        if (!line) continue
        const text = line.translateToString(true)
        if (!text.trim()) continue
        // Match either ASCII `>` or the Unicode `❯`/`›` Claude CLI uses,
        // optionally followed by trailing user input. The prompt marker just
        // needs to be present at/near the start of the last non-empty line.
        if (/^\s*[›❯>]\s?/.test(text)) found = true
        break
      }
      if (found === lastPromptVisible) return
      lastPromptVisible = found
      const next = new Map(tabPromptVisible.value)
      next.set(tabId, found)
      tabPromptVisible.value = next
    }
    function scheduleCheckPromptVisible(): void {
      if (promptVisibleTimer) return
      promptVisibleTimer = setTimeout(() => {
        promptVisibleTimer = null
        checkPromptVisible()
      }, 50)
    }
    terminal.onRender(scheduleCheckPromptVisible)

    // One shared tick checkpoints only parsed writes. Snapshot payloads never
    // enter the UI state API or a multi-tab IPC serialization.
    const snapshot = terminalSnapshots.attach(tabId, scrollback => serializeAddon.serialize({ scrollback }))
    const snapshotDirty = terminal.onWriteParsed(() => {
      if (!terminalRegistry.get(tabId)?.needsClear) snapshot.markDirty()
    })
    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    // Register in global registry for bridge.onOutput routing
    terminalRegistry.set(tabId, { terminal, fitAddon, needsClear: false, pendingOutput: [], rafHandle: null })

    // Send initial size
    bridge.resize(tabId, terminal.cols, terminal.rows)

    return () => {
      snapshotDirty.dispose()
      snapshot.detach() // close invalidates the owner before this cleanup

      try { firstOutput.dispose() } catch { /* already disposed */ }
      const entry = terminalRegistry.get(tabId)
      if (entry?.rafHandle !== null && entry?.rafHandle !== undefined) {
        cancelAnimationFrame(entry.rafHandle)
      }
      terminalRegistry.delete(tabId)
      if (promptVisibleTimer) clearTimeout(promptVisibleTimer)
      const next = new Map(tabPromptVisible.value)
      if (next.delete(tabId)) tabPromptVisible.value = next
      // Dispose the GPU renderer first — terminal.dispose() will release
      // the addon, but explicit dispose surfaces WebGL context release
      // immediately so a fast tab-recreate doesn't pile up zombie GL
      // contexts.
      disposeRenderer()
      disposeConsole()
      try { serializeAddon.dispose() } catch { /* ignore */ }
      terminal.dispose()
      terminalRef.current = null
      fitAddonRef.current = null
    }
  }, [tabId])

  // Live theme switching — when the user toggles dark/light/system the CSS
  // variables on <html> change first, then this effect rebuilds the
  // xterm palette from the new computed values. Two RAFs let the
  // browser actually paint the new variables before getComputedStyle
  // reads them (one frame is sometimes enough but Chromium occasionally
  // batches the var update past the first rAF).
  useEffect(() => {
    const _theme = resolvedTheme.value
    void _theme
    void themeNonce.value // also rebuild on a same-kind theme swap (Kamin↔Bridge)
    const apply = () => {
      const t = terminalRef.current
      if (!t) return
      t.options.theme = buildTerminalTheme()
    }
    requestAnimationFrame(() => requestAnimationFrame(apply))
  }, [resolvedTheme.value, themeNonce.value])

  return { containerRef, terminalRef, fitAddonRef, hasOutput }
}
