// Teammate lifecycle as it really arrives on the wire. The server's
// `leanEntries()` strips `toolUseResult` before `jsonl:entries` leaves the
// process, so these fixtures carry only what the renderer can see: the Agent
// tool_use, its tool_result TEXT, and the STRING-content user entries that the
// CLI writes for `<teammate-message>` / `<task-notification>`. Shapes are copied
// from the INC-2026-0002 gate transcript.
import { describe, it, expect, beforeEach } from 'vitest'
import { parseAgentEntries, markAllAgentsExited } from './useAgentTree'
import { tabAgentTrees, tabAgentHistory, tabJsonlLive, findAgentByName } from '../signals/agents'
import { partitionAgents } from '../signals/agent-partition'

const TAB = 'tab-1'
let seq = 0
const ts = (): string => new Date(1_700_000_000_000 + (seq += 1000)).toISOString()

function agentCall(name: string, teamName?: string, id = `toolu_${name}`): any {
  return {
    type: 'assistant', uuid: `u-${id}`, timestamp: ts(),
    message: { role: 'assistant', content: [{ type: 'tool_use', id, name: 'Agent', input: { description: `${name} task`, name, team_name: teamName, subagent_type: 'general-purpose', prompt: 'x' } }] },
  }
}

/** The teammate spawn echo: on the wire only the text survives (no toolUseResult). */
function spawnEcho(name: string, id = `toolu_${name}`): any {
  return {
    type: 'user', uuid: `u-res-${id}`, timestamp: ts(),
    message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: id, content: [{ type: 'text', text: `Spawned successfully. (internal metadata)\nagent_id: ${name}@session-abc\nname: ${name}\nThe agent is now running and will receive instructions via mailbox.` }] }] },
  }
}

function teammateMessage(name: string, body: string, summary?: string): any {
  const attrs = `teammate_id="${name}" color="blue"${summary ? ` summary="${summary}"` : ''}`
  return {
    type: 'user', uuid: `u-tm-${seq}`, timestamp: ts(),
    message: { role: 'user', content: `Another Claude session sent a message:\n<teammate-message ${attrs}>\n${body}\n</teammate-message>\n\nThis came from another Claude session.` },
  }
}

const idle = (name: string): any => teammateMessage(name, JSON.stringify({ type: 'idle_notification', from: name, timestamp: ts(), idleReason: 'available' }))
const report = (name: string): any => teammateMessage(name, `${name} status = done.`, `${name} final report`)
const kicked = (name: string): any => teammateMessage(name, JSON.stringify({ type: 'shutdown_approved', from: name }))

function taskNotification(taskId: string, status: string): any {
  return {
    type: 'user', uuid: `u-tn-${seq}`, timestamp: ts(),
    message: { role: 'user', content: `<task-notification>\n<task-id>${taskId}</task-id>\n<status>${status}</status>\n<summary>done</summary>\n</task-notification>` },
  }
}

function asyncLaunch(name: string, id: string, agentId: string): any {
  return {
    type: 'user', uuid: `u-al-${id}`, timestamp: ts(),
    message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: id, content: `Async agent launched successfully.\nagentId: ${agentId}\nname: ${name}` }] },
  }
}

function statusOf(name: string): string | undefined {
  return findAgentByName(tabAgentTrees.value.get(TAB), name)?.status
}

function panel() {
  return partitionAgents(tabAgentTrees.value.get(TAB), tabAgentHistory.value.get(TAB) ?? [])
}

beforeEach(() => {
  tabAgentTrees.value = new Map()
  tabAgentHistory.value = new Map()
  tabJsonlLive.value = new Set()
})

describe('teammate lifecycle from wire entries', () => {
  it('a spawned teammate is running, not done, until its idle notification', () => {
    parseAgentEntries(TAB, [agentCall('alpha', 'br25'), spawnEcho('alpha')])
    expect(statusOf('alpha')).toBe('running')
    expect(panel().active.count).toBe(1)
    parseAgentEntries(TAB, [report('alpha'), idle('alpha')])
    expect(statusOf('alpha')).toBe('done')
    const p = panel()
    expect(p.active.count).toBe(0)
    expect(p.completed.count).toBe(1)
    expect(p.completed.teams[0].name).toBe('br25')
  })

  it('a kicked teammate ends as terminated', () => {
    parseAgentEntries(TAB, [agentCall('beta', 'br25'), spawnEcho('beta'), kicked('beta')])
    expect(statusOf('beta')).toBe('terminated')
  })

  it('finishes teammates in a different order than they were spawned', () => {
    parseAgentEntries(TAB, [agentCall('alpha', 'br25'), spawnEcho('alpha'), agentCall('beta', 'br25'), spawnEcho('beta')])
    parseAgentEntries(TAB, [idle('beta')])
    expect(statusOf('beta')).toBe('done')
    expect(statusOf('alpha')).toBe('running')
    let p = panel()
    expect(p.active.teams[0].members.map((m) => m.name)).toEqual(['alpha'])
    expect(p.completed.teams[0].members.map((m) => m.name)).toEqual(['beta'])
    parseAgentEntries(TAB, [idle('alpha')])
    p = panel()
    expect(p.active.count).toBe(0)
    expect(p.completed.count).toBe(2)
  })

  it('replay delivered newest-batch-first still closes the lifecycle', () => {
    const entries = [agentCall('alpha', 'br25'), spawnEcho('alpha'), agentCall('beta', 'br25'), spawnEcho('beta'), report('alpha'), idle('alpha'), report('beta'), idle('beta')]
    // the extension's replay emits the transcript tail first, chunk by chunk
    parseAgentEntries(TAB, entries.slice(4))
    parseAgentEntries(TAB, entries.slice(0, 4))
    expect(statusOf('alpha')).toBe('done')
    expect(statusOf('beta')).toBe('done')
    expect(panel().active.count).toBe(0)
    expect(panel().completed.count).toBe(2)
    expect(tabAgentTrees.value.get(TAB)!.standaloneAgents.size).toBe(0) // synthetic records folded in
  })

  it('a repeated replay does not duplicate rows or reopen a finished teammate', () => {
    const entries = [agentCall('alpha', 'br25'), spawnEcho('alpha'), idle('alpha')]
    parseAgentEntries(TAB, entries)
    parseAgentEntries(TAB, entries)
    const p = panel()
    expect(p.completed.count).toBe(1)
    expect(p.active.count).toBe(0)
    expect(statusOf('alpha')).toBe('done')
  })

  it('replayComplete moves finished teammates to history without changing the panel', () => {
    parseAgentEntries(TAB, [agentCall('alpha', 'br25'), spawnEcho('alpha'), agentCall('beta', 'br25'), spawnEcho('beta'), idle('alpha')])
    const before = panel()
    markAllAgentsExited(TAB)
    const after = panel()
    expect((tabAgentHistory.value.get(TAB) ?? []).map((a) => a.name)).toEqual(['alpha'])
    expect(after.active.teams[0].members.map((m) => m.name)).toEqual(before.active.teams[0].members.map((m) => m.name))
    expect(after.completed.teams[0].members.map((m) => `${m.name}:${m.status}`)).toEqual(['alpha:done'])
    expect(after.active.count + after.completed.count).toBe(before.active.count + before.completed.count)
  })

  it('a background agent finishes through its string-content task notification', () => {
    parseAgentEntries(TAB, [agentCall('worker', undefined, 'toolu_bg'), asyncLaunch('worker', 'toolu_bg', 'agent-42')])
    expect(statusOf('worker')).toBe('running')
    parseAgentEntries(TAB, [taskNotification('agent-42', 'completed')])
    expect(statusOf('worker')).toBe('done')
  })

  it('a blocking subagent completes on its tool_result', () => {
    parseAgentEntries(TAB, [agentCall('reader'), {
      type: 'user', uuid: 'u-r', timestamp: ts(),
      message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: 'toolu_reader', content: 'summary text' }] },
    }])
    expect(statusOf('reader')).toBe('done')
  })
})
