// xterm 6 removed CanvasAddon. Disposing WebGL restores its built-in DOM
// renderer, retaining output and input when a GPU context cannot be used.
import type { Terminal } from '@xterm/xterm'
import { WebglAddon } from '@xterm/addon-webgl'

export function enableTerminalRenderer(terminal: Terminal): () => void {
  let webgl: WebglAddon | null = null
  let contextLoss: { dispose(): void } | null = null
  function dispose(): void {
    contextLoss?.dispose()
    contextLoss = null
    try { webgl?.dispose() } catch { /* a lost context may already be released */ }
    webgl = null
  }
  try {
    webgl = new WebglAddon()
    contextLoss = webgl.onContextLoss(() => {
      dispose()
      terminal.refresh(0, Math.max(0, terminal.rows - 1))
    })
    terminal.loadAddon(webgl)
  } catch {
    dispose()
    terminal.refresh(0, Math.max(0, terminal.rows - 1))
  }
  return dispose
}
