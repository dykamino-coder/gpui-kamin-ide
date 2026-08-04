import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../../hooks/useBridge'
import { showToast } from '../../../signals/toasts'
import { setHostBusy } from '../../../lib/host-ready'

type Phase =
  | { k: 'idle' }
  | { k: 'busy'; done: number; total: number; pct: number }
  | { k: 'done'; count: number }

/** Bulk-export every transcript tied to the current token into a chosen folder.
 *  The extension loops the token's sessions and pulls each JSONL in byte-range
 *  batches (so a big transcript can't stall the one-shot path); this button owns
 *  the folder prompt's outcome + per-file progress. */
export function ExportTranscriptsButton(): JSX.Element {
  const bridge = useBridge()
  const [phase, setPhase] = useState<Phase>({ k: 'idle' })

  useEffect(() => bridge.onJsonlDownloadAllProgress((p) => {
    setPhase((cur) => cur.k === 'busy'
      ? { k: 'busy', done: p.fileIndex, total: p.fileTotal, pct: p.size > 0 ? Math.min(100, Math.floor((p.bytes / p.size) * 100)) : 0 }
      : cur)
  }), [])

  const run = async (): Promise<void> => {
    if (phase.k === 'busy') return
    setPhase({ k: 'busy', done: 0, total: 0, pct: 0 })
    // A long export can stall this frame; hold off the crash-watchdog reload.
    setHostBusy(true)
    let res: { success: boolean; dir?: string; count?: number; error?: string }
    try {
      res = await bridge.downloadAllJsonl()
    } finally {
      setHostBusy(false)
    }
    if (!res.success && res.error === 'Cancelled') { setPhase({ k: 'idle' }); return }
    if (res.success) {
      setPhase({ k: 'done', count: res.count ?? 0 })
      showToast({ type: 'success', title: 'Export complete', message: `${String(res.count ?? 0)} session log(s) saved to ${res.dir ?? 'the folder'}.` })
    } else {
      setPhase({ k: 'idle' })
      showToast({ type: 'error', title: 'Export failed', message: res.error ?? 'Unknown error' })
    }
    setTimeout(() => { setPhase((c) => (c.k === 'done' ? { k: 'idle' } : c)) }, 5000)
  }

  const busy = phase.k === 'busy'
  const label = busy
    ? (phase.total > 0 ? `Exporting ${String(phase.done)}/${String(phase.total)} · ${String(phase.pct)}%` : 'Preparing export…')
    : phase.k === 'done' ? `Saved ${String(phase.count)} log(s)` : 'Download all session logs'
  const icon = busy ? 'fas fa-spinner fa-spin' : phase.k === 'done' ? 'fas fa-check' : 'fas fa-download'

  return (
    <button type="button" class="btn" disabled={busy} onClick={() => { void run() }}>
      <i class={icon} style="margin-right:6px" />{label}
    </button>
  )
}
