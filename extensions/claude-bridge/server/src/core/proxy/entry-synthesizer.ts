// ============================================================================
// SSE → JSONL assistant entry synthesizer.
// Builds an entry shape that JsonlViewer renders identically to the
// real JSONL entry CLI will write later — same content blocks, same
// usage shape, same model. Dedup happens downstream by message.id.
// ============================================================================

import { randomUUID } from "node:crypto"
import type { AnthropicSseEvent, AnthropicUsage } from "./sse-parser"

/** Identical shape to what jsonl-watcher emits for `type:'assistant'`. */
export interface SynthAssistantEntry {
  type: "assistant"
  uuid: string
  timestamp: string
  parentUuid: string | null
  isSidechain: boolean
  /** The owning teammate/subagent for a sidechain stream — the CLI's
   *  `x-claude-code-agent-id` (`<name>-<n>@<team>`). Present only when this
   *  stream is a subagent's, so the client can route it to that agent's view
   *  instead of the main chat. Undefined for the main agent's stream. */
  subagentId?: string
  sessionId?: string
  model?: string
  usage?: AnthropicUsage
  message: {
    id: string
    role: "assistant"
    model: string
    content: SynthContentBlock[]
    stop_reason?: string | null
    usage?: AnthropicUsage
  }
  __streaming: boolean
  __streamingActiveBlock?: number
}

export type SynthContentBlock =
  | { type: "text"; text: string }
  | { type: "thinking"; thinking: string }
  | { type: "tool_use"; id: string; name: string; input: any }
  | { type: "redacted_thinking"; data: string }

export interface SynthCallbacks {
  /** Fires on every state change (after each delta) and on finalize. This is
   *  the OLD-client full-snapshot path (server throttles it to ~10fps). */
  onUpdate: (entry: SynthAssistantEntry) => void
  /** Fires ONLY at structural boundaries (message_start, content_block
   *  start/stop, message_delta, finalize, abort, error) — the NEW-client
   *  snapshot path that brackets the deltas. Sent immediately (unthrottled) so
   *  it always precedes the deltas of the block it opens. */
  onSnapshot?: (entry: SynthAssistantEntry) => void
  /** Fires on each text/thinking delta with just the appended slice — the
   *  NEW-client incremental path. NOT emitted for tool input_json_delta (tool
   *  args ride the boundary snapshot's static "preparing args…" placeholder). */
  onDelta?: (d: { msgId: string; blockIdx: number; appendText: string; kind: "text" | "thinking" }) => void
}

/** Per-PTY-session synthesizer. Each `feed` call processes one or more SSE
 *  events and mutates a single in-flight assistant entry. After
 *  `message_stop` (or stream end) the entry is "finalized" and a new
 *  `message_start` opens a fresh one. */
export class EntrySynthesizer {
  private current: SynthAssistantEntry | null = null
  private partialJson = new Map<number, string>()
  // Once aborted (user interrupt / upstream reset), the synth is spent: ignore
  // any late buffered SSE the transport still flushes (e.g. queued upstreamNode
  // 'data' between abort() and destroy(), or a dispatcher.close() flush). Those
  // late events belong to the KILLED turn and, if processed, re-emit a phantom
  // "old interrupted block" that races the NEXT turn's stream and renders out of
  // order for a beat before the JSONL settles it. This is a PER-REQUEST synth,
  // so latching it off is safe — a new turn gets a fresh synth.
  private aborted = false
  private readonly cb: SynthCallbacks
  private readonly ptySessionId: string
  private readonly cliConversationId?: string
  private readonly markSidechain: boolean
  private readonly subagentId?: string
  private readonly mainModel?: string

  constructor(opts: {
    ptySessionId: string
    cliConversationId?: string
    callbacks: SynthCallbacks
    markSidechain?: boolean
    /** The CLI's `x-claude-code-agent-id` when this request is a subagent's —
     *  makes `markSidechain` reliable (a teammate runs on a non-Haiku model that
     *  the response-side heuristics can't tell from a main turn) and lets the
     *  client route the stream to the owning agent. */
    subagentId?: string
    /** Session-level model the user picked. Used to decide whether a
     *  Haiku-model response is a side-quest (default) or actually the
     *  main turn (when user picked Haiku explicitly). */
    mainModel?: string
  }) {
    this.ptySessionId = opts.ptySessionId
    this.cliConversationId = opts.cliConversationId
    this.cb = opts.callbacks
    this.markSidechain = !!opts.markSidechain
    this.subagentId = opts.subagentId
    this.mainModel = opts.mainModel
  }

  feed(events: AnthropicSseEvent[]): void {
    if (this.aborted) return
    for (const evt of events) this.handleOne(evt)
  }

  /** A structural boundary — notify BOTH the old-client full-snapshot path and
   *  the new-client boundary-snapshot path. Deltas happen between these. */
  private emitSnapshot(): void {
    if (!this.current) return
    this.cb.onUpdate(this.current)
    this.cb.onSnapshot?.(this.current)
  }

  /** Force-finalize any in-flight entry (e.g. on connection abort). */
  // `hard` = a genuine mid-stream kill (user interrupt / upstream reset) — latch
  // the synth OFF so late buffered SSE for the dead turn can't re-emit a phantom
  // block. On a NORMAL close (hard=false) we must NOT latch: res 'close' fires on
  // every turn-end and races the async tee 'data'/'end' events, so latching there
  // would drop the tail of a healthy response (broke streaming — regression).
  abort(reason: string = "aborted", hard = false): void {
    if (hard) this.aborted = true
    if (!this.current) return
    this.current.message.stop_reason = reason
    this.current.__streaming = false
    this.current.__streamingActiveBlock = undefined
    this.emitSnapshot()
    this.current = null
    this.partialJson.clear()
  }

  private handleOne(evt: AnthropicSseEvent): void {
    try {
      switch (evt.type) {
        case "message_start":
          this.onMessageStart(evt)
          break
        case "content_block_start":
          this.onContentBlockStart(evt)
          break
        case "content_block_delta":
          this.onContentBlockDelta(evt)
          break
        case "content_block_stop":
          this.onContentBlockStop(evt)
          break
        case "message_delta":
          this.onMessageDelta(evt)
          break
        case "message_stop":
          this.onMessageStop()
          break
        case "error":
          this.onError(evt)
          break
        case "ping":
        case "unknown":
        default:
          break
      }
    } catch {
      // Defensive — never let a malformed event kill the stream.
    }
  }

  private onMessageStart(evt: Extract<AnthropicSseEvent, { type: "message_start" }>): void {
    const msg = evt.message
    // Side-quest detection. CLI side-quests (title generator, classifier,
    // sideQuery) run on HAIKU with short prompts; a real main turn runs on the
    // user's chosen model. Signals:
    //   • Haiku response while the user picked something heavier → side-quest.
    //   • A SHORT Haiku input → side-quest (distinguishes a Haiku side-quest
    //     from a Haiku MAIN turn when the user actually picked Haiku).
    // The input-size test is gated on `isHaiku` ON PURPOSE: with prompt caching
    // a legitimate main turn reports a TINY `input_tokens` (e.g. 2 — the whole
    // prompt was a cache hit and `cache_read_input_tokens` isn't yet counted at
    // message_start), so an un-gated `inputCtx < 3000` misfiled cached Opus/
    // Sonnet turns as side-quests and HID them from the chat entirely (they
    // still streamed to the raw CLI terminal). A non-Haiku response is the
    // user's own model → never a side-quest by input size.
    const u = msg.usage as any | undefined
    const inputCtx = (u?.input_tokens ?? 0) + (u?.cache_read_input_tokens ?? 0) + (u?.cache_creation_input_tokens ?? 0)
    const respFam = (msg.model || "").toLowerCase()
    const mainFam = (this.mainModel || "").toLowerCase()
    const isHaiku = respFam.includes("haiku")
    const userChoseHaiku = mainFam.includes("haiku")
    const sidechain = this.markSidechain
      || (isHaiku && !userChoseHaiku)
      || (isHaiku && inputCtx > 0 && inputCtx < 3000)
    this.current = {
      type: "assistant",
      uuid: `stream-${msg.id}`,
      timestamp: new Date().toISOString(),
      parentUuid: null,
      isSidechain: sidechain,
      ...(this.subagentId ? { subagentId: this.subagentId } : {}),
      sessionId: this.cliConversationId,
      model: msg.model,
      usage: msg.usage,
      message: {
        id: msg.id,
        role: "assistant",
        model: msg.model,
        content: [],
        usage: msg.usage,
      },
      __streaming: true,
    }
    this.partialJson.clear()
    this.emitSnapshot()
  }

  private onContentBlockStart(evt: Extract<AnthropicSseEvent, { type: "content_block_start" }>): void {
    if (!this.current) return
    const cb = evt.content_block
    const idx = evt.index
    let block: SynthContentBlock
    if (cb.type === "text") {
      block = { type: "text", text: cb.text ?? "" }
    } else if (cb.type === "thinking") {
      block = { type: "thinking", thinking: cb.thinking ?? "" }
    } else if (cb.type === "tool_use") {
      block = {
        type: "tool_use",
        id: cb.id,
        name: cb.name,
        // Mark as streaming until input_json_delta finalizes — UI shows placeholder.
        input: { __streaming: true, __partial: "" },
      }
      this.partialJson.set(idx, "")
    } else if ((cb as any).type === "redacted_thinking") {
      block = { type: "redacted_thinking", data: (cb as any).data ?? "" }
    } else {
      // Unknown content type (e.g. a future server_tool_use / web_search
      // block). Insert an EMPTY text placeholder rather than skipping —
      // skipping left a hole at content[idx], which JSON.stringify turns into
      // `null`, and the client's renderContentBlocks did `block.type` on it →
      // TypeError crash. A dense array keeps the render safe.
      block = { type: "text", text: "" }
    }
    this.current.message.content[idx] = block
    this.current.__streamingActiveBlock = idx
    this.emitSnapshot()
  }

  private onContentBlockDelta(evt: Extract<AnthropicSseEvent, { type: "content_block_delta" }>): void {
    if (!this.current) return
    const idx = evt.index
    const block = this.current.message.content[idx]
    if (!block) return
    const delta = evt.delta
    if (delta.type === "text_delta" && block.type === "text") {
      block.text += delta.text
      this.cb.onDelta?.({ msgId: this.current.message.id, blockIdx: idx, appendText: delta.text, kind: "text" })
    } else if (delta.type === "thinking_delta" && block.type === "thinking") {
      block.thinking += delta.thinking
      this.cb.onDelta?.({ msgId: this.current.message.id, blockIdx: idx, appendText: delta.thinking, kind: "thinking" })
    } else if (delta.type === "input_json_delta" && block.type === "tool_use") {
      const prev = this.partialJson.get(idx) ?? ""
      const next = prev + delta.partial_json
      // Cap per-block partial JSON at 1MB to guard against malformed streams.
      // The FULL accumulation stays in partialJson (onContentBlockStop parses
      // it into the real input); what we expose on the entry — which ships over
      // WS+IPC on every 100ms throttle tick — is only a short preview. The UI
      // renders a "preparing args…" placeholder, not the partial itself, so
      // streaming the whole (up-to-1MB) blob every tick was pure waste on big
      // Write/Edit tool calls.
      const full = next.length > 1_048_576 ? next.slice(0, 1_048_576) : next
      this.partialJson.set(idx, full)
      ;(block.input as any).__partial = full.length > 256 ? full.slice(0, 256) : full
    }
    this.current.__streamingActiveBlock = idx
    this.cb.onUpdate(this.current)
  }

  private onContentBlockStop(evt: Extract<AnthropicSseEvent, { type: "content_block_stop" }>): void {
    if (!this.current) return
    const idx = evt.index
    const block = this.current.message.content[idx]
    if (!block) return
    if (block.type === "tool_use") {
      const raw = this.partialJson.get(idx) ?? ""
      try {
        const parsed = raw ? JSON.parse(raw) : {}
        block.input = parsed
      } catch {
        block.input = { __parseError: true, __raw: raw.slice(0, 2000) }
      }
      this.partialJson.delete(idx)
    }
    this.emitSnapshot()
  }

  private onMessageDelta(evt: Extract<AnthropicSseEvent, { type: "message_delta" }>): void {
    if (!this.current) return
    if (evt.delta.stop_reason) this.current.message.stop_reason = evt.delta.stop_reason
    if (evt.usage) {
      const merged: AnthropicUsage = { ...(this.current.message.usage ?? {}), ...evt.usage }
      this.current.message.usage = merged
      this.current.usage = merged
    }
    this.emitSnapshot()
  }

  private onMessageStop(): void {
    if (!this.current) return
    this.current.__streaming = false
    this.current.__streamingActiveBlock = undefined
    this.emitSnapshot()
    this.current = null
    this.partialJson.clear()
  }

  private onError(evt: Extract<AnthropicSseEvent, { type: "error" }>): void {
    if (!this.current) {
      // Error without active message — synthesize a minimal one so UI shows it.
      this.current = {
        type: "assistant",
        uuid: `stream-error-${randomUUID()}`,
        timestamp: new Date().toISOString(),
        parentUuid: null,
        isSidechain: this.markSidechain,
        model: "unknown",
        message: {
          id: `msg_err_${randomUUID().slice(0, 12)}`,
          role: "assistant",
          model: "unknown",
          content: [{ type: "text", text: `[API Error: ${evt.error.type}] ${evt.error.message}` }],
          stop_reason: "error",
        },
        __streaming: false,
      }
      this.emitSnapshot()
      this.current = null
      return
    }
    const errText = `[API Error: ${evt.error.type}] ${evt.error.message}`
    this.current.message.content.push({ type: "text", text: errText })
    this.current.message.stop_reason = "error"
    this.current.__streaming = false
    this.current.__streamingActiveBlock = undefined
    this.emitSnapshot()
    this.current = null
    this.partialJson.clear()
  }
}
