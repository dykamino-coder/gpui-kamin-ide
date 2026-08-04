import { computed } from '@preact/signals'
import { jsonlEntriesByTab, getJsonlStructureVersion } from './jsonl'
import { getToolResults } from '../lib/tool-result-cache'
import { activeTabId } from './tabs'
import { stripMcpPrefix } from '../lib/mcp-tool-name'

export type FileOpKind = 'edit' | 'write' | 'multiedit' | 'read'

export interface FileEntry {
  path: string
  ops: number
  lastOp: FileOpKind
  lastAt: string | undefined
  /** Most common operation kind across this file's touches — useful for
   *  icon pick when user mixed Read + Edit + Write in the same file. */
  primaryOp: FileOpKind
}

/** Heuristic: did this tool_result represent a failed call? CLI marks
 *  hard errors with `is_error:true`, but some failures (file-not-found,
 *  permission denied) come back as plain text starting with `Error:` or
 *  wrapped in a `<tool_use_error>` block. Filter all of these out so the
 *  Files panel doesn't list ghost reads/writes the agent never actually
 *  performed. */
function isFailedResult(result: { content: string; isError: boolean } | undefined): boolean {
  if (!result) return false
  if (result.isError) return true
  const c = result.content
  if (!c) return false
  if (c.startsWith('<tool_use_error>')) return true
  if (/^Error:/.test(c.trim())) return true
  return false
}

/** List of files touched by tool calls in the active tab's JSONL. Sorted
 *  by recency (last touch first). Write > Edit/MultiEdit > Read in
 *  precedence for the `primaryOp` display — reads are informational, the
 *  right-panel Files tab should highlight modifications. Failed calls
 *  (file-not-found errors, permission denials, is_error=true) are
 *  filtered out — they don't represent files the agent actually
 *  read/wrote, so listing them just clutters the Read tab. */
export const activeFiles = computed<FileEntry[]>(() => {
  const id = activeTabId.value
  if (!id) return []
  const entries = jsonlEntriesByTab.value.get(id) ?? []

  // tool_use_id → result text. This used to be walked here, inline, with a
  // comment saying it was kept local to avoid pulling a viewer util into a
  // signals computed. That was the wrong trade once measured: the walk touches
  // the body of every record, this computed re-runs on every store change, and
  // on a real 31k-entry session it cost 26ms per arriving record — while the
  // viewer was building a byte-identical map right next to it. One shared,
  // incrementally-appended map now serves both.
  const results = getToolResults(id, entries, getJsonlStructureVersion(id)).map

  const map = new Map<string, { ops: Map<FileOpKind, number>; lastOp: FileOpKind; lastAt: string | undefined }>()

  for (const e of entries) {
    if (e.type !== 'assistant') continue
    const ts: string | undefined = (e as any).timestamp
    const content = (e as any).message?.content
    if (!Array.isArray(content)) continue
    for (const b of content) {
      if (b?.type !== 'tool_use') continue
      const shortName = stripMcpPrefix(String(b.name || ''))
      let kind: FileOpKind | null = null
      if (shortName === 'Edit') kind = 'edit'
      else if (shortName === 'MultiEdit') kind = 'multiedit'
      else if (shortName === 'Write') kind = 'write'
      else if (shortName === 'Read') kind = 'read'
      if (!kind) continue
      const p = b.input?.file_path
      if (typeof p !== 'string' || !p) continue
      // Skip failed calls — those don't count as the agent having
      // touched the file.
      if (isFailedResult(results?.get(b.id))) continue
      let rec = map.get(p)
      if (!rec) {
        rec = { ops: new Map(), lastOp: kind, lastAt: ts }
        map.set(p, rec)
      }
      rec.ops.set(kind, (rec.ops.get(kind) ?? 0) + 1)
      rec.lastOp = kind
      rec.lastAt = ts
    }
  }

  const out: FileEntry[] = []
  for (const [path, rec] of map) {
    const totalOps = [...rec.ops.values()].reduce((a, b) => a + b, 0)
    // Precedence: write > multiedit > edit > read
    const primary: FileOpKind =
      rec.ops.get('write') ? 'write'
      : rec.ops.get('multiedit') ? 'multiedit'
      : rec.ops.get('edit') ? 'edit'
      : 'read'
    out.push({ path, ops: totalOps, lastOp: rec.lastOp, lastAt: rec.lastAt, primaryOp: primary })
  }

  out.sort((a, b) => {
    const ta = a.lastAt ? Date.parse(a.lastAt) : 0
    const tb = b.lastAt ? Date.parse(b.lastAt) : 0
    return tb - ta
  })
  return out
})
