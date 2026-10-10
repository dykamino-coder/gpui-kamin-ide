/** The native browser download owns streamed bytes and cancellation. A scoped
 * HttpOnly cookie issued by authenticated POST authorizes its GET; neither
 * Bearer credentials nor a complete Blob enter a URL or the renderer. */
function launchAttachment(url: string): () => void {
  const frame = document.createElement('iframe')
  frame.hidden = true
  frame.referrerPolicy = 'no-referrer'
  frame.src = url
  document.body.appendChild(frame)
  return () => frame.remove()
}
export function createJsonlDownloader(
  onBusy: (busy: boolean) => void,
  request: typeof fetch = fetch,
  launch = launchAttachment,
) {
  let busy = false
  let disposed = false
  let abort: AbortController | undefined
  let cooldown: ReturnType<typeof setTimeout> | undefined
  let removeFrame: (() => void) | undefined
  return {
    async start(sessionId: string, token: string | null): Promise<boolean> {
      if (disposed || busy) return false
      busy = true
      onBusy(true)
      abort = new AbortController()
      try {
        const expected = `/api/dashboard/sessions/${encodeURIComponent(sessionId)}/jsonl-download`
        const response = await request(expected, {
          method: 'POST',
          signal: abort.signal,
          headers: token ? { Authorization: `Bearer ${token}` } : {},
        })
        if (!response.ok) throw new Error(`HTTP ${response.status}`)
        const { url } = (await response.json()) as { url?: unknown }
        if (disposed) return false
        if (url !== expected) throw new Error('Invalid download destination')
        removeFrame?.()
        removeFrame = launch(url)
        // Synchronous lock covers rapid clicks before state rerenders; cooldown
        // limits repeated exports. Server also enforces 2/caller and 8 global.
        cooldown = setTimeout(() => {
          busy = false
          onBusy(false)
        }, 10_000)
        return true
      } catch (error) {
        if (disposed) return false
        busy = false
        onBusy(false)
        throw error
      } finally {
        abort = undefined
      }
    },
    dispose(): void {
      disposed = true
      abort?.abort()
      if (cooldown) clearTimeout(cooldown)
      removeFrame?.()
    },
  }
}
