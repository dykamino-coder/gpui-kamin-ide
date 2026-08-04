import { useCallback } from 'preact/hooks'
import { useBridge } from './useBridge'
import { pendingAttachments } from '../signals/ui'
import { resizeImageBase64 } from '../utils/resize-image'

export interface ClipboardPasteHandlers {
  onPaste: (e: ClipboardEvent) => void
}

export function useClipboardPaste(): ClipboardPasteHandlers {
  const bridge = useBridge()

  const onPaste = useCallback((e: ClipboardEvent) => {
    const clipboardData = e.clipboardData
    if (!clipboardData) return

    for (const item of Array.from(clipboardData.items)) {
      if (item.kind !== 'file') continue
      const blob = item.getAsFile()
      if (!blob) continue

      if (item.type.startsWith('image/')) {
        e.preventDefault()
        blob.arrayBuffer().then(async (buffer) => {
          const bytes = new Uint8Array(buffer)
          let binary = ''
          for (let i = 0; i < bytes.length; i += 8192) {
            binary += String.fromCharCode(...bytes.subarray(i, i + 8192))
          }
          const rawBase64 = btoa(binary)
          // Downscale big screenshots to 1080p before saving to disk.
          const resized = await resizeImageBase64(rawBase64, item.type).catch(() => ({
            base64: rawBase64, mime: item.type, resized: false, width: 0, height: 0,
          }))
          const filePath = await bridge.saveClipboardImage(resized.base64, resized.mime)
          const name = filePath.split(/[/\\]/).pop() || 'clipboard-image'
          const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : 'png'
          pendingAttachments.value = [
            ...pendingAttachments.value,
            { path: filePath, name, ext, dataUri: `data:${resized.mime};base64,${resized.base64}` },
          ]
        })
        return
      }

      // Non-image file pasted (e.g. file from Explorer copied to clipboard):
      // байты → temp через save-dropped-file (аудит #70 A3 — прежний
      // getDroppedFilePath="" делал ветку тихим no-op).
      e.preventDefault()
      void (async () => {
        const buf = new Uint8Array(await blob.arrayBuffer())
        let binary = ''
        for (let i = 0; i < buf.length; i += 8192) {
          binary += String.fromCharCode(...buf.subarray(i, i + 8192))
        }
        const pastedPath = await bridge.saveDroppedFile(btoa(binary), blob.name || 'attachment')
        if (!pastedPath) return
        const name = blob.name || pastedPath.split(/[/\\]/).pop() || 'attachment'
        const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : ''
        pendingAttachments.value = [
          ...pendingAttachments.value,
          { path: pastedPath, name, ext },
        ]
      })()
      return
    }
    // Text paste — let browser handle it normally
  }, [bridge])

  return { onPaste }
}
