// ============================================================================
// Anthropic SSE parser — incremental, chunk-safe.
// Splits buffer on `\n\n` event boundaries, decodes `event:` + `data:` lines,
// JSON-parses data payloads. Tolerates malformed events (skip + log) and
// chunks split mid-record (buffers leftover until next push).
// ============================================================================

export type AnthropicSseEvent =
  | { type: "message_start"; message: { id: string; model: string; role: "assistant"; usage: AnthropicUsage } }
  | { type: "content_block_start"; index: number; content_block: AnthropicContentBlockStart }
  | { type: "content_block_delta"; index: number; delta: AnthropicContentDelta }
  | { type: "content_block_stop"; index: number }
  | { type: "message_delta"; delta: { stop_reason: string | null; stop_sequence: string | null }; usage: AnthropicUsage }
  | { type: "message_stop" }
  | { type: "ping" }
  | { type: "error"; error: { type: string; message: string } }
  | { type: "unknown"; raw: string }

export interface AnthropicUsage {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

export type AnthropicContentBlockStart =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "tool_use"; id: string; name: string; input: Record<string, unknown> }
  | { type: "redacted_thinking"; data: string }

export type AnthropicContentDelta =
  | { type: "text_delta"; text: string }
  | { type: "thinking_delta"; thinking: string }
  | { type: "input_json_delta"; partial_json: string }
  | { type: "signature_delta"; signature: string }

export class SseStreamParser {
  private buf = ""

  /** Push a raw chunk of bytes (already utf-8 decoded) and return whatever
   *  full events became available. Leftover (incomplete) data stays in buf. */
  push(chunk: string): AnthropicSseEvent[] {
    this.buf += chunk
    const out: AnthropicSseEvent[] = []
    while (true) {
      const sep = this.buf.indexOf("\n\n")
      if (sep < 0) {
        // Some servers use \r\n\r\n — handle both.
        const sepCRLF = this.buf.indexOf("\r\n\r\n")
        if (sepCRLF < 0) break
        const block = this.buf.slice(0, sepCRLF)
        this.buf = this.buf.slice(sepCRLF + 4)
        const evt = this.parseBlock(block)
        if (evt) out.push(evt)
        continue
      }
      const block = this.buf.slice(0, sep)
      this.buf = this.buf.slice(sep + 2)
      const evt = this.parseBlock(block)
      if (evt) out.push(evt)
    }
    return out
  }

  /** Force flush — call on stream end. Any partial data at this point is
   *  considered malformed; we drop it. */
  end(): AnthropicSseEvent[] {
    if (!this.buf.trim()) return []
    const evt = this.parseBlock(this.buf)
    this.buf = ""
    return evt ? [evt] : []
  }

  private parseBlock(block: string): AnthropicSseEvent | null {
    if (!block.trim()) return null
    let eventName: string | null = null
    const dataLines: string[] = []
    for (const line of block.split(/\r?\n/)) {
      if (line.startsWith("event:")) {
        eventName = line.slice(6).trim()
      } else if (line.startsWith("data:")) {
        dataLines.push(line.slice(5).replace(/^\s/, ""))
      } else if (line.startsWith(":")) {
        // SSE comment, skip
      }
    }
    if (dataLines.length === 0) return null
    const dataStr = dataLines.join("\n")
    let payload: unknown
    try {
      payload = JSON.parse(dataStr)
    } catch {
      return { type: "unknown", raw: `parse-error: ${dataStr.slice(0, 200)}` }
    }
    if (typeof payload !== "object" || payload === null) {
      return { type: "unknown", raw: dataStr.slice(0, 200) }
    }
    const p = payload as Record<string, unknown>
    const typeFromPayload = typeof p.type === "string" ? p.type : eventName
    if (!typeFromPayload) return { type: "unknown", raw: dataStr.slice(0, 200) }
    return p as unknown as AnthropicSseEvent
  }
}
