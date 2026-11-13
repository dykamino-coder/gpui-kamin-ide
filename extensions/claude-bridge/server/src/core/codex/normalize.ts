// ============================================================================
// codex app-server notifications → normalized AgentEvent stream.
//
// Pure projection (no I/O) so it is tested against recorded protocol traces.
// The item lifecycle is item/started → deltas → item/completed; `completed`
// carries the authoritative final content. Notifications Bridge does not
// render (rate limits, MCP startup status, remote control…) produce nothing;
// an item type it does not know produces `item.unsupported` rather than
// disappearing.
// ============================================================================

import type { AgentEvent, AgentTokenUsage, AgentToolOutput, AgentToolRef } from '../../shared/agent-events'
import type { ItemNotification, ThreadItem, TokenUsageBreakdown, Turn } from './protocol'

type Ev = AgentEvent
type Item = Record<string, unknown> & { type: string; id?: string }

function usage(u: TokenUsageBreakdown): AgentTokenUsage {
  return {
    totalTokens: u.totalTokens,
    inputTokens: u.inputTokens,
    cachedInputTokens: u.cachedInputTokens,
    cacheWriteInputTokens: u.cacheWriteInputTokens ?? 0,
    outputTokens: u.outputTokens,
    reasoningOutputTokens: u.reasoningOutputTokens,
  }
}

function userText(item: Item): string {
  const content = Array.isArray(item.content) ? (item.content as Array<{ type?: string; text?: string }>) : []
  return content
    .filter((c) => c.type === 'text' && typeof c.text === 'string')
    .map((c) => c.text)
    .join('\n')
}

function mcpOutput(result: unknown): AgentToolOutput[] {
  const content = (result as { content?: unknown[] } | null)?.content
  if (!Array.isArray(content)) return []
  return content.map((block): AgentToolOutput => {
    const b = block as { type?: string; text?: string; data?: string; mimeType?: string }
    if (b.type === 'text' && typeof b.text === 'string') return { type: 'text', text: b.text }
    if (b.type === 'image' && typeof b.data === 'string') {
      return { type: 'image', url: `data:${b.mimeType ?? 'image/png'};base64,${b.data}` }
    }
    return { type: 'json', value: block }
  })
}

function dynamicOutput(items: unknown): AgentToolOutput[] {
  if (!Array.isArray(items)) return []
  return items.map((it): AgentToolOutput => {
    const x = it as { type?: string; text?: string; imageUrl?: string }
    if (x.type === 'inputText' && typeof x.text === 'string') return { type: 'text', text: x.text }
    if (x.type === 'inputImage' && typeof x.imageUrl === 'string') return { type: 'image', url: x.imageUrl }
    return { type: 'json', value: it }
  })
}

function toolRef(item: Item): AgentToolRef | undefined {
  if (item.type === 'mcpToolCall') return { source: 'mcp', server: String(item.server), name: String(item.tool) }
  if (item.type === 'dynamicToolCall') {
    return { source: 'dynamic', ...(item.namespace ? { server: String(item.namespace) } : {}), name: String(item.tool) }
  }
  if (item.type === 'commandExecution') return { source: 'builtin', name: 'command' }
  if (item.type === 'fileChange') return { source: 'builtin', name: 'file_change' }
  if (item.type === 'webSearch') return { source: 'builtin', name: 'web_search' }
  return undefined
}

function toolInput(item: Item): unknown {
  if (item.type === 'commandExecution') return { command: item.command }
  if (item.type === 'fileChange') return { changes: item.changes }
  if (item.type === 'webSearch') return { query: item.query, ...(item.action ? { action: item.action } : {}) }
  return item.arguments
}

function turnStatus(turn: Turn): 'completed' | 'interrupted' | 'failed' {
  return turn.status === 'interrupted' || turn.status === 'failed' ? turn.status : 'completed'
}

export class CodexEventNormalizer {
  constructor(private readonly now: () => number = Date.now) {}

  /** Maps one server notification to zero or more normalized events. */
  normalize(method: string, params: unknown, emittedAtMs?: unknown): Ev[] {
    const p = (params ?? {}) as Record<string, any>
    const threadId = typeof p.threadId === 'string' ? p.threadId : (p.thread?.id ?? '')
    const at = typeof emittedAtMs === 'number' ? emittedAtMs : this.now()
    const base = { threadId, at }
    switch (method) {
      case 'turn/started':
        return [{ ...base, kind: 'turn.started', turnId: p.turn.id }]
      case 'turn/completed': {
        const turn = p.turn as Turn
        const ev: Ev = { ...base, kind: 'turn.completed', turnId: turn.id, status: turnStatus(turn) }
        if (turn.error) {
          ev.error = {
            message: turn.error.message,
            ...(turn.error.additionalDetails ? { details: turn.error.additionalDetails } : {}),
          }
        }
        if (typeof turn.durationMs === 'number') ev.durationMs = turn.durationMs
        return [ev]
      }
      case 'item/started':
      case 'item/completed':
        return this.item(method === 'item/completed', p as ItemNotification, base)
      case 'item/agentMessage/delta':
        return [{ ...base, kind: 'message.assistant.delta', id: p.itemId, turnId: p.turnId, delta: p.delta }]
      case 'item/reasoning/summaryTextDelta':
        return [
          {
            ...base,
            kind: 'reasoning.delta',
            id: p.itemId,
            turnId: p.turnId,
            channel: 'summary',
            index: p.summaryIndex ?? 0,
            delta: p.delta,
          },
        ]
      case 'item/reasoning/textDelta':
        return [
          {
            ...base,
            kind: 'reasoning.delta',
            id: p.itemId,
            turnId: p.turnId,
            channel: 'content',
            index: p.contentIndex ?? 0,
            delta: p.delta,
          },
        ]
      case 'thread/tokenUsage/updated':
        return [
          {
            ...base,
            kind: 'usage',
            turnId: p.turnId,
            total: usage(p.tokenUsage.total),
            last: usage(p.tokenUsage.last),
            contextWindow: p.tokenUsage.modelContextWindow ?? null,
          },
        ]
      // Deprecated twin of the contextCompaction item; the item is authoritative.
      case 'thread/compacted':
        return []
      case 'model/rerouted':
        return [
          {
            ...base,
            kind: 'model.rerouted',
            turnId: p.turnId,
            from: p.fromModel,
            to: p.toModel,
            reason: String(p.reason),
          },
        ]
      case 'error':
        return [
          {
            ...base,
            kind: 'notice',
            level: 'error',
            message: p.error?.message ?? 'codex error',
            ...(p.turnId ? { turnId: p.turnId } : {}),
            willRetry: Boolean(p.willRetry),
          },
        ]
      case 'warning':
      case 'configWarning':
      case 'deprecationNotice':
        return typeof p.message === 'string' || typeof p.summary === 'string'
          ? [{ ...base, kind: 'notice', level: 'warning', message: p.message ?? p.summary }]
          : []
      default:
        return []
    }
  }

  private item(completed: boolean, n: ItemNotification, base: { threadId: string; at: number }): Ev[] {
    const item = n.item as ThreadItem as Item
    const turnId = n.turnId
    const at = (completed ? n.completedAtMs : n.startedAtMs) ?? base.at
    const b = { threadId: base.threadId || n.threadId, at }
    const id = String(item.id ?? '')
    switch (item.type) {
      case 'userMessage':
        return completed ? [{ ...b, kind: 'message.user', id, turnId, text: userText(item) }] : []
      case 'agentMessage':
        return completed ? [{ ...b, kind: 'message.assistant', id, turnId, text: String(item.text ?? '') }] : []
      case 'reasoning':
        return completed
          ? [
              {
                ...b,
                kind: 'reasoning',
                id,
                turnId,
                summary: (item.summary as string[] | undefined) ?? [],
                content: (item.content as string[] | undefined) ?? [],
              },
            ]
          : []
      case 'contextCompaction':
        return completed ? [{ ...b, kind: 'context.compaction', turnId, id }] : []
      case 'mcpToolCall':
      case 'dynamicToolCall':
      case 'commandExecution':
      case 'fileChange':
      case 'webSearch': {
        // webSearch: provider-side search (enabled like Claude's WebSearch), runs at OpenAI.
        const tool = toolRef(item)!
        const input = toolInput(item)
        if (!completed) return [{ ...b, kind: 'tool.started', id, turnId, tool, input }]
        return [this.toolCompleted(item, tool, input, id, turnId, b)]
      }
      // Other items (plan, hookPrompt, collab agents, image views, …) are not
      // reachable with Bridge's lockdown but must stay visible if they appear.
      default:
        return completed ? [{ ...b, kind: 'item.unsupported', turnId, itemType: item.type, ...(id ? { id } : {}) }] : []
    }
  }

  private toolCompleted(
    item: Item,
    tool: AgentToolRef,
    input: unknown,
    id: string,
    turnId: string,
    b: { threadId: string; at: number },
  ): Ev {
    const durationMs = typeof item.durationMs === 'number' ? { durationMs: item.durationMs } : {}
    if (item.type === 'mcpToolCall') {
      const error = (item.error as { message?: string } | null)?.message
      return {
        ...b,
        kind: 'tool.completed',
        id,
        turnId,
        tool,
        input,
        status: item.status === 'failed' ? 'failed' : 'completed',
        output: mcpOutput(item.result),
        ...(error ? { error } : {}),
        ...durationMs,
      }
    }
    if (item.type === 'dynamicToolCall') {
      return {
        ...b,
        kind: 'tool.completed',
        id,
        turnId,
        tool,
        input,
        status: item.status === 'failed' || item.success === false ? 'failed' : 'completed',
        output: dynamicOutput(item.contentItems),
        ...durationMs,
      }
    }
    if (item.type === 'webSearch') {
      const results = Array.isArray(item.results)
        ? item.results.map((value): AgentToolOutput => ({ type: 'json', value }))
        : []
      return { ...b, kind: 'tool.completed', id, turnId, tool, input, status: 'completed', output: results }
    }
    const status = String(item.status)
    const output: AgentToolOutput[] =
      typeof item.aggregatedOutput === 'string' && item.aggregatedOutput
        ? [{ type: 'text', text: item.aggregatedOutput }]
        : []
    return {
      ...b,
      kind: 'tool.completed',
      id,
      turnId,
      tool,
      input,
      status: status === 'declined' ? 'declined' : status === 'completed' ? 'completed' : 'failed',
      output,
      ...durationMs,
    }
  }
}
