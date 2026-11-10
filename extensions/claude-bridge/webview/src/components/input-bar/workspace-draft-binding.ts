// Связывает общий textarea с хостовым портом без повторной отправки восстановлений.
import type { InputDraftPort } from '../../lib/input-draft-port'
import { pendingAttachments } from '../../signals/ui'

export function bindWorkspaceDraft(port: InputDraftPort, tabId: string, el: HTMLTextAreaElement | null,
  setHasText: (value: boolean) => void): () => void {
  let applying = false
  let loaded = false
  let last = ''
  const current = () => ({ text: el?.value ?? '', attachments: pendingAttachments.value })
  function flush(): void {
    if (!loaded || applying) return
    const value = current()
    const serialized = JSON.stringify(value)
    if (serialized === last) return
    last = serialized
    port.save(tabId, value)
  }
  if (el) { el.value = ''; el.readOnly = true }
  setHasText(false)
  pendingAttachments.value = []
  const unwatch = port.watch(tabId, value => {
    applying = true
    loaded = true
    if (el) { el.value = value.text; el.readOnly = false }
    pendingAttachments.value = value.attachments
    last = JSON.stringify(value)
    setHasText(!!value.text.trim())
    el?.dispatchEvent(new Event('input', { bubbles: true }))
    applying = false
  })
  const unsubscribe = pendingAttachments.subscribe(flush)
  el?.addEventListener('input', flush)
  // Capture precedes the browser entry's pagehide transport disposal.
  window.addEventListener('pagehide', flush, true)
  return () => {
    flush()
    unwatch(); unsubscribe()
    el?.removeEventListener('input', flush)
    window.removeEventListener('pagehide', flush, true)
    if (el) el.readOnly = false
  }
}
