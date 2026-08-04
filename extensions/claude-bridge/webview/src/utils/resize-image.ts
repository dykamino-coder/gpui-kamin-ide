// Downscale large pasted/dropped images before we persist them to disk and
// ship them to the CLI. A screenshot from a 4K monitor is ~10MB; Claude's
// vision tools don't benefit from resolutions above ~1080p on the long edge,
// and uploading the full file wastes tokens + disk.

export interface ResizedImage {
  base64: string
  mime: string
  /** True if downscaling actually happened; false if the source was already
   *  within the target box (we return the original bytes verbatim). */
  resized: boolean
  width: number
  height: number
}

const MAX_EDGE = 1920 // "1080p" = 1920x1080 — keep the longer edge ≤ 1920.
const JPEG_QUALITY = 0.9

/** Resize a base64-encoded image to `maxEdge` px on the long edge. Returns
 *  a JPEG for JPEG sources, PNG for transparent/PNG sources. */
export async function resizeImageBase64(
  base64: string,
  mime: string,
  maxEdge: number = MAX_EDGE,
): Promise<ResizedImage> {
  const dataUri = `data:${mime};base64,${base64}`
  const img = await loadImage(dataUri)
  const { naturalWidth: w, naturalHeight: h } = img
  if (!w || !h || Math.max(w, h) <= maxEdge) {
    return { base64, mime, resized: false, width: w, height: h }
  }
  const scale = maxEdge / Math.max(w, h)
  const targetW = Math.round(w * scale)
  const targetH = Math.round(h * scale)

  const canvas = document.createElement('canvas')
  canvas.width = targetW
  canvas.height = targetH
  const ctx = canvas.getContext('2d')
  if (!ctx) return { base64, mime, resized: false, width: w, height: h }
  ctx.drawImage(img, 0, 0, targetW, targetH)

  // JPEG for photos (smaller files, faster round-trip) unless the source was
  // a PNG that may rely on transparency.
  const outMime = mime === 'image/png' ? 'image/png' : 'image/jpeg'
  const blob = await new Promise<Blob | null>((resolve) =>
    canvas.toBlob(resolve, outMime, outMime === 'image/jpeg' ? JPEG_QUALITY : undefined),
  )
  if (!blob) return { base64, mime, resized: false, width: w, height: h }
  const outBuf = await blob.arrayBuffer()
  const bytes = new Uint8Array(outBuf)
  let binary = ''
  for (let i = 0; i < bytes.length; i += 8192) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 8192))
  }
  return {
    base64: btoa(binary),
    mime: outMime,
    resized: true,
    width: targetW,
    height: targetH,
  }
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image()
    img.onload = () => resolve(img)
    img.onerror = reject
    img.src = src
  })
}
