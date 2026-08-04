// Search APIs over the file index. Two surfaces:
//
//   findFile(query)      — Quick Open (Ctrl+P). Fuzzy basename match
//                          with simple position-aware scoring.
//   findInFiles(query)   — Find in Files (Ctrl+Shift+F). Straight
//                          substring scan + line snippet. Capped to
//                          200 hits / 5000 files scanned per call.
//
// Phase B+ swaps `findInFiles` to a trigram index for sub-50ms
// queries on 100k+ workspaces; the public shape stays the same so
// the renderer doesn't change.

import { readFile } from "node:fs/promises"
import { getIndex, type IndexedFile } from "./file-index.js"

const FIND_FILE_MAX = 50
const FIND_TEXT_MAX_HITS = 200
const FIND_TEXT_MAX_FILES = 5000
const KB = 1024
const FIND_TEXT_MAX_KB = 256
const FIND_TEXT_MAX_BYTES = FIND_TEXT_MAX_KB * KB
const SCORE_BASENAME_PREFIX = 1000
const SCORE_BASENAME_CONTAINS = 500
const SCORE_PATH_CONTAINS = 100
const SNIPPET_PREFIX = 40
const SNIPPET_SUFFIX = 80

export interface FindFileHit {
  rel: string
  abs: string
  /** Score; UI sorts descending. */
  score: number
}

export interface FindTextHit {
  rel: string
  abs: string
  line: number
  /** Char offset of the match within the snippet, for highlighting. */
  matchStart: number
  matchEnd: number
  snippet: string
}

export function findFile(query: string): FindFileHit[] {
  const q = query.toLowerCase().trim()
  if (q.length === 0) {
    // Empty query → MRU-style: just return the first slice (Phase B+
    // swaps to "recently opened" once the renderer ships that store).
    return getIndex().slice(0, FIND_FILE_MAX).map((f) => ({ rel: f.rel, abs: f.abs, score: 0 }))
  }
  const hits: FindFileHit[] = []
  for (const f of getIndex()) {
    const score = scoreFile(f, q)
    if (score > 0) hits.push({ rel: f.rel, abs: f.abs, score })
  }
  hits.sort((a, b) => b.score - a.score || a.rel.length - b.rel.length)
  return hits.slice(0, FIND_FILE_MAX)
}

function scoreFile(f: IndexedFile, q: string): number {
  const rel = f.rel.toLowerCase()
  const base = baseOf(rel)
  if (base.startsWith(q)) return SCORE_BASENAME_PREFIX - base.length
  if (base.includes(q)) return SCORE_BASENAME_CONTAINS - base.length
  if (rel.includes(q)) return SCORE_PATH_CONTAINS - rel.length
  return 0
}

function baseOf(rel: string): string {
  const i = rel.lastIndexOf("/")
  return i >= 0 ? rel.slice(i + 1) : rel
}

// Bumped on every call; a superseded scan (the user kept typing) bails at its
// next file instead of reading thousands of files to completion — fast typing
// otherwise stacked several ~GB read passes back-to-back on the host loop.
let findGen = 0

export async function findInFiles(query: string): Promise<FindTextHit[]> {
  const q = query
  if (q.length < 2) return []
  const gen = ++findGen
  const out: FindTextHit[] = []
  let scanned = 0
  for (const f of getIndex()) {
    if (gen !== findGen) return out // a newer search superseded this one
    if (scanned >= FIND_TEXT_MAX_FILES) break
    if (out.length >= FIND_TEXT_MAX_HITS) break
    if (f.size > FIND_TEXT_MAX_BYTES) continue
    scanned += 1
    let buf
    try { buf = await readFile(f.abs, "utf8") } catch { continue }
    if (!buf.includes(q)) continue
    const lines = buf.split(/\r?\n/)
    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i]
      if (!line) continue
      const idx = line.indexOf(q)
      if (idx < 0) continue
      const start = Math.max(0, idx - SNIPPET_PREFIX)
      const end = Math.min(line.length, idx + q.length + SNIPPET_SUFFIX)
      const snippet = line.slice(start, end)
      out.push({
        rel: f.rel,
        abs: f.abs,
        line: i + 1,
        matchStart: idx - start,
        matchEnd: idx - start + q.length,
        snippet,
      })
      if (out.length >= FIND_TEXT_MAX_HITS) break
    }
  }
  return out
}
