// Clipboard write that works from a sandboxed webview iframe.
//
// navigator.clipboard.writeText requires the DOCUMENT to be focused; a copy
// gesture inside the Bridge iframe keeps focus in the iframe, so the direct
// call rejects with "Document is not focused". The host bridge routes the
// write through vscode.env.clipboard → the native KaminIDE clipboard, which has no
// focus requirement (see memory: reference_kaminide_clipboard).
import { showToast } from '../signals/toasts'

interface ClipboardBridge {
  writeClipboard?: (text: string) => Promise<void> | void
}

export async function copyToClipboard(bridge: ClipboardBridge, text: string): Promise<void> {
  if (typeof bridge.writeClipboard === 'function') {
    await bridge.writeClipboard(text)
    return
  }
  await navigator.clipboard.writeText(text)
}

/** Convenience for call sites that don't already hold the bridge (global click
 *  delegates, tool renderers): reads window.kaminBridge and routes through
 *  the host clipboard, falling back to navigator.clipboard only outside the
 *  webview (dashboard). */
export async function writeClipboardText(text: string): Promise<void> {
  const bridge = (window as unknown as { kaminBridge?: ClipboardBridge }).kaminBridge
  await copyToClipboard(bridge ?? {}, text)
}

/** Copy + HONEST feedback: the success toast fires only after the write actually
 *  resolves; a rejection shows an error toast instead. Replaces the fire-and-
 *  forget `void writeClipboardText(t).catch(()=>{}); showToast(success)` pattern
 *  that claimed "Copied" even when the write failed (and the direct
 *  navigator.clipboard path, which always rejects inside the sandboxed iframe). */
export async function copyWithToast(text: string, successTitle: string, duration = 1500): Promise<void> {
  try {
    await writeClipboardText(text)
    showToast({ type: 'success', title: successTitle, duration })
  } catch {
    showToast({ type: 'error', title: 'Copy failed', message: 'Could not access clipboard' })
  }
}
