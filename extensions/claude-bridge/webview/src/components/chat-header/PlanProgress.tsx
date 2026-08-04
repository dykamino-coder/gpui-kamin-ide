import type { JSX } from 'preact'
import { useSignal, useSignalEffect } from '@preact/signals'
import { activeTabId } from '../../signals/tabs'
import { activePlan } from '../../signals/plan'
import { replayProgressByTab } from '../../signals/replay-progress'
import { showToast } from '../../signals/toasts'

// Долго читать не нужно, совпадает с остальными completion-тостами.
const PLAN_COMPLETE_TOAST_MS = 4000

/** Compact "N/M tasks" badge in the header. Reads the shared `activePlan`
 *  signal so it stays in sync with the RightPanel (same ID resolution,
 *  same deleted-tombstone handling). Fires a single toast when a plan
 *  flips partial → complete. */
export function PlanProgress(): JSX.Element | null {
  // Per-tab last seen ratio. Toast fires ONLY on a LIVE transition to
  // complete: the first settled snapshot of a tab is a baseline — a plan
  // that was already finished before the chat was opened/switched-to is
  // old news, not an event (user feedback).
  const seen = useSignal<Map<string, string>>(new Map())
  useSignalEffect(() => {
    const id = activeTabId.value
    const p = activePlan.value
    // While the transcript is still replaying, `activePlan` walks a partial
    // history and flickers through every PAST count — don't celebrate a
    // transient "all done" that a mid-replay snapshot briefly shows.
    if (!id || !p || replayProgressByTab.value.get(id) != null) return
    const key = `${p.completed}/${p.total}`
    const prev = seen.value.get(id)
    if (prev === key) return
    const next = new Map(seen.value)
    next.set(id, key)
    seen.value = next
    if (prev === undefined) return // baseline, not a transition
    if (p.allDone && p.total > 0) {
      showToast({
        type: 'success',
        title: 'Plan complete',
        message: `All ${p.total} task${p.total === 1 ? '' : 's'} finished`,
        duration: PLAN_COMPLETE_TOAST_MS,
      })
    }
  })

  // Hide the badge entirely during replay: the count is rebuilt from a growing
  // partial stream, so it appears/disappears and counts up through every past
  // task. Show it only once the transcript has settled (replay cleared to null).
  const tid = activeTabId.value
  if (tid && replayProgressByTab.value.get(tid) != null) return null

  const p = activePlan.value
  if (!p) return null

  const done = p.allDone
  const color = done ? 'var(--accent-green)' : p.inProgress > 0 ? 'var(--accent-yellow)' : 'var(--accent-primary)'
  const bg = done ? 'var(--tint-green-medium)' : p.inProgress > 0 ? 'var(--tint-yellow-medium)' : 'var(--tint-primary-medium)'

  return (
    <div
      data-tooltip={`Plan: ${p.completed}/${p.total} completed${p.inProgress > 0 ? ` · ${p.inProgress} in progress` : ''}`}
      style={`display:inline-flex;align-items:center;gap:6px;margin-left:10px;padding:4px 10px;border-radius:var(--radius-xs);background:${bg};color:${color};font-size:11px;font-weight:600;letter-spacing:0.02em`}
    >
      <i aria-hidden="true" class={done ? 'fas fa-check-double' : p.inProgress > 0 ? 'fas fa-circle-half-stroke' : 'fas fa-list-check'} style="font-size:11px" />
      <span style="font-variant-numeric:tabular-nums">
        {p.completed}/{p.total}
      </span>
      {done && <span style="text-transform:uppercase;font-size:9px;letter-spacing:0.05em">done</span>}
    </div>
  )
}
