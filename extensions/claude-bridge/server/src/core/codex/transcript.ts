// ============================================================================
// Normalized codex events → Bridge transcript entries (JsonlEntry shape).
//
// Owner decision: a codex session gets its own Bridge transcript in the same
// entry format the viewer, watcher, pagination, mirror and export already
// consume, marked `engine: "codex"` on every line. It is a projection of
// completed items, not a forged Claude CLI file: codex's rollout stays the
// source of truth for resume, and live deltas travel as streaming frames.
// Tool names follow the MCP convention the viewer strips (`mcp__<server>__X`).
// ============================================================================

import type { AgentEvent, AgentToolOutput, AgentToolRef } from '../../shared/agent-events'
import { ENGINE_USAGE_SUBTYPE } from '../../shared/engine-accounting'
import type { ContentBlock, JsonlEntry } from '../../shared/jsonl-types'

/** System entry with one model response's token usage (no content, not drawn). */
const CODEX_USAGE_SUBTYPE = ENGINE_USAGE_SUBTYPE

export interface CodexUsage {
  input_tokens: number
  cache_read_input_tokens: number
  cache_creation_input_tokens: number
  output_tokens: number
  reasoning_output_tokens: number
}

export type CodexTranscriptEntry = JsonlEntry & {
  engine: 'codex'
  /** System entry kind, as in Claude transcripts (`compact_boundary`, …). */
  subtype?: string
  level?: 'info' | 'warning' | 'error'
  /** CODEX_USAGE_SUBTYPE entries: the model and the response's tokens. */
  model?: string
  usage?: CodexUsage
  contextWindow?: number
}

/** Provider web search renders with the viewer's existing WebSearch card. */
function toolName(tool: AgentToolRef): string {
  if (tool.source === 'builtin') return tool.name === 'web_search' ? 'WebSearch' : tool.name
  return `mcp__${tool.server ?? 'dynamic'}__${tool.name}`
}

function resultContent(output: AgentToolOutput[], error?: string): string | ContentBlock[] {
  const blocks: ContentBlock[] = output.map((o) =>
    o.type === 'text'
      ? { type: 'text', text: o.text }
      : o.type === 'image'
        ? ({ type: 'image', source: { type: 'url', url: o.url } } as unknown as ContentBlock)
        : { type: 'text', text: JSON.stringify(o.value) },
  )
  if (error) blocks.push({ type: 'text', text: `Error: ${error}` })
  return blocks.length ? blocks : ''
}

export class CodexTranscriptProjector {
  private parent: string | null
  private model: string | undefined

  /** `parentUuid` continues an existing transcript (resume). */
  constructor(
    private readonly cwd?: string,
    parentUuid: string | null = null,
  ) {
    this.parent = parentUuid
  }

  /** The model the next turn runs on (a model change in the chat). */
  setModel(model: string | null): void {
    this.model = model ?? undefined
  }

  /** Entries to append for one event, in order. */
  project(event: AgentEvent): CodexTranscriptEntry[] {
    switch (event.kind) {
      case 'session.started':
        this.model = event.model ?? undefined
        return []
      case 'model.rerouted':
        this.model = event.to
        return [this.system(event, `Model rerouted from ${event.from} to ${event.to} (${event.reason})`, 'info')]
      case 'message.user':
        return [this.entry(event, event.id, 'user', { role: 'user', content: event.text })]
      case 'message.assistant':
        return [this.assistant(event, event.id, [{ type: 'text', text: event.text }])]
      case 'reasoning': {
        const text = (event.summary.length ? event.summary : event.content).join('\n\n')
        return text ? [this.assistant(event, event.id, [{ type: 'thinking', thinking: text }])] : []
      }
      case 'tool.completed': {
        const name = toolName(event.tool)
        const input = (event.input && typeof event.input === 'object' ? event.input : { value: event.input }) as Record<
          string,
          unknown
        >
        return [
          this.assistant(event, `${event.id}:tool_use`, [{ type: 'tool_use', id: event.id, name, input }]),
          this.entry(event, `${event.id}:tool_result`, 'user', {
            role: 'user',
            content: [
              {
                type: 'tool_result',
                tool_use_id: event.id,
                content: resultContent(
                  event.output,
                  event.status === 'completed' ? undefined : (event.error ?? event.status),
                ),
              },
            ],
          }),
        ]
      }
      case 'usage': {
        const e = this.usage(event)
        return e ? [e] : []
      }
      case 'context.compaction': {
        const e = this.system(event, 'Conversation compacted', 'info', event.id)
        e.subtype = 'compact_boundary'
        e.compactMetadata = { trigger: 'auto' }
        return [e]
      }
      case 'turn.completed':
        if (event.status === 'interrupted') {
          const e = this.system(event, 'Interrupted by user', 'info')
          e.interruptedByUser = true
          return [e]
        }
        return event.status === 'failed' ? [this.system(event, event.error?.message ?? 'Turn failed', 'error')] : []
      case 'notice':
        // Retried errors are transient; the final outcome arrives as turn.completed.
        return event.level === 'error' && event.willRetry ? [] : [this.system(event, event.message, event.level)]
      case 'item.unsupported':
        return [this.system(event, `Unsupported codex item: ${event.itemType}`, 'warning')]
      default:
        return []
    }
  }

  private entry(
    event: AgentEvent,
    uuid: string,
    type: 'user' | 'assistant' | 'system',
    message?: JsonlEntry['message'],
  ): CodexTranscriptEntry {
    const entry: CodexTranscriptEntry = {
      type,
      uuid,
      parentUuid: this.parent,
      timestamp: new Date(event.at).toISOString(),
      sessionId: event.threadId,
      engine: 'codex',
      ...(this.cwd ? { cwd: this.cwd } : {}),
      ...(message ? { message } : {}),
    }
    this.parent = uuid
    return entry
  }

  private assistant(event: AgentEvent, uuid: string, content: ContentBlock[]): CodexTranscriptEntry {
    const id = 'id' in event ? event.id : uuid
    return this.entry(event, uuid, 'assistant', {
      role: 'assistant',
      id,
      content,
      ...(this.model ? { model: this.model } : {}),
    })
  }

  /** One model response's tokens (codex `last`), in the Claude usage shape the
   *  accounting reads. codex counts cached reads and cache writes inside
   *  inputTokens, Claude reports them beside it, so they are split out. The
   *  uuid is keyed by the thread's running total: a usage update codex repeats
   *  (resume) maps to the same row instead of a second response. */
  private usage(event: Extract<AgentEvent, { kind: 'usage' }>): CodexTranscriptEntry | null {
    const u = event.last
    if (!(u.totalTokens > 0)) return null
    const cacheRead = Math.max(0, u.cachedInputTokens)
    const cacheWrite = Math.max(0, u.cacheWriteInputTokens ?? 0)
    // Bookkeeping, not conversation: it hangs off the chain without becoming
    // the parent of the next entry, so the chat's parent chain is unchanged.
    const parent = this.parent
    const e = this.entry(event, `usage:${event.turnId}:${event.total.totalTokens}`, 'system')
    this.parent = parent
    e.subtype = CODEX_USAGE_SUBTYPE
    e.model = this.model
    e.usage = {
      input_tokens: Math.max(0, u.inputTokens - cacheRead - cacheWrite),
      cache_read_input_tokens: cacheRead,
      cache_creation_input_tokens: cacheWrite,
      output_tokens: Math.max(0, u.outputTokens),
      reasoning_output_tokens: Math.max(0, u.reasoningOutputTokens),
    }
    if (event.contextWindow) e.contextWindow = event.contextWindow
    return e
  }

  private system(
    event: AgentEvent,
    text: string,
    level: 'info' | 'warning' | 'error',
    id?: string,
  ): CodexTranscriptEntry {
    const e = this.entry(event, id ?? `${event.kind}:${event.at}:${this.parent ?? 'root'}`, 'system')
    e.content = text
    e.level = level
    return e
  }
}
