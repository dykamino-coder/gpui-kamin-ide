// Synthetic, sanitized CLI session transcripts for the chat-render audit.
//
// Every record is generated here: no text, id or timing comes from a real
// session. The shapes follow what Claude Code 2.1.x writes to its session JSONL
// (see extensions/claude-bridge/docs/chat-pipeline.md "CLI reference behaviour"):
//  - one chain linked by parentUuid, each record parented to the previous one;
//  - one API message (message.id) written as one row per content block;
//  - compaction writes a `system/compact_boundary` with parentUuid null plus a
//    `logicalParentUuid`, then an `isCompactSummary` user row;
//  - uuid-less bookkeeping rows (cost-state, api-request, …) interleave freely.
// `_pos` simulates the byte offset the server tags on page/replay delivery.
import type { JsonlEntryData } from '../../types/jsonl'

export type Rec = JsonlEntryData & { logicalParentUuid?: string | null; leafUuid?: string; operation?: string }

type Block =
  | { type: 'text'; text: string }
  | { type: 'thinking'; thinking: string }
  | { type: 'tool_use'; id: string; name: string; input: Record<string, unknown> }

export class TranscriptBuilder {
  readonly recs: Rec[] = []
  private pos = 0
  private t = Date.UTC(2026, 0, 1, 9, 0, 0)
  private seq = 0
  /** Tip of the main chain (the parent of the next chain record). */
  parent: string | undefined

  constructor(private readonly tag = 's') {}

  private id(kind: string): string { return `${this.tag}-${kind}-${String(++this.seq)}` }

  private push(r: Rec): Rec {
    this.t += 1000
    r.timestamp = new Date(this.t).toISOString()
    r._pos = this.pos
    this.pos += JSON.stringify(r).length + 1
    this.recs.push(r)
    return r
  }

  private chain(r: Rec): Rec {
    r.uuid = this.id('u')
    r.parentUuid = this.parent
    this.parent = r.uuid
    return this.push(r)
  }

  user(text: string, extra: Partial<Rec> = {}): Rec {
    return this.chain({ type: 'user', message: { role: 'user', content: text }, ...extra } as Rec)
  }

  /** One assistant API message; `split` writes one row per block (CLI 2.1.x). */
  assistant(blocks: Block[], opts: { split?: boolean; msgId?: string } = {}): string {
    const msgId = opts.msgId ?? this.id('msg')
    const rows = opts.split === false ? [blocks] : blocks.map((b) => [b])
    for (const content of rows) {
      this.chain({ type: 'assistant', message: { id: msgId, role: 'assistant', content } } as unknown as Rec)
    }
    return msgId
  }

  say(text: string, opts: { split?: boolean } = {}): string {
    return this.assistant([{ type: 'thinking', thinking: '' }, { type: 'text', text }], opts)
  }

  /** Prose, then N parallel tool calls in one message, then their results. */
  toolBurst(prose: string, calls: number): string {
    const ids = Array.from({ length: calls }, () => this.id('tool'))
    const msgId = this.assistant([
      { type: 'text', text: prose },
      ...ids.map((id) => ({ type: 'tool_use' as const, id, name: 'Read', input: { file_path: `/w/${id}.txt` } })),
    ])
    for (const id of ids) {
      this.chain({ type: 'user', message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: id, content: `ok ${id}` }] } } as unknown as Rec)
    }
    return msgId
  }

  interrupted(partial: string): string {
    const msgId = this.assistant([{ type: 'text', text: partial }])
    this.user('[Request interrupted by user]')
    return msgId
  }

  meta(text: string): Rec { return this.user(text, { isMeta: true }) }

  /** A subagent row written into the main file (isSidechain), off the main chain. */
  sidechain(text: string): Rec {
    return this.push({ type: 'assistant', uuid: this.id('side'), parentUuid: undefined, isSidechain: true,
      message: { id: this.id('msg'), role: 'assistant', content: [{ type: 'text', text }] } } as unknown as Rec)
  }

  /** uuid-less bookkeeping rows (a cost-state flood, telemetry). */
  bookkeeping(n: number, type = 'cost-state'): void {
    for (let i = 0; i < n; i++) this.push({ type, costUSD: i } as unknown as Rec)
  }

  queueOperation(operation: 'enqueue' | 'dequeue', content: string): void {
    this.push({ type: 'queue-operation', operation, content } as unknown as Rec)
  }

  compact(summary: string): void {
    const boundary = this.push({ type: 'system', subtype: 'compact_boundary', uuid: this.id('u'), parentUuid: undefined,
      logicalParentUuid: this.parent ?? null, content: 'Conversation compacted', compactMetadata: { trigger: 'auto' } } as unknown as Rec)
    this.parent = boundary.uuid
    this.user(summary, { isCompactSummary: true, isMeta: true })
  }

  /** A rewind: continue the chain from an earlier record, abandoning the rest. */
  rewindTo(uuid: string): void { this.parent = uuid }
}

/** A plain conversation of `turns` user/assistant pairs. */
export function conversation(turns: number, tag = 's', opts: { split?: boolean } = {}): TranscriptBuilder {
  const b = new TranscriptBuilder(tag)
  for (let t = 0; t < turns; t++) {
    b.user(`question ${String(t)}`)
    b.say(`answer ${String(t)}`, opts)
  }
  return b
}

const clone = <T>(v: T): T => structuredClone(v)

/** Rows as a server page / replay delivers them: `_pos`, and `_ord` = `_pos`. */
export function asPaged(recs: readonly Rec[]): JsonlEntryData[] {
  return recs.map((r) => { const c = clone(r); (c as { _ord?: number })._ord = c._pos; return c })
}

/** Rows as the live tail delivers them: untagged. */
export function asLive(recs: readonly Rec[]): JsonlEntryData[] {
  return recs.map((r) => { const c = clone(r); delete c._pos; delete (c as { _ord?: number })._ord; return c })
}

/** Deterministic PRNG (mulberry32) for the property tests. */
export function rng(seed: number): () => number {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** Cut `rows` into random contiguous batches. */
export function chunk<T>(rows: readonly T[], rand: () => number, max = 40): T[][] {
  const out: T[][] = []
  for (let i = 0; i < rows.length;) {
    const n = 1 + Math.floor(rand() * max)
    out.push(rows.slice(i, i + n))
    i += n
  }
  return out
}
