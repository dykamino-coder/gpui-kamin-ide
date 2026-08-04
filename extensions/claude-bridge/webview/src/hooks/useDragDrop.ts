import { useState, useEffect, useCallback } from 'preact/hooks'
import { useBridge } from './useBridge'
import { pendingAttachments } from '../signals/ui'
import { resizeImageBase64 } from '../utils/resize-image'

export interface DragDropState {
  isDragging: boolean
}

export function useDragDrop(): DragDropState {
  const bridge = useBridge()
  const [isDragging, setIsDragging] = useState(false)
  const [dragCounter, setDragCounter] = useState(0)

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault()
    const dt = e.dataTransfer
    if (dt && dt.types.includes('Files')) {
      setDragCounter(c => {
        const next = c + 1
        if (next === 1) setIsDragging(true)
        return next
      })
    }
  }, [])

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault()
    setDragCounter(c => {
      const next = Math.max(0, c - 1)
      if (next === 0) setIsDragging(false)
      return next
    })
  }, [])

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault()
    if (e.dataTransfer) e.dataTransfer.dropEffect = 'copy'
  }, [])

  const handleDrop = useCallback(async (e: DragEvent) => {
    e.preventDefault()
    setDragCounter(0)
    setIsDragging(false)

    const files = e.dataTransfer?.files
    if (!files) return

    for (const file of Array.from(files)) {
      if (file.type.startsWith('image/')) {
        const buffer = await file.arrayBuffer()
        const bytes = new Uint8Array(buffer)
        let binary = ''
        for (let i = 0; i < bytes.length; i += 8192) {
          binary += String.fromCharCode(...bytes.subarray(i, i + 8192))
        }
        const rawBase64 = btoa(binary)
        // Downscale 4K screenshots etc. to 1080p before persisting — saves disk,
        // saves tokens, keeps Claude's vision quality the same.
        const resized = await resizeImageBase64(rawBase64, file.type).catch(() => ({
          base64: rawBase64, mime: file.type, resized: false, width: 0, height: 0,
        }))
        const filePath = await bridge.saveClipboardImage(resized.base64, resized.mime)
        const baseName = file.name || 'dropped-image'
        const ext = resized.mime === 'image/jpeg' ? 'jpg' : resized.mime === 'image/png' ? 'png'
          : baseName.includes('.') ? baseName.split('.').pop()!.toLowerCase() : 'png'
        const name = resized.resized && !baseName.includes('.')
          ? `${baseName}.${ext}`
          : baseName
        pendingAttachments.value = [
          ...pendingAttachments.value,
          { path: filePath, name, ext, dataUri: `data:${resized.mime};base64,${resized.base64}` },
        ]
        continue
      }

      // Non-image: у песочного вебвью нет OS-пути File (webUtils — Electron),
      // поэтому байты уезжают в temp через save-dropped-file и CLI получает
      // реальный путь (аудит #70 A3: прежний getDroppedFilePath="" делал
      // ветку тихим no-op).
      const buf = new Uint8Array(await file.arrayBuffer())
      let binary = ''
      for (let i = 0; i < buf.length; i += 8192) {
        binary += String.fromCharCode(...buf.subarray(i, i + 8192))
      }
      const droppedPath = await bridge.saveDroppedFile(btoa(binary), file.name || 'attachment')
      if (!droppedPath) continue
      const baseName = file.name || droppedPath.split(/[/\\]/).pop() || 'attachment'
      const ext = baseName.includes('.') ? baseName.split('.').pop()!.toLowerCase() : ''
      pendingAttachments.value = [
        ...pendingAttachments.value,
        { path: droppedPath, name: baseName, ext },
      ]
    }
  }, [bridge])

  useEffect(() => {
    document.addEventListener('dragenter', handleDragEnter)
    document.addEventListener('dragleave', handleDragLeave)
    document.addEventListener('dragover', handleDragOver)
    document.addEventListener('drop', handleDrop)
    return () => {
      document.removeEventListener('dragenter', handleDragEnter)
      document.removeEventListener('dragleave', handleDragLeave)
      document.removeEventListener('dragover', handleDragOver)
      document.removeEventListener('drop', handleDrop)
    }
  }, [handleDragEnter, handleDragLeave, handleDragOver, handleDrop])

  return { isDragging }
}
