// Reading live state for the command API, and the one distinction that matters:
// "the turn ended" is NOT "the view settled".
//
// A compaction ends the turn, then clears the store and re-replays — for a
// second or two afterwards the entry count and the segment strip are mid-
// rebuild. A driver that stops at `!working` reads that gap and concludes the
// feature is broken (it did, once, and the report was wrong). `settling` names
// the gap so nothing has to infer it.
//
// The shape + the pure helpers live in `command-model.ts`; only this file
// touches signals.
import { activeTabId } from '../signals/tabs'
import { boundTabId } from '../signals/session-binding'
import { mcpLoading, tabPromptReady } from '../signals/connection'
import { tabActivity } from '../signals/ui'
import { replayProgressByTab } from '../signals/replay-progress'
import { jsonlEntriesByTab, archivedViewTs } from '../signals/jsonl'
import { compactSegments, activeSegmentIdx } from '../signals/compact-segments'
import { localQueue } from '../signals/queue'
import type { BridgeState } from './command-model'

export function snapshot(): BridgeState {
  const id = activeTabId.value
  const replayPct = id ? (replayProgressByTab.value.get(id) ?? null) : null
  const working = id ? (tabActivity.value.get(id)?.isWorking ?? false) : false
  const promptReady = id ? (tabPromptReady.value.get(id) ?? false) : false
  const settling = id === null || boundTabId.value !== id || replayPct !== null || mcpLoading.value
  return {
    activeTabId: id,
    boundTabId: boundTabId.value,
    mcpLoading: mcpLoading.value,
    promptReady,
    working,
    replayPct,
    entryCount: id ? (jsonlEntriesByTab.value.get(id)?.length ?? 0) : 0,
    segmentCount: compactSegments.value.length,
    activeSegment: activeSegmentIdx.value,
    archived: id ? archivedViewTs(id) !== undefined : false,
    settling,
    busy: settling || working || !promptReady,
    queued: id ? (localQueue.value.get(id)?.length ?? 0) : 0,
  }
}
