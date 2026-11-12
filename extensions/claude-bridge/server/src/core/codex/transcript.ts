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
import type { ContentBlock, JsonlEntry } from '../../shared/jsonl-types'

export type CodexTranscriptEntry = JsonlEntry & {
  engine: 'codex'
  /** System entry kind, as in Claude transcripts (`compact_boundary`, …). */
  subtype?: string
  level?: 'info' | 'warning' | 'error'
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
