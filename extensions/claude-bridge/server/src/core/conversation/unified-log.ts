// The single normalized transcript of a conversation that spans engines
// (manifest.ts). It is an ordinary Bridge transcript file — one JsonlEntry per
// line — so the client's replay, paging and local mirror work on it unchanged.
//
// Entries keep their uuids. Each segment's native transcript starts its own
// parent chain, so the first entry of a segment (and any entry whose parent the
// log does not hold) is re-parented onto the log's last entry: the client walks
// one chain from the newest entry back, and a segment left on its own chain
// would read as an abandoned branch and vanish from the chat.

import fs from 'node:fs'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import type { JsonlEntry } from '../../shared/jsonl-types'
import type { AgentEngineId } from '../../shared/agent-events'

type Entry = JsonlEntry & { uuid?: string; parentUuid?: string | null; subtype?: string }

export const ENGINE_SWITCH_SUBTYPE = 'engine_switch'

/** Last uuid in a transcript file (scans back from the end). */
export function lastUuidIn(file: string): string | null {
  let raw: string
  try {
    raw = fs.readFileSync(file, 'utf8')
  } catch {
    return null
  }
  const lines = raw.trimEnd().split('\n')
  for (let i = lines.length - 1; i >= 0; i--) {
    try {
      const uuid = (JSON.parse(lines[i]!) as { uuid?: unknown }).uuid
      if (typeof uuid === 'string' && uuid) return uuid
    } catch {
      /* torn line */
    }
  }
  return null
}

export class UnifiedLog {
  private last: string | null
  /** Uuids of the segment being copied: parents inside it are kept. */
  private segment = new Set<string>()

  constructor(readonly file: string) {
    this.last = lastUuidIn(file)
  }

  /** A new segment starts: its entries chain onto what the log holds now. */
  beginSegment(): void {
    this.segment = new Set()
  }

  append(entries: readonly JsonlEntry[]): void {
    if (entries.length === 0) return
    const lines: string[] = []
    for (const raw of entries) {
      const e = { ...(raw as Entry) }
      if (typeof e.uuid === 'string' && e.uuid) {
        const keepsParent =
          e.subtype === 'compact_boundary' || (typeof e.parentUuid === 'string' && this.segment.has(e.parentUuid))
        if (!keepsParent && !e.isSidechain) e.parentUuid = this.last
        this.segment.add(e.uuid)
        if (!e.isSidechain) this.last = e.uuid
      }
      lines.push(JSON.stringify(e))
    }
    fs.mkdirSync(path.dirname(this.file), { recursive: true })
    fs.appendFileSync(this.file, lines.join('\n') + '\n')
  }

  /** "→ GPT-6.1-Sol": where the conversation moved to another engine. */
  appendSwitchMarker(
    to: { engine: AgentEngineId; model: string | null; label: string },
    sessionId: string,
  ): JsonlEntry {
    const marker = {
      type: 'system',
      subtype: ENGINE_SWITCH_SUBTYPE,
      uuid: randomUUID(),
      parentUuid: this.last,
      timestamp: new Date().toISOString(),
      sessionId,
      engine: to.engine,
      model: to.model,
      level: 'info',
      content: `→ ${to.label}`,
    } as unknown as JsonlEntry
    this.beginSegment()
    this.append([marker])
    return marker
  }
}

/** First segment of a conversation that becomes multi-engine: the log starts
 *  as a byte copy of the chat's transcript so far. */
export function seedUnifiedLog(file: string, source: string): void {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  let data = ''
  try {
    data = fs.readFileSync(source, 'utf8')
  } catch {
    /* an empty chat has no transcript yet */
  }
  // A torn last line (process killed mid-write) is dropped, not copied.
  const end = data.lastIndexOf('\n')
  fs.writeFileSync(file, end >= 0 ? data.slice(0, end + 1) : '', { mode: 0o600 })
}
