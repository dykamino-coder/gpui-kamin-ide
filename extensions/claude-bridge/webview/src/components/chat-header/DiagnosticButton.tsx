import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { useBridge } from '../../hooks/useBridge'
import { compactSegments, activeSegmentIdx } from '../../signals/compact-segments'
import { collectSessionDiag, jsonlEntriesByTab, getJsonlStructureVersion } from '../../signals/jsonl'
import { getRenderModel } from '../jsonl-viewer/derived-cache'

interface DiagnosticButtonProps {
  tabId: string
}

/** What the VIEWER thinks it should draw, next to what the store holds.
 *
 *  This is the half a store dump can't answer. A real report — "the chat is
 *  empty" — came with 16,321 entries present, `_ord` monotonic, and every drop
 *  benign: the data was all there, so the fault was somewhere in visibility,
 *  and the dump could only hint. This runs the SAME pipeline the viewer runs
 *  (getRenderModel → merge/group/chain/visibility) and records what survives
 *  each stage, so an empty chat names its own cause: either `visibleMerged` is
 *  empty (a filter ate it) or it's full of types that render null (the window
 *  is full of bookkeeping) — those need opposite fixes.
 *
 *  Collected HERE rather than in collectSessionDiag because compact-segments
 *  imports jsonl; pulling it into jsonl would close a module cycle. */
function collectRenderDiag(tabId: string): unknown {
  try {
    const entries = jsonlEntriesByTab.value.get(tabId) ?? []
    const segs = compactSegments.value
    const idx = activeSegmentIdx.value
    const model = getRenderModel(tabId, entries, segs, idx, getJsonlStructureVersion(tabId))
    const byType: Record<string, number> = {}
    for (const e of model.visibleMerged) byType[e.type] = (byType[e.type] ?? 0) + 1
    return {
      activeSegmentIdx: idx,
      segments: segs.map((s) => ({ from: s.from, to: s.to, ts: s.ts, visibleCount: s.visibleCount })),
      mergedCount: model.merged.length,
      visibleMergedCount: model.visibleMerged.length,
      visibleByType: byType,
      // The tail is what the render window actually mounts. `type`/`uuid` alone
      // can't tell an empty bubble from a full one — so also record a text
      // preview and the streaming flag. A final answer that "shows in the
      // console but not the chat" appears here as a trailing `assistant` whose
      // `streaming` is still true with an empty `preview`: the stub never took
      // the canonical text (the exact shape of the reported bug).
      visibleTail: model.visibleMerged.slice(-40).map((e) => {
        const c = (e as { message?: { content?: unknown } }).message?.content
        let preview = ''
        if (typeof c === 'string') preview = c.slice(0, 80)
        else if (Array.isArray(c)) {
          const t = c.find((b) => (b as { type?: string })?.type === 'text') as { text?: string } | undefined
          if (t?.text) preview = t.text.slice(0, 80)
        }
        return { type: e.type, uuid: e.uuid, streaming: (e as { __streaming?: unknown }).__streaming ?? false, preview }
      }),
      ver: model.ver,
    }
  } catch (err) {
    return { error: err instanceof Error ? err.message : String(err) }
  }
}

// #78 diagnostic: dumps this session's chat store (row summaries + parent links)
// + the recent drop log + what the viewer's own pipeline makes of it.
//
// Saves to a FILE, not the clipboard: on the sessions this bug actually happens
// to, the dump is megabytes of JSON — awkward to paste, easy to truncate, and
// lost the moment anything else is copied. A file attaches to a report as-is.
export function DiagnosticButton({ tabId }: DiagnosticButtonProps): JSX.Element {
  const bridge = useBridge()
  const [saved, setSaved] = useState(false)
  const dump = async (): Promise<void> => {
    const json = JSON.stringify({
      ...(collectSessionDiag(tabId) as Record<string, unknown>),
      render: collectRenderDiag(tabId),
    }, null, 2)
    const name = `session-diag-${tabId}-${new Date().toISOString().replace(/[:.]/g, '-')}.json`
    const res = await bridge.saveSessionDiag(name, json)
    if (!res.success) return // cancelled or failed — don't claim success
    setSaved(true)
    setTimeout(() => { setSaved(false) }, 1500)
  }
  return (
    <button
      class="header-btn"
      data-tooltip={saved ? 'Diagnostic saved' : 'Save session diagnostic (#78)'}
      type="button"
      onClick={() => { void dump() }}
    >
      <i class={saved ? 'fas fa-check' : 'fas fa-stethoscope'} />
    </button>
  )
}
