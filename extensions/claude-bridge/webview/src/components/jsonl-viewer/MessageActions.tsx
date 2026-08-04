import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { showToast } from '../../signals/toasts'

/** Compact "Copy / Save" button group rendered in the top-right of a
 *  message entry header. Visibility is driven by the parent's `:hover`
 *  state via opacity in CSS — buttons stay reachable but don't clutter
 *  the static reading view. */
export function MessageActions({
  text,
  defaultName,
}: {
  text: string
  defaultName?: string
}): JSX.Element {
  const [copied, setCopied] = useState(false)

  async function handleCopy(e: MouseEvent): Promise<void> {
    e.stopPropagation()
    // Clipboard via the host — the sandboxed webview (opaque origin) can't use
    // navigator.clipboard, so route through vscode.env.clipboard (bridge).
    const bridge = (window as any).electronBridge
    try {
      if (bridge?.writeClipboard) await bridge.writeClipboard(text)
      else await navigator.clipboard.writeText(text)
      setCopied(true)
      setTimeout(() => setCopied(false), 1200)
    } catch {
      showToast({ type: 'error', title: 'Copy failed', message: 'Could not access clipboard' })
    }
  }

  async function handleSave(e: MouseEvent): Promise<void> {
    e.stopPropagation()
    const bridge = (window as any).electronBridge
    if (bridge?.saveTextAs) {
      const r = await bridge.saveTextAs(text, defaultName ?? `message-${Date.now()}.md`)
      if (r?.ok) {
        showToast({ type: 'success', title: 'Saved', message: r.filePath ?? '', duration: 1800 })
      } else if (r?.error && r.error !== 'Cancelled') {
        showToast({ type: 'error', title: 'Save failed', message: r.error })
      }
      return
    }
    // Fallback for non-Electron (dashboard) — browser blob download
    try {
      const blob = new Blob([text], { type: 'text/markdown;charset=utf-8' })
      const url = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = url
      a.download = defaultName ?? `message-${Date.now()}.md`
      document.body.appendChild(a)
      a.click()
      a.remove()
      URL.revokeObjectURL(url)
    } catch {
      showToast({ type: 'error', title: 'Save failed', message: 'Browser download blocked' })
    }
  }

  return (
    <span class="msg-actions">
      <button
        type="button"
        class="msg-action-btn"
        title={copied ? 'Copied!' : 'Copy message'}
        onClick={handleCopy}
      >
        <i class={copied ? 'fas fa-check' : 'fas fa-copy'} style="font-size:11px" />
      </button>
      <button
        type="button"
        class="msg-action-btn"
        title="Save as file…"
        onClick={handleSave}
      >
        <i class="fas fa-floppy-disk" style="font-size:11px" />
      </button>
    </span>
  )
}
