// ============================================================================
// Which transcript lines carry billable usage, for every engine.
//
// A Claude transcript bills on `assistant` lines: one API response is split
// over N lines that share message.id and repeat its usage. A codex transcript
// (Bridge's projection, codex/transcript.ts) writes each completed item —
// reasoning, text, tool call — as its own `assistant` line WITHOUT usage, and
// each model response's tokens as one `system/api_usage` line. Counting codex
// item lines as responses multiplied "assistant messages" and requests by the
// items per response. Every engine-aware consumer (DuckDB sweeper, live
// counters, project outline, the chat cost bar) classifies lines here.
// Lines without an `engine` field are Claude's: Claude CLI writes none, and a
// multi-engine conversation log keeps each copied line's own field.
// Shared by the server and the webview.
// ============================================================================

export type AccountingEngine = 'claude' | 'codex'

/** System subtype of a codex line with one model response's usage. */
export const ENGINE_USAGE_SUBTYPE = 'api_usage'

export interface EngineUsageBlock {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
  reasoning_output_tokens?: number
}

interface Line {
  type?: unknown
  subtype?: unknown
  engine?: unknown
}

export function lineEngine(line: Line | null | undefined): AccountingEngine {
  return line?.engine === 'codex' ? 'codex' : 'claude'
}

/** A codex `system/api_usage` line: one model response with its tokens. */
export function isEngineUsageLine(line: Line | null | undefined): boolean {
  return line?.type === 'system' && line.subtype === ENGINE_USAGE_SUBTYPE && lineEngine(line) === 'codex'
}

/** A codex item line (`assistant`, no usage): content only, never a response. */
export function isUsagelessItemLine(line: Line | null | undefined): boolean {
  return line?.type === 'assistant' && lineEngine(line) === 'codex'
}
