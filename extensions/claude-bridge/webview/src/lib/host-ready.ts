// Tell the KaminIDE host that this webview has actually PAINTED (not just that
// the iframe document loaded). The host's WebviewPanelView holds a loading cover
// until this arrives, so there's no flash of an empty themed background between
// the iframe's `load` event and Preact's first paint. Double-rAF guarantees the
// browser has committed a frame before we signal. Harmless if there's no parent
// (standalone). The host also has a fallback timer, so a webview that never
// calls this still un-covers.
export function notifyHostReady(): void {
  try {
    requestAnimationFrame(() => requestAnimationFrame(() => {
      try { parent.postMessage({ __kaminReady: true }, "*") } catch { /* no parent */ }
    }))
  } catch { /* rAF unavailable */ }
  installCrashPong()
}

// Answer the host's liveness ping so it can tell a CRASHED webview render process
// (WebView2 "sad face" — the whole Bridge-iframe process died, e.g. OOM on a huge
// session) from a live one, and auto-reload the dead frame instead of leaving the
// user stuck. A pong only lands if this frame's JS event loop is alive; a crashed
// process can't reply, so the host reloads it (WebviewPanelView crash-watchdog).
/** What THIS frame retains. `performance.memory.usedJSHeapSize` cannot answer
 *  that: it reports the whole isolate, and every Bridge panel is same-origin, so
 *  all of them ride in ONE renderer process and report the IDENTICAL number. A
 *  freeze report showing `chat=602MB console=602MB plan=602MB` is one 602MB heap
 *  seen three times, not three copies — the per-frame heap it was added to
 *  attribute is unattributable by construction. Counts of what the frame itself
 *  holds are attributable, so report those and label the heap as shared. */
export interface FrameRetention {
  role?: string
  tabs?: number
  entries?: number
}

let retentionProvider: (() => FrameRetention) | undefined

/** Registered by whichever root owns a session store (only chat holds a full
 *  one). Frames that register nothing report just the shared heap. */
export function setFrameRetentionProvider(fn: () => FrameRetention): void {
  retentionProvider = fn
}

/** Tell the host a long, deliberate operation is running in this frame.
 *
 *  The host's crash-watchdog reloads a frame that misses enough liveness pings,
 *  which is right for a dead render process and WRONG here: exporting a big
 *  transcript could stall the frame past the limit, and the reload then threw
 *  away the mounted session and forced a full replay of tens of thousands of
 *  entries. The user saw "the iframe reloads every time I download the log".
 *  The frame announces the operation BEFORE it starts, so the announcement
 *  cannot itself be blocked by the stall it is warning about. */
export function setHostBusy(busy: boolean): void {
  try { parent.postMessage({ __kaminBusy: busy === true }, "*") } catch { /* no parent */ }
}

let crashPongInstalled = false
function installCrashPong(): void {
  if (crashPongInstalled) return
  crashPongInstalled = true
  try {
    window.addEventListener("message", (e: MessageEvent) => {
      const d = e.data as { __kaminPing?: unknown } | null
      if (d && d.__kaminPing) {
        // The pong already flows every 4s, so the memory report rides it and
        // needs no new channel. `heapMB` is the SHARED renderer heap (see
        // FrameRetention above — identical from every panel); `retention` is
        // what this frame alone is holding, which is what a memory report
        // actually needs to name a culprit.
        // `performance.memory` is Chromium-only and not in TS's lib — hence the
        // cast. Absent → omit the field rather than report a zero.
        let heapMB: number | undefined
        try {
          const mem = (performance as unknown as { memory?: { usedJSHeapSize?: number } }).memory
          if (mem?.usedJSHeapSize) heapMB = Math.round(mem.usedJSHeapSize / 1_048_576)
        } catch { /* not exposed */ }
        let retention: FrameRetention | undefined
        try { retention = retentionProvider?.() } catch { /* provider threw — omit */ }
        try { parent.postMessage({ __kaminPong: true, heapMB, retention }, "*") } catch { /* no parent */ }
      }
    })
  } catch { /* no window */ }
}
