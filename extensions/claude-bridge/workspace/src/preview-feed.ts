// One host-side observer of preview state for every connected browser view.
// Browsers used to poll GET /preview/state once a second each; now the host
// pushes descriptor changes at once and samples the entry fingerprint (the
// remount fallback for dev servers without HMR) once per interval for all views,
// and only while a remote is mounted and someone listens.
import type { PreviewSnapshot, WorkspacePreview } from './preview.js'

export interface PreviewFeed {
  /** Delivers the current state promptly, then every change. Returns unsubscribe. */
  subscribe(listener: (state: PreviewSnapshot) => void): () => void
  close(): void
}

export function createPreviewFeed(preview: WorkspacePreview, intervalMs = 1000): PreviewFeed {
  const listeners = new Set<(state: PreviewSnapshot) => void>()
  let last: { state: PreviewSnapshot; key: string } | undefined
  let timer: ReturnType<typeof setTimeout> | undefined
  let sampling = false
  let again = false
  let closed = false

  function deliver(listener: (state: PreviewSnapshot) => void, state: PreviewSnapshot): void {
    try { listener(structuredClone(state)) } catch { /* one view cannot stop the others */ }
  }
  function schedule(): void {
    clearTimeout(timer)
    timer = undefined
    if (closed || !listeners.size || !preview.copy()) return
    timer = setTimeout(() => { void sample() }, intervalMs)
    timer.unref?.()
  }
  async function sample(): Promise<void> {
    if (closed || !listeners.size) return
    if (sampling) { again = true; return }
    sampling = true
    try {
      do {
        again = false
        const state = await preview.snapshot()
        const key = JSON.stringify(state)
        if (closed || key === last?.key) continue
        last = { state, key }
        for (const listener of [...listeners]) deliver(listener, state)
      } while (again && !closed)
    } catch { /* snapshot never rejects by contract; keep the feed alive regardless */ }
    finally {
      sampling = false
      schedule()
    }
  }
  const detach = preview.onChange(() => { void sample() })

  return {
    subscribe(listener) {
      if (closed) return () => {}
      // Without listeners nothing sampled the fingerprint; the cache is stale.
      if (!listeners.size) last = undefined
      listeners.add(listener)
      if (last) deliver(listener, last.state)
      void sample()
      return () => {
        listeners.delete(listener)
        if (!listeners.size) { clearTimeout(timer); timer = undefined }
      }
    },
    close() {
      closed = true
      detach()
      clearTimeout(timer)
      listeners.clear()
    },
  }
}
