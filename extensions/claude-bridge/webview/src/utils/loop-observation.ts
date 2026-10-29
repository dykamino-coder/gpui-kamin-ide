import type { JsonlEntryData } from '../types/jsonl'
import { hasCliXmlTags, isCaveatOnlyEntry, isSyntheticUserText } from '../components/jsonl-viewer/utils'
import { isInterruptUserEntry, isSkippedUserEntry } from '../components/jsonl-viewer/prepare-jsonl-entries'

export interface CompletedTurn {
  key: string
  endedAt: number
  durationMs: number
  toolCalls: number
}
export interface TurnObservation {
  key: string
  startedAt: number
  toolCalls: number
  repeatedCalls: number
  repeatedErrors: number
}
export interface TurnAnalysis {
  completed: CompletedTurn[]
  current: TurnObservation | null
}

// Canonical comparison is local and bounded; arguments/results never leave
// this scan, enter UI state, storage, telemetry, or an agent message.
function signature(value: unknown): string | undefined {
  let budget = 8192
  const seen = new Set<object>()
  function visit(v: unknown, depth: number): string {
    if (depth > 12) throw new Error('deep')
    let out: string
    if (v === null || typeof v !== 'object') {
      if (typeof v === 'string' && v.length > budget) throw new Error('large')
      out = JSON.stringify(v) ?? 'null'
    } else {
      if (seen.has(v)) throw new Error('cycle')
      seen.add(v)
      if (Array.isArray(v)) {
        if (v.length > 128) throw new Error('large')
        out = '[' + v.map((x) => visit(x, depth + 1)).join(',') + ']'
      } else {
        const keys = Object.keys(v).sort()
        if (keys.length > 128) throw new Error('large')
        out =
          '{' +
          keys.map((k) => JSON.stringify(k) + ':' + visit((v as Record<string, unknown>)[k], depth + 1)).join(',') +
          '}'
      }
      seen.delete(v)
    }
    budget -= out.length
    if (budget < 0) throw new Error('large')
    return out
  }
  try {
    return visit(value, 0)
  } catch {
    return undefined
  }
}

export function analyzeTurns(entries: readonly JsonlEntryData[]): TurnAnalysis {
  const completed: CompletedTurn[] = []
  let current: TurnObservation | null = null
  const seenCalls = new Set<string>()
  const seenResults = new Set<string>()
  const names = new Map<string, string>()
  let callSignature: string | undefined
  let errorSignature: string | undefined
  function finish(endedAt: number, durationMs?: number) {
    if (!current) return
    const duration = durationMs ?? endedAt - current.startedAt
    if (Number.isFinite(duration) && duration > 0 && Number.isFinite(endedAt)) {
      completed.push({ key: current.key, endedAt, durationMs: duration, toolCalls: current.toolCalls })
      if (completed.length > 40) completed.shift()
    }
    current = null
  }
  for (const e of entries) {
    if (e.isSidechain || e.subagentId || e.__streaming) continue
    if (
      e.interruptedByUser ||
      e.skippedByUser ||
      e.subtype === 'compact_boundary' ||
      isInterruptUserEntry(e) ||
      isSkippedUserEntry(e)
    ) {
      current = null
      continue
    }
    const at = Date.parse(e.timestamp ?? '')
    const content = e.message?.content
    const userText =
      e.type !== 'user'
        ? ''
        : typeof content === 'string'
          ? content
          : Array.isArray(content)
            ? content
                .filter((b) => b.type === 'text')
                .map((b) => b.text ?? '')
                .join('\n')
            : ''
    const isPrompt =
      e.type === 'user' &&
      !e.isMeta &&
      !e.isCompactSummary &&
      !isSyntheticUserText(userText) &&
      !hasCliXmlTags(userText) &&
      !isCaveatOnlyEntry(e) &&
      (typeof content === 'string'
        ? content.trim().length > 0
        : Array.isArray(content) && content.length > 0 && content.every((b) => b.type !== 'tool_result'))
    if (isPrompt) {
      // An interrupted/incomplete previous turn never trains the baseline.
      current =
        Number.isFinite(at) && e.uuid
          ? {
              key: e.uuid,
              startedAt: at,
              toolCalls: 0,
              repeatedCalls: 0,
              repeatedErrors: 0,
            }
          : null
      seenCalls.clear()
      seenResults.clear()
      names.clear()
      callSignature = undefined
      errorSignature = undefined
    }
    if (!current) continue
    if (Array.isArray(content))
      for (const b of content) {
        if (e.type === 'assistant' && b.type === 'tool_use' && b.id && !seenCalls.has(b.id)) {
          seenCalls.add(b.id)
          names.set(b.id, b.name ?? '')
          current.toolCalls++
          const next = signature([b.name, b.input])
          current.repeatedCalls = next !== undefined && next === callSignature ? current.repeatedCalls + 1 : 1
          callSignature = next
        }
        if (e.type === 'user' && b.type === 'tool_result' && b.tool_use_id && !seenResults.has(b.tool_use_id)) {
          seenResults.add(b.tool_use_id)
          const name = names.get(b.tool_use_id)
          const next =
            b.is_error && b._fullLen === undefined && name !== undefined ? signature([name, b.content]) : undefined
          current.repeatedErrors =
            next !== undefined && next === errorSignature ? current.repeatedErrors + 1 : next === undefined ? 0 : 1
          errorSignature = next
        }
      }
    if (e.type === 'system' && e.subtype === 'turn_duration') finish(at, e.durationMs)
    else if (e.type === 'assistant' && e.message?.stop_reason === 'end_turn') finish(at)
  }
  return { completed, current }
}

export interface TurnBaseline {
  durationMs: number
  toolCalls: number
  samples: number
}
export class ProjectTurnHistory {
  private projects = new Map<string, Map<string, CompletedTurn>>()

  observe(project: string, conversation: string, turns: readonly CompletedTurn[]): TurnBaseline | null {
    const history = this.projects.get(project) ?? new Map<string, CompletedTurn>()
    for (const turn of turns) history.set(`${conversation}:${turn.key}`, turn)
    const recent = [...history.entries()].sort((a, b) => b[1].endedAt - a[1].endedAt).slice(0, 40)
    this.projects.delete(project)
    this.projects.set(project, new Map(recent))
    if (this.projects.size > 32) this.projects.delete(this.projects.keys().next().value!)
    if (recent.length < 5) return null
    const median = (values: number[]) => {
      values.sort((a, b) => a - b)
      const mid = values.length >> 1
      return values.length % 2 ? values[mid]! : (values[mid - 1]! + values[mid]!) / 2
    }
    return {
      durationMs: median(recent.map(([, t]) => t.durationMs)),
      toolCalls: median(recent.map(([, t]) => t.toolCalls)),
      samples: recent.length,
    }
  }
}

export type LoopSignal = 'repeated-calls' | 'repeated-errors' | 'long-turn' | 'many-calls'
export function loopSignal(
  current: TurnObservation | null,
  baseline: TurnBaseline | null,
  now: number,
  liveWorking: boolean,
): LoopSignal | null {
  if (!liveWorking || !current || now < current.startedAt) return null
  if (current.repeatedErrors >= 3) return 'repeated-errors'
  if (current.repeatedCalls >= 4) return 'repeated-calls'
  if (!baseline || baseline.samples < 5) return null
  if (baseline.toolCalls > 0 && current.toolCalls > 2.5 * baseline.toolCalls) return 'many-calls'
  if (now - current.startedAt > 2.5 * baseline.durationMs) return 'long-turn'
  return null
}
