import type { JSX } from 'preact'
import { useBridge } from '../../hooks/useBridge'
import { pendingAttachments } from '../../signals/ui'

export function AttachButton(): JSX.Element {
  const bridge = useBridge()

  async function handleClick(): Promise<void> {
    // Open file dialog for any file type. openFileDialog is the universal
    // picker; fall back to openImageDialog only if the bridge truly lacks it.
    const files = bridge.openFileDialog
      ? await bridge.openFileDialog()
      : await bridge.openImageDialog()
    for (const file of files as Array<{ path: string; name?: string; ext?: string; base64?: string; mimeType?: string; isImage?: boolean }>) {
      const name = file.name || file.path.split(/[/\\]/).pop() || file.path
      const ext = file.ext ?? (name.includes('.') ? name.split('.').pop()!.toLowerCase() : '')
      const isImage = file.isImage ?? ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg'].includes(ext)
      pendingAttachments.value = [
        ...pendingAttachments.value,
        {
          path: file.path,
          name,
          ext,
          dataUri: isImage && file.base64 ? `data:${file.mimeType};base64,${file.base64}` : undefined,
        },
      ]
    }
  }

  return (
    <button
      class="attach-btn"
      data-tooltip="Attach file"
      type="button"
      onClick={handleClick}
    >
      <i class="fas fa-paperclip" />
    </button>
  )
}
