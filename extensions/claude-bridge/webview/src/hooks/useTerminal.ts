import { storage } from "@bridge/storage"
import { useEffect, useRef, useState } from 'preact/hooks'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import { WebglAddon } from '@xterm/addon-webgl'
import { CanvasAddon } from '@xterm/addon-canvas'
import { SerializeAddon } from '@xterm/addon-serialize'
import { stripMouseTracking } from '../lib/strip-mouse-tracking'
import { buildTerminalTheme } from '../theme/terminal-theme'
import { resolvedTheme, themeNonce } from '../theme/apply-theme'
import { useBridge } from './useBridge'
import { terminalRegistry, terminalSnapshotKey } from '../signals/terminal-registry'
import { terminalAtBottom } from '../signals/ui'
import { activeTabId } from '../signals/tabs'
import { tabPromptVisible, tabPromptReady } from '../signals/connection'
import { localQueue } from '../signals/queue'

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

function copyText(bridge: { writeClipboard?: (t: string) => unknown } | undefined, text: string): void {
  // Sandboxed webview iframe (opaque origin): navigator.clipboard.writeText AND
  // document.execCommand('copy') are both blocked, so terminal copy silently did
  // nothing. Route through the host clipboard (vscode.env.clipboard via the
  // `write-clipboard` channel) — same path the chat copy button uses.
  if (bridge?.writeClipboard) { void bridge.writeClipboard(text); return }
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

async function pasteToTerminal(
  bridge: { sendInput(tabId: string, text: string): void; readClipboard?: () => Promise<string> },
  tabId: string,
): Promise<void> {
  try {
    // navigator.clipboard.readText is denied inside the sandboxed iframe (focus is
    // in the iframe, not the top document) — same reason copy routes through the
    // host. Read via the host clipboard (native arboard); fall back to navigator
    // only outside the webview (dashboard), where it works.
    const text = bridge.readClipboard ? await bridge.readClipboard() : await navigator.clipboard.readText()
    if (text) bridge.sendInput(tabId, text)
  } catch {
    // clipboard unavailable / denied — ignore
  }
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
      allowProposedApi: true,
      scrollback: 5000,
    })

    const fitAddon = new FitAddon()
    terminal.loadAddon(fitAddon)
    terminal.loadAddon(new WebLinksAddon())

    // Scrollback snapshot — restores terminal contents across renderer
    // reloads (HMR, devtools refresh, full application relaunch). Without
    // this, every reload presents the user with an empty xterm even
    // though the PTY session is still running on the server side; the
    // server replays JSONL but not the raw PTY output stream that
    // produced the rendered terminal layout (cursor moves, redraws,
    // etc). 256KB cap per tab keeps localStorage well under Chromium's
    // 5MB quota even with 30 active tabs.
    const serializeAddon = new SerializeAddon()
    terminal.loadAddon(serializeAddon)
    const SNAPSHOT_KEY = terminalSnapshotKey(tabId)
    const SNAPSHOT_MAX_BYTES = 256 * 1024
    const SNAPSHOT_INTERVAL_MS = 10_000

    terminal.open(container)
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

    // Restore prior snapshot, if any. We do this BEFORE attaching the
    // server output stream so the replay layers cleanly on top of the
    // stored buffer (server keeps its own scrollback and will deliver
    // anything new since disconnect).
    try {
      const stored = storage.getItem(SNAPSHOT_KEY)
      // Snapshots taken before the mouse-tracking filter landed may still
      // carry the DECSET that disables drag-selection — strip on restore too.
      if (stored) terminal.write(stripMouseTracking(stored))
    } catch { /* localStorage may be quota-exceeded or disabled */ }

    // Renderer ladder: WebGL → Canvas → DOM. WebGL gives ~5-9× the throughput
    // of the DOM renderer on heavy streams (per VSCode benchmarks) and uses
    // GPU-side glyph caching. We try it first; on context loss (driver
    // crash, GPU eviction, sandbox without GL) the addon emits
    // `onContextLoss` — we dispose and fall back to Canvas. Canvas itself
    // can fail in environments with no 2D acceleration; in that case we
    // just leave the default DOM renderer in place. xterm's DOM renderer
    // ALWAYS works, so any failure path is safe.
    let activeRenderer: WebglAddon | CanvasAddon | null = null
    function tryWebgl(): boolean {
      try {
        const webgl = new WebglAddon()
        webgl.onContextLoss(() => {
          try { webgl.dispose() } catch { /* ignore */ }
          activeRenderer = null
          tryCanvas()
        })
        terminal.loadAddon(webgl)
        activeRenderer = webgl
        return true
      } catch {
        return false
      }
    }
    function tryCanvas(): void {
      try {
        const canvas = new CanvasAddon()
        terminal.loadAddon(canvas)
        activeRenderer = canvas
      } catch {
        // Both accelerated paths failed — DOM stays. Nothing to do.
      }
    }
    if (!tryWebgl()) tryCanvas()

    // Clipboard: paste via native paste event on xterm's textarea
    const xtermTextarea = container.querySelector('textarea')
    if (xtermTextarea) {
      xtermTextarea.addEventListener('paste', (e) => {
        e.preventDefault()
        e.stopImmediatePropagation()
        const text = (e as ClipboardEvent).clipboardData?.getData('text/plain')
        if (text) {
          bridge.sendInput(tabId, text)
        } else {
          pasteToTerminal(bridge, tabId).catch(() => { console.warn('[terminal] Paste failed') })
        }
      }, true)
    }

    // Key handler: use ev.code (physical key) — works with any keyboard layout
    terminal.attachCustomKeyEventHandler((ev: KeyboardEvent) => {
      if (ev.type !== 'keydown') return true

      // Печатный символ без модификаторов: НЕ отдавать xterm'у keydown —
      // offscreen-рендерер CEF не получает смену раскладки (нет
      // WM_INPUTLANGCHANGE), и keydown-трансляция VK→символ залипает на
      // стартовом языке («ввод в консоли не переключается на русский»).
      // keypress придёт следом с готовым юникодом из CHAR-события шелла
      // (там раскладка юзера учтена) — его xterm и обработает.
      if (ev.key.length === 1 && !ev.ctrlKey && !ev.altKey && !ev.metaKey) {
        return false
      }

      // Ctrl+Shift+C → always copy selection
      if (ev.ctrlKey && ev.shiftKey && ev.code === 'KeyC') {
        const sel = terminal.getSelection()
        if (sel) {
          copyText(bridge, sel)
          terminal.clearSelection()
        }
        return false
      }

      // Ctrl+Shift+V / Ctrl+V → block xterm, let paste event handle it
      if (ev.ctrlKey && ev.code === 'KeyV') {
        return false
      }

      // Ctrl+C → copy if selection, else send SIGINT
      if (ev.ctrlKey && !ev.shiftKey && ev.code === 'KeyC') {
        const sel = terminal.getSelection()
        if (sel) {
          copyText(bridge, sel)
          terminal.clearSelection()
          return false
        }
        return true // let xterm send ^C
      }

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

      // Ctrl+X → copy selection (cut)
      if (ev.ctrlKey && ev.code === 'KeyX') {
        const sel = terminal.getSelection()
        if (sel) {
          copyText(bridge, sel)
          terminal.clearSelection()
          return false
        }
        return true
      }

      return true
    })

    // Context menu (right-click): copy / paste
    container.addEventListener('contextmenu', (e) => {
      e.preventDefault()
      const sel = terminal.getSelection()

      // Remove existing menu if any
      document.querySelector('.terminal-context-menu')?.remove()

      const menu = document.createElement('div')
      menu.className = 'terminal-context-menu'
      menu.style.cssText = `
        position: fixed; left: ${e.clientX}px; top: ${e.clientY}px;
        background: var(--bg-surface); border: 1px solid var(--bg-overlay); border-radius: var(--radius-sm);
        padding: 4px 0; z-index: 99999; min-width: 140px;
        box-shadow: 0 4px 12px rgba(0,0,0,0.4); font-size: 13px;
      `

      const itemStyle = `
        padding: 6px 14px; color: var(--text-primary); cursor: pointer;
        display: flex; align-items: center; gap: 10px;
      `
      const hoverIn = (el: HTMLElement) => { el.style.background = 'var(--bg-overlay)' }
      const hoverOut = (el: HTMLElement) => { el.style.background = 'transparent' }

      if (sel) {
        const copyItem = document.createElement('div')
        copyItem.style.cssText = itemStyle
        copyItem.innerHTML = '<i class="fas fa-copy" style="width:16px;text-align:center;color:var(--text-muted)"></i> Copy'
        copyItem.onmouseenter = () => hoverIn(copyItem)
        copyItem.onmouseleave = () => hoverOut(copyItem)
        copyItem.onclick = () => { copyText(bridge, sel); terminal.clearSelection(); menu.remove() }
        menu.appendChild(copyItem)
      }

      const pasteItem = document.createElement('div')
      pasteItem.style.cssText = itemStyle
      pasteItem.innerHTML = '<i class="fas fa-paste" style="width:16px;text-align:center;color:var(--text-muted)"></i> Paste'
      pasteItem.onmouseenter = () => hoverIn(pasteItem)
      pasteItem.onmouseleave = () => hoverOut(pasteItem)
      pasteItem.onclick = () => {
        pasteToTerminal(bridge, tabId).catch(() => { console.warn('[terminal] Paste failed') })
        menu.remove()
      }
      menu.appendChild(pasteItem)

      if (sel) {
        const selectAllItem = document.createElement('div')
        selectAllItem.style.cssText = itemStyle
        selectAllItem.innerHTML = '<i class="fas fa-check-double" style="width:16px;text-align:center;color:var(--text-muted)"></i> Select All'
        selectAllItem.onmouseenter = () => hoverIn(selectAllItem)
        selectAllItem.onmouseleave = () => hoverOut(selectAllItem)
        selectAllItem.onclick = () => { terminal.selectAll(); menu.remove() }
        menu.appendChild(selectAllItem)
      }

      document.body.appendChild(menu)

      const dismiss = (ev: MouseEvent) => {
        if (!menu.contains(ev.target as Node)) {
          menu.remove()
          document.removeEventListener('mousedown', dismiss)
        }
      }
      setTimeout(() => document.addEventListener('mousedown', dismiss), 0)
    })

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

    // Periodic snapshot — debounced to once per SNAPSHOT_INTERVAL_MS so
    // the cost of serializing 5000 rows (~5-10ms on Chromium per the
    // addon benchmarks) stays off the render hot path. Cap on bytes
    // protects localStorage from overflowing on shells that produce
    // massive ANSI graphs (htop, btop, etc.).
    const snapshotTimer = setInterval(() => {
      try {
        const data = serializeAddon.serialize({ scrollback: 1000 })
        if (data.length <= SNAPSHOT_MAX_BYTES) {
          storage.setItem(SNAPSHOT_KEY, data)
        } else {
          // Too big — drop the oldest material by trimming from the
          // front. Loose heuristic: keep the trailing 90% of the cap so
          // the cursor / current screen survives.
          storage.setItem(SNAPSHOT_KEY, data.slice(-Math.floor(SNAPSHOT_MAX_BYTES * 0.9)))
        }
      } catch { /* quota or disabled — silently drop */ }
    }, SNAPSHOT_INTERVAL_MS)

    terminalRef.current = terminal
    fitAddonRef.current = fitAddon

    // Register in global registry for bridge.onOutput routing
    terminalRegistry.set(tabId, { terminal, fitAddon, needsClear: false, pendingOutput: [], rafHandle: null, outputCarry: '' })

    // Send initial size
    bridge.resize(tabId, terminal.cols, terminal.rows)

    return () => {
      // One last snapshot before tear-down so the next mount starts
      // from the very latest state, not from up-to-10s-ago.
      clearInterval(snapshotTimer)
      try {
        const data = serializeAddon.serialize({ scrollback: 1000 })
        const trimmed = data.length > SNAPSHOT_MAX_BYTES
          ? data.slice(-Math.floor(SNAPSHOT_MAX_BYTES * 0.9))
          : data
        storage.setItem(SNAPSHOT_KEY, trimmed)
      } catch { /* ignore */ }

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
      try { activeRenderer?.dispose() } catch { /* ignore */ }
      activeRenderer = null
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
