import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { jsonlEntriesByTab } from '../../signals/jsonl'

// The number an OOM report leads with: the shared WebView2 renderer heap, and
// what THIS chat frame is retaining (entries × tabs) — the two figures from
// `render process died (OUT OF MEMORY) — shared heap: …MB; retained: …`. A live
// readout next to the diagnostic dump so a user can see how close a marathon
// session is to the ~4GB renderer ceiling BEFORE it dies, not only after.
function readHeapMB(): number | undefined {
  try {
    // Chromium-only, absent from TS's lib and gated off in some builds — cast +
    // guard, omit rather than report a misleading zero.
    const mem = (performance as unknown as { memory?: { usedJSHeapSize?: number } }).memory
    if (mem?.usedJSHeapSize) return Math.round(mem.usedJSHeapSize / 1_048_576)
  } catch { /* not exposed */ }
  return undefined
}

interface Snapshot {
  heapMB?: number
  totalEntries: number
  tabs: { id: string; entries: number }[]
}

function snapshot(): Snapshot {
  const tabList = [...jsonlEntriesByTab.value.entries()]
    .map(([id, e]) => ({ id, entries: e.length }))
    .sort((a, b) => b.entries - a.entries)
  return {
    heapMB: readHeapMB(),
    totalEntries: tabList.reduce((n, t) => n + t.entries, 0),
    tabs: tabList,
  }
}

const row = 'display:flex;justify-content:space-between;gap:16px'

/** ⓘ button next to the diagnostic dump: current renderer heap + retained
 *  chat entries/tabs, sampled fresh each time the popover opens. */
export function MemoryInfoButton(): JSX.Element {
  const [snap, setSnap] = useState<Snapshot | null>(null)
  const toggle = (): void => { setSnap((cur) => (cur ? null : snapshot())) }

  return (
    <div style="position:relative;display:inline-flex">
      <button
        class="header-btn"
        data-tooltip="Memory: renderer heap + retained chat entries"
        type="button"
        onClick={toggle}
      >
        <i class="fas fa-circle-info" />
      </button>
      {snap && (
        <div
          role="dialog"
          style="position:absolute;top:100%;right:0;margin-top:6px;z-index:var(--z-titlebar-popover,10001);min-width:230px;padding:10px 12px;background:var(--bg-surface);border:1px solid var(--divider-soft);border-radius:var(--radius-sm);box-shadow:var(--shadow-md);font-size:11px;color:var(--text-secondary);line-height:1.5"
        >
          <div style={`${row};margin-bottom:5px`}>
            <span>Renderer heap (shared)</span>
            <b style="color:var(--text-primary)">{snap.heapMB != null ? `${snap.heapMB} MB` : 'n/a'}</b>
          </div>
          <div style={`${row};margin-bottom:6px`}>
            <span>Chat retained</span>
            <b style="color:var(--text-primary)">{snap.totalEntries} entries · {snap.tabs.length} tabs</b>
          </div>
          {snap.tabs.slice(0, 6).map((t) => (
            <div key={t.id} style={`${row};color:var(--text-muted)`}>
              <span style="max-width:150px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap">{t.id}</span>
              <span style="font-variant-numeric:tabular-nums">{t.entries}</span>
            </div>
          ))}
          <div style="margin-top:8px;font-size:10px;color:var(--text-disabled)">
            The renderer dies near a ~4GB heap — this is the figure an OOM report names.
          </div>
        </div>
      )}
    </div>
  )
}
