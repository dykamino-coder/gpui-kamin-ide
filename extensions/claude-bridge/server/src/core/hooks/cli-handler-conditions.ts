// The CLI keeps executing prompt/agent/mcp_tool handlers. Only their native
// argument predicate is supplied by Bridge, through a watched settings layer.
// --settings remains an immutable, per-process declaration snapshot.
import fs from 'node:fs'
import path from 'node:path'
import { listSession } from './registry'
import { conditionForCli, conditionHolds } from './hook-condition'
import type { HookInputPayload } from './types'
import type { PathRuleContext } from './path-rule'

export const HOOK_CONDITION_STATE = 'CLAUDE_BRIDGE_HOOK_CONDITIONS'
export const HOOK_CONDITION_SESSION = 'CLAUDE_BRIDGE_HOOK_SESSION'

export function hasCliHandlerConditions(sessionId: string): boolean {
  return listSession(sessionId).some(
    ({ event, handler }) =>
      ['prompt', 'agent', 'mcp_tool'].includes(handler.type) && conditionForCli(event, handler.if).serverRule,
  )
}

export function readHookConditionState(settingsFile: string): string | undefined {
  try {
    return JSON.parse(fs.readFileSync(settingsFile, 'utf8')).env?.[HOOK_CONDITION_STATE]
  } catch {
    // Launch settings are regenerated even if the previous generated file
    // was removed or interrupted. There is then no live marker to preserve.
    return undefined
  }
}

function writeState(settingsFile: string, sessionId: string, marker?: Record<string, unknown>): void {
  const settings = JSON.parse(fs.readFileSync(settingsFile, 'utf8'))
  const env = { ...settings.env }
  const states = JSON.parse(env[HOOK_CONDITION_STATE] || '{}')
  if (marker) states[sessionId] = marker
  else delete states[sessionId]
  if (Object.keys(states).length) env[HOOK_CONDITION_STATE] = JSON.stringify(states)
  else delete env[HOOK_CONDITION_STATE]
  settings.env = env
  // Each update merges the current file synchronously, including other live
  // sessions sharing a resumed conversation directory. The rename lets the
  // watcher read a complete JSON object.
  const temporary = path.join(path.dirname(settingsFile), `.bridge-hook-state-${sessionId}.json`)
  try {
    fs.writeFileSync(temporary, JSON.stringify(settings, null, 2))
    fs.renameSync(temporary, settingsFile)
  } finally {
    fs.rmSync(temporary, { force: true })
  }
}

export class CliHandlerConditions {
  private tail: Promise<void> = Promise.resolve()
  private current?: { generation: string; restored?: string; release: () => void }
  private closed = false
  private epoch = 0

  constructor(
    private sessionId: string,
    private settingsFile: string,
    private context: PathRuleContext,
  ) {}

  async prepare(
    event: string,
    payload: Partial<HookInputPayload>,
    generation: string,
    signal?: AbortSignal,
  ): Promise<{ generation?: string }> {
    const rules: Record<string, boolean> = {}
    for (const reg of listSession(this.sessionId)) {
      if (reg.event !== event || !['prompt', 'agent', 'mcp_tool'].includes(reg.handler.type)) continue
      const rule = conditionForCli(event, reg.handler.if).serverRule
      if (rule && reg.handler.if) {
        rules[reg.handler.if] = conditionHolds(
          rule,
          { ...payload, hook_event_name: event, session_id: this.sessionId },
          this.context,
        )
      }
    }
    if (!Object.keys(rules).length) return {}
    // Waiting in the UDS request, rather than a sandbox Promise queue, keeps a
    // slow original handler out of another mod event's ten-second own budget.
    const before = this.tail
    const epoch = this.epoch
    let release!: () => void
    this.tail = new Promise<void>((resolve) => {
      release = resolve
    })
    await before
    if (this.closed || epoch !== this.epoch || signal?.aborted) {
      release()
      throw new Error(this.closed ? 'Hook conditions disposed' : 'Hook conditions cancelled')
    }
    try {
      writeState(this.settingsFile, this.sessionId, { generation, event, rules })
      this.current = { generation, release }
      return { generation }
    } catch (error) {
      release()
      throw error
    }
  }

  restore(generation: string): { generation?: string } {
    if (this.current?.generation !== generation) return {}
    const restored = `${generation}:restore`
    writeState(this.settingsFile, this.sessionId, { generation: restored })
    this.current.restored = restored
    return { generation: restored }
  }

  release(generation: string): void {
    if (this.current?.generation !== generation) return
    const current = this.current
    try {
      if (!current.restored) writeState(this.settingsFile, this.sessionId, { generation: `${generation}:restore` })
    } finally {
      this.current = undefined
      current.release()
    }
  }

  reset(): void {
    ++this.epoch
    try {
      writeState(this.settingsFile, this.sessionId)
    } finally {
      this.current?.release()
      this.current = undefined
    }
  }

  dispose(): void {
    if (this.closed) return
    this.closed = true
    this.reset()
  }
}
