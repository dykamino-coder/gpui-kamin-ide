// Opening a compact segment, shared by the strip's click handler and the command
// API. Only the state transitions live here; the strip keeps its own UI bits
// (closing the overflow menu, scrolling the pill into view), which a scripted
// call has no business doing.
import type { ElectronBridge } from '../../shared/types'
import type { DisplaySegment } from '../signals/compact-segments'
import { setActiveSegment } from '../signals/compact-segments'
import { clearJsonlEntries, setArchivedView, archivedViewTs } from '../signals/jsonl'
import { activeTabId } from '../signals/tabs'

/** Show `seg`. Resident (in-window) → just move the index. Out-of-window → load
 *  it from the mirror as an archived view. Latest → back to the live Current
 *  view, reloading if we were parked in an archived one. */
export function openDisplaySegment(
  bridge: ElectronBridge, seg: DisplaySegment, totalSegments: number,
): void {
  const id = activeTabId.value
  if (!id) return
  const wasArchived = archivedViewTs(id) !== undefined
  if (seg.isLatest) {
    if (wasArchived) {
      setArchivedView(id, null)
      clearJsonlEntries(id)
      try { bridge.requestJsonlReplay(id) } catch { /* offline — nothing to reload */ }
    }
    setActiveSegment(seg.residentIdx >= 0 ? seg.residentIdx : totalSegments - 1)
    return
  }
  if (seg.resident && seg.residentIdx >= 0 && !wasArchived) {
    setActiveSegment(seg.residentIdx) // in-window: no load needed
    return
  }
  // Out-of-window (or switching between archived views): load it whole by ts
  // range. The gate holds live entries off; onSegmentPage replaces the window.
  setArchivedView(id, seg.ts ?? '')
  try { bridge.loadSegment(id, seg.fromTs, seg.toTs) } catch { setArchivedView(id, null) }
}
