/** Shared host adapter for the existing browser MediaRecorder/Whisper flow. */
export async function transcribeAudio(config: { whisperUrl?: string; whisperToken?: string; whisperModel?: string },
  base64: string, mimeType: string): Promise<{ text?: string; error?: string }> {
  try {
    if (!config.whisperUrl) return { error: 'Configure a Whisper API URL in settings' }
    const url = new URL(config.whisperUrl)
    if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) throw new Error('Invalid transcription URL')
    const extensions: Record<string, string> = { 'audio/wav': 'wav', 'audio/webm': 'webm', 'audio/mp4': 'mp4', 'audio/ogg': 'ogg', 'audio/mpeg': 'mp3' }
    const mime = String(mimeType).split(';')[0].trim().toLowerCase()
    if (!extensions[mime]) throw new Error('Unsupported audio format')
    if (typeof base64 !== 'string' || base64.length > 11_184_812 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(base64)) {
      throw new Error('Invalid audio encoding or audio exceeds 8 MiB')
    }
    const bytes = Buffer.from(base64, 'base64')
    if (!bytes.length || bytes.length > 8 * 1024 * 1024) throw new Error('Audio must contain 1 byte to 8 MiB')
    const body = new FormData()
    body.append('file', new Blob([bytes], { type: mime }), `recording.${extensions[mime]}`)
    body.append('model', config.whisperModel || 'whisper-1')
    const response = await fetch(url, { method: 'POST', body, redirect: 'error',
      headers: config.whisperToken ? { Authorization: `Bearer ${config.whisperToken}` } : undefined,
      signal: AbortSignal.timeout(120_000) })
    if (!response.ok) return { error: `Transcription failed (HTTP ${response.status})` }
    const result = await response.json() as { text?: unknown }
    if (typeof result.text !== 'string') throw new Error('Transcription returned no text')
    return { text: result.text }
  } catch {
    // Upstream error bodies and fetch URLs may contain credentials; keep them private.
    return { error: 'Could not transcribe audio; check endpoint, format and connectivity' }
  }
}
