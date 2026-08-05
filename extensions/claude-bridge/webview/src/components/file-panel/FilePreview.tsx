import type { JSX } from 'preact'
import { useEffect, useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'

const MIME_BY_EXT: Record<string, string> = {
  png: 'image/png',
  jpg: 'image/jpeg',
  jpeg: 'image/jpeg',
  gif: 'image/gif',
  webp: 'image/webp',
  bmp: 'image/bmp',
  ico: 'image/x-icon',
  svg: 'image/svg+xml',
  avif: 'image/avif',
  pdf: 'application/pdf',
}

const IMAGE_EXTS = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico', 'svg', 'avif'])

export function isPreviewablePath(filePath: string): boolean {
  const ext = filePath.toLowerCase().match(/\.([a-z0-9]+)$/)?.[1] ?? ''
  return ext === 'pdf' || IMAGE_EXTS.has(ext)
}

/** Native preview for images + PDFs. Reads the file as base64 once and
 *  renders an <img> (image) or <iframe> (PDF — Chromium's built-in
 *  Chromium PDF viewer handles it). Skips CodeMirror entirely so the
 *  user doesn't see binary garbage. No editing — read-only by design. */
export function FilePreview({ filePath }: { filePath: string }): JSX.Element {
  const bridge = useBridge()
  const [dataUrl, setDataUrl] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [size, setSize] = useState<number>(0)

  const ext = filePath.toLowerCase().match(/\.([a-z0-9]+)$/)?.[1] ?? ''
  const mime = MIME_BY_EXT[ext] ?? 'application/octet-stream'
  const isImage = IMAGE_EXTS.has(ext)
  const isPdf = ext === 'pdf'

  useEffect(() => {
    let cancelled = false
    setDataUrl(null)
    setError(null)
    bridge.fileViewerReadBinary(filePath).then((res) => {
      if (cancelled) return
      setSize(res.size)
      setDataUrl(`data:${mime};base64,${res.base64}`)
    }).catch((e) => {
      if (cancelled) return
      setError(String(e))
    })
    return () => { cancelled = true }
  }, [filePath, mime])

  if (error) {
    return (
      <div style="flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:24px;color:var(--accent-red);font-size:11px">
        Failed to read file: {error}
      </div>
    )
  }
  if (!dataUrl) {
    return (
      <div style="flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:24px;color:var(--text-muted);font-size:11px">
        Loading preview…
      </div>
    )
  }

  if (isImage) {
    return (
      <div style="flex:1;min-height:0;display:flex;flex-direction:column;background:var(--bg-mantle)">
        <div style="flex-shrink:0;padding:6px 12px;border-bottom:1px solid var(--bg-surface);font-size:10px;color:var(--text-muted);font-family:ui-monospace,'Cascadia Code',Menlo,Consolas,monospace;display:flex;align-items:center;gap:8px">
          <i class="fas fa-image" style="color:var(--accent-pink)" />
          <span>{ext.toUpperCase()}</span>
          <span style="color:var(--text-disabled)">·</span>
          <span>{formatBytes(size)}</span>
        </div>
        <div style="flex:1;min-height:0;overflow:auto;display:flex;align-items:center;justify-content:center;padding:16px">
          <img
            src={dataUrl}
            alt={filePath}
            style="max-width:100%;max-height:100%;object-fit:contain;border-radius:var(--radius-sm);box-shadow:0 4px 16px rgba(0,0,0,0.25);background:repeating-conic-gradient(var(--bg-base) 0% 25%, var(--bg-surface) 0% 50%) 50% / 16px 16px"
          />
        </div>
      </div>
    )
  }

  if (isPdf) {
    // Native Chromium PDF viewer's toolbar can't be themed via CSS
    // (it's an internal extension UI). Hide it via URL fragment hints
    // and let our own header (file info + future controls) drive the
    // chrome — wrapped in app tokens. The document itself renders
    // untouched (white paper, real text). Use blob URL because data:
    // URLs ignore the fragment in Chromium's PDF embed.
    return <PdfPreview dataUrl={dataUrl} filePath={filePath} sizeBytes={size} />
  }

  return (
    <div style="flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:24px;color:var(--text-muted);font-size:11px">
      Unsupported file type for native preview.
    </div>
  )
}

/** PDF block — wraps Chromium's embedded viewer in our chrome. The
 *  data URL is converted to a blob URL with `#toolbar=0&navpanes=0&
 *  scrollbar=1` fragment hints so Chromium's own toolbar/sidebar are
 *  hidden; the document area stays exactly as Chromium renders it. */
function PdfPreview({ dataUrl, filePath, sizeBytes }: { dataUrl: string; filePath: string; sizeBytes: number }): JSX.Element {
  const [blobUrl, setBlobUrl] = useState<string | null>(null)
  useEffect(() => {
    // Decode the data URL into a Blob so we can append the
    // `#toolbar=0` fragment Chromium honors only on http/blob URLs.
    const base64 = dataUrl.split(',')[1] ?? ''
    const bin = atob(base64)
    const buf = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) buf[i] = bin.charCodeAt(i)
    const blob = new Blob([buf], { type: 'application/pdf' })
    const url = URL.createObjectURL(blob)
    setBlobUrl(url)
    return () => { URL.revokeObjectURL(url) }
  }, [dataUrl])

  return (
    <div style="flex:1;min-height:0;display:flex;flex-direction:column;background:var(--bg-mantle)">
      <div style="flex-shrink:0;padding:6px 12px;border-bottom:1px solid var(--bg-surface);font-size:10px;color:var(--text-muted);font-family:ui-monospace,'Cascadia Code',Menlo,Consolas,monospace;display:flex;align-items:center;gap:8px">
        <i class="fas fa-file-pdf" style="color:var(--accent-red)" />
        <span>PDF</span>
        <span style="color:var(--text-disabled)">·</span>
        <span>{formatBytes(sizeBytes)}</span>
        <span style="margin-left:auto;color:var(--text-disabled);font-size:10px">scroll · Ctrl+± zoom</span>
      </div>
      {blobUrl && (
        <iframe
          src={`${blobUrl}#toolbar=0&navpanes=0&scrollbar=1&view=FitH`}
          title={filePath}
          style="flex:1;min-height:0;border:none;background:var(--bg-base)"
        />
      )}
    </div>
  )
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`
  return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`
}
