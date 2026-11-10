// Tells the CLI's conversation-compaction request (manual /compact and
// auto-compact alike) from a normal turn, at the request body.
//
// The CLI sends the summary request with its own system prompt (checked in
// the 2.1.3xx bundle: `systemPrompt: ["You are a helpful AI assistant tasked
// with summarizing conversations."]`). Its streamed answer is the
// `<analysis>…<summary>` text that the TUI never shows: the TUI shows
// "Compacting conversation…" and then the compact boundary plus the summary
// row. Streaming it into the chat as an assistant answer leaves a ghost stub
// too, because no canonical assistant row with that message id is ever written.
//
// We match the raw JSON fragment `"text":"<prompt>`, the start of a system
// block. Inside a message the same prose is a JSON string, so the quotes
// around it are escaped (`\"`) and the raw bytes differ: quoting the prompt in
// a chat cannot trigger the match.
// The scan streams over the request body: no buffering, only a tail of
// `marker.length - 1` bytes is carried between chunks.

import { Buffer } from 'node:buffer'
import { Transform, type TransformCallback } from 'node:stream'
import type { SynthAssistantEntry } from './entry-synthesizer'

export const COMPACTION_SYSTEM_PROMPT = 'You are a helpful AI assistant tasked with summarizing conversations.'
// No closing quote: a CLI that appends to the same system block still matches.
const MARKER = Buffer.from(`"text":${JSON.stringify(COMPACTION_SYSTEM_PROMPT).slice(0, -1)}`, 'utf8')

/** Whole-body check (buffered diagnostic path, tests). */
export function isCompactionRequestBody(body: Buffer | string): boolean {
  return (typeof body === 'string' ? Buffer.from(body, 'utf8') : body).includes(MARKER)
}

/** Pass-through that records whether the body carried the compaction prompt. */
export class CompactionRequestScanner extends Transform {
  matched = false
  private tail: Buffer = Buffer.alloc(0)

  override _transform(chunk: Buffer, _encoding: BufferEncoding, done: TransformCallback): void {
    if (!this.matched) {
      const window = this.tail.length ? Buffer.concat([this.tail, chunk]) : chunk
      if (window.includes(MARKER)) {
        this.matched = true
        this.tail = Buffer.alloc(0)
      } else {
        const keep = Math.min(MARKER.length - 1, window.length)
        this.tail = Buffer.from(window.subarray(window.length - keep))
      }
    }
    done(null, chunk)
  }
}

/** What a client sees of a compaction stream: same message id and lifecycle
 *  (`__streaming` until the final snapshot), no content. */
export function compactionMarker(entry: SynthAssistantEntry): SynthAssistantEntry {
  return {
    ...entry,
    message: { ...entry.message, content: [] },
    __streamingActiveBlock: undefined,
    __compaction: true,
  }
}
