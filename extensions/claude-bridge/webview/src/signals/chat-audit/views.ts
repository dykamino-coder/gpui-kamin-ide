// Two projections of the same transcript, reduced to the user/assistant
// messages a reader sees, so the audit can compare them exactly:
//  - cliView: what Claude Code shows on resume (reference model, see
//    extensions/claude-bridge/docs/chat-pipeline.md "CLI reference behaviour" for the sources);
//  - chatView: what the Bridge chat renders — the REAL store, render model,
//    segment selection and row renderer, not a re-implementation.
import type { ContentBlock, JsonlEntryData } from '../../types/jsonl'
import type { Rec } from './transcript'
import { getJsonlEntries, getJsonlStructureVersion } from '../jsonl'
import { compactSegments, activeSegmentIdx } from '../compact-segments'
import { getRenderModel } from '../../components/jsonl-viewer/derived-cache'
import { JsonlEntry } from '../../components/jsonl-viewer/JsonlEntry'
import { selectDrawableWindow } from '../../components/jsonl-viewer/render-window'
import { isSyntheticUserText } from '../../components/jsonl-viewer/utils'

const CHAIN_TYPES = new Set(['user', 'assistant', 'system', 'attachment'])

function textOf(content: unknown): string {
  if (typeof content === 'string') return content
  if (!Array.isArray(content)) return ''
  return (content as ContentBlock[]).filter((b) => b?.type === 'text').map((b) => b.text ?? '').join('\n')
}

function isToolResultOnly(content: unknown): boolean {
  return Array.isArray(content) && content.length > 0
    && (content as ContentBlock[]).every((b) => b?.type === 'tool_result')
}

/** Append `U:`/`A:` lines; consecutive rows of one assistant message fold into one line. */
function emit(out: string[], lastMsg: { id?: string }, e: JsonlEntryData): void {
  if (e.type === 'user') {
    lastMsg.id = undefined
    const c = e.message?.content
    if (e.isMeta || e.isCompactSummary || isToolResultOnly(c)) return
    const t = textOf(c).trim()
    if (t && !isSyntheticUserText(t)) out.push(`U:${t}`)
    return
  }
  if (e.type !== 'assistant') return
  const t = textOf(e.message?.content)
  if (!t.trim() || isSyntheticUserText(t)) return
  const id = e.message?.id
  if (id && lastMsg.id === id && out.length > 0) out[out.length - 1] += `\n${t}`
  else out.push(`A:${t}`)
  lastMsg.id = id
}

/** Reference: leaf = last main-chain record in file order (or last-prompt.leafUuid),
 *  walk parentUuid to the root; a compact boundary (parentUuid null) ends the walk.
 *  A duplicated uuid resolves to its LAST copy. */
export function cliView(recs: readonly Rec[]): string[] {
  const byUuid = new Map<string, Rec>()
  let leaf: string | undefined
  for (const r of recs) {
    if (r.type === 'last-prompt' && r.leafUuid) { leaf = r.leafUuid; continue }
    if (!r.uuid) continue
    byUuid.set(r.uuid, r)
    if (CHAIN_TYPES.has(r.type) && !r.isSidechain) leaf = r.uuid
  }
  const chain: Rec[] = []
  for (let cur = leaf ? byUuid.get(leaf) : undefined; cur; cur = cur.parentUuid ? byUuid.get(cur.parentUuid) : undefined) {
    chain.push(cur)
    if (chain.length > recs.length) break // cycle guard
  }
  chain.reverse()
  const out: string[] = []
  const last = {}
  for (const r of chain) emit(out, last, r)
  return out
}

/** Every row the chat can show for `tabId` (the whole visible projection of the
 *  selected segment), plus the rows mounted in the initial render window. */
export function chatView(tabId: string, cap = 150): { all: string[]; window: string[] } {
  const entries = getJsonlEntries(tabId)
  const model = getRenderModel(tabId, entries, compactSegments.value, activeSegmentIdx.value, getJsonlStructureVersion(tabId))
  const draw = (e: JsonlEntryData) => JsonlEntry({ entry: e, toolResults: model.toolResults })
  const flatten = (rows: JsonlEntryData[]): string[] => {
    const out: string[] = []
    const last = {}
    const walk = (e: JsonlEntryData): void => {
      const group = (e as { _groupEntries?: JsonlEntryData[] })._groupEntries
      if (e.type === 'tool_group' && group) { for (const g of group) walk(g); return }
      emit(out, last, e)
    }
    for (const e of rows) walk(e)
    return out
  }
  const drawn = model.visibleMerged.filter((e) => draw(e) != null)
  const win = selectDrawableWindow(model.visibleMerged, cap, draw)
  return { all: flatten(drawn), window: flatten(win.drawn.map((d) => (d.value.props as { entry: JsonlEntryData }).entry)) }
}

/** `part` is a contiguous run of `whole` ending at its last element. */
export function isSuffix(part: readonly string[], whole: readonly string[]): boolean {
  if (part.length > whole.length) return false
  const off = whole.length - part.length
  return part.every((v, i) => v === whole[off + i])
}
