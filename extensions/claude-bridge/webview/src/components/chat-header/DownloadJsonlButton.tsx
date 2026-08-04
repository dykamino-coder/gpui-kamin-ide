import type { JSX } from 'preact'
import { useState, useEffect } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId } from '../../signals/tabs'
import { setHostBusy } from '../../lib/host-ready'

interface DownloadJsonlButtonProps {
  /** Resolves with the outcome — the button OWNS reporting it. */
  onDownload: () => Promise<{ success: boolean; error?: string }>
}

type Phase =
  | { k: 'idle' }
  /** `pct` is undefined until the first chunk lands — a small transcript still
   *  arrives as ONE frame and never reports progress at all. */
  | { k: 'busy'; pct?: number }
  | { k: 'done' }
  | { k: 'failed'; msg: string }

const RESULT_LINGER_MS = 4000

/** Download the session transcript. Shows progress and, crucially, FAILURE.
 *
 *  It used to be `void bridge.downloadJsonl(tabId)` — the promise discarded. So
 *  when the export of a big session failed (it crosses as ONE WS message and hit
 *  a 30s timeout), the button did nothing at all: no file, no error, no hint.
 *  The user's report was literally "the jsonl of this chat never downloaded to
 *  the folder". A control that can fail must be able to say so. */
export function DownloadJsonlButton({ onDownload }: DownloadJsonlButtonProps): JSX.Element {
  const [phase, setPhase] = useState<Phase>({ k: 'idle' })
  const bridge = useBridge()

  // Real bytes from the server's chunked transfer — a spinner on a 36MB export
  // says nothing about whether it is moving or wedged.
  useEffect(() => {
    const unsub = bridge.onJsonlDownloadProgress((tabId: string, p: { received: number; total: number }) => {
      if (tabId !== activeTabId.value || p.total <= 0) return
      setPhase((cur) => (cur.k === 'busy'
        ? { k: 'busy', pct: Math.min(99, Math.floor((p.received / p.total) * 100)) }
        : cur))
    })
    return unsub
  }, [])

  const run = async (): Promise<void> => {
    if (phase.k === 'busy') return // a second click would race two transfers
    setPhase({ k: 'busy' })
    // Hold off the crash-watchdog: a big export can stall this frame, and a
    // reload mid-download costs the mounted session and a full replay.
    setHostBusy(true)
    let res: { success: boolean; error?: string }
    try {
      res = await onDownload()
    } finally {
      setHostBusy(false)
    }
    // Cancelling the save dialog is not a failure — say nothing.
    if (!res.success && res.error === 'Cancelled') { setPhase({ k: 'idle' }); return }
    setPhase(res.success ? { k: 'done' } : { k: 'failed', msg: res.error ?? 'Download failed' })
    setTimeout(() => { setPhase({ k: 'idle' }) }, RESULT_LINGER_MS)
  }

  const icon = phase.k === 'busy' ? 'fas fa-spinner fa-spin'
    : phase.k === 'done' ? 'fas fa-check'
      : phase.k === 'failed' ? 'fas fa-triangle-exclamation'
        : 'fas fa-download'
  const tooltip = phase.k === 'busy' ? (phase.pct === undefined ? 'Downloading session log…' : `Downloading session log — ${String(phase.pct)}%`)
    : phase.k === 'done' ? 'Session log saved'
      : phase.k === 'failed' ? phase.msg
        : 'Download session log'

  return (
    <button
      class="header-btn"
      data-tooltip={tooltip}
      type="button"
      disabled={phase.k === 'busy'}
      style={phase.k === 'failed' ? 'color:var(--accent-red)' : undefined}
      onClick={() => { void run() }}
    >
      {phase.k === 'busy' && phase.pct !== undefined
        ? <span class="download-pct">{phase.pct}%</span>
        : <i class={icon} />}
    </button>
  )
}
