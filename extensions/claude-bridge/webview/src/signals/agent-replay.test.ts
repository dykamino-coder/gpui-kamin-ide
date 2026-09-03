// Replay of a session's transcript is collected per generation and published
// as ONE snapshot: the panel never sees an empty tree followed by partial
// chunks, an older or interrupted replay cannot land on top of a newer one,
// and entries that arrive on the boundary are neither lost nor doubled.
import { describe, it, expect, beforeEach } from 'vitest'
import { beginAgentReplay, stageAgentEntries, publishAgentReplay, isAgentReplayStaging, abandonAgentReplay, stagedAgentEntryCount } from './agent-replay'
import { tabAgentTrees, tabAgentHistory, tabJsonlLive, findAgentByName } from './agents'
import { partitionAgents } from './agent-partition'
import { parseAgentEntries, markAllAgentsExited } from '../hooks/useAgentTree'

const TAB = 'tab-r'
let seq = 0
const ts = (): string => new Date(1_700_000_000_000 + (seq += 1000)).toISOString()
const ord = (): number => seq / 1000

function agentCall(name: string, team = 'br26'): any {
  return {
    type: 'assistant', uuid: `u-${name}`, timestamp: ts(), _ord: ord(),
    message: { role: 'assistant', content: [{ type: 'tool_use', id: `toolu_${name}`, name: 'Agent', input: { description: `${name} task`, name, team_name: team, subagent_type: 'general-purpose', prompt: 'x' } }] },
  }
}
function spawnEcho(name: string): any {
  return {
    type: 'user', uuid: `u-res-${name}`, timestamp: ts(), _ord: ord(),
    message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `toolu_${name}`, content: [{ type: 'text', text: `Spawned successfully.\nagent_id: ${name}@session-x\nname: ${name}\nrunning.` }] }] },
  }
}
function idle(name: string): any {
  return {
    type: 'user', uuid: `u-idle-${name}-${seq}`, timestamp: ts(), _ord: ord(),
    message: { role: 'user', content: `Another Claude session sent a message:\n<teammate-message teammate_id="${name}" color="blue">\n${JSON.stringify({ type: 'idle_notification', from: name, timestamp: ts(), idleReason: 'available' })}\n</teammate-message>` },
  }
}
const statusOf = (name: string): string | undefined => findAgentByName(tabAgentTrees.value.get(TAB), name)?.status
const panel = () => partitionAgents(tabAgentTrees.value.get(TAB), tabAgentHistory.value.get(TAB) ?? [])
const snapshotNames = (): string[] => {
  const t = tabAgentTrees.value.get(TAB)
  return t ? [...t.teams.values()].flatMap((team) => [...team.agents.keys()]) : []
}

beforeEach(() => {
  tabAgentTrees.value = new Map()
  tabAgentHistory.value = new Map()
  tabJsonlLive.value = new Set()
  abandonAgentReplay(TAB)
})

describe('generation-scoped agent replay', () => {
  it('cold hydration publishes once, in transcript order, from newest-first chunks', () => {
    const transcript = [agentCall('a'), spawnEcho('a'), agentCall('b'), spawnEcho('b'), idle('a'), idle('b')]
    const gen = beginAgentReplay(TAB)
    expect(stageAgentEntries(TAB, transcript.slice(4))).toBe(true) // tail first
    expect(tabAgentTrees.value.get(TAB)).toBeUndefined() // nothing published yet
    expect(stageAgentEntries(TAB, transcript.slice(0, 4))).toBe(true)
    expect(tabAgentTrees.value.get(TAB)).toBeUndefined()
    expect(publishAgentReplay(TAB, gen)).toBe(true)
    expect(statusOf('a')).toBe('done')
    expect(statusOf('b')).toBe('done')
    expect(isAgentReplayStaging(TAB)).toBe(false)
  })

  it('a resync keeps the last consistent snapshot visible until replayComplete', () => {
    parseAgentEntries(TAB, [agentCall('a'), spawnEcho('a'), agentCall('b'), spawnEcho('b')])
    expect(panel().active.count).toBe(2)
    const gen = beginAgentReplay(TAB)
    stageAgentEntries(TAB, [agentCall('a'), spawnEcho('a')]) // partial chunk
    expect(panel().active.count).toBe(2) // no flicker: old snapshot still published
    expect(snapshotNames()).toEqual(['a', 'b'])
    stageAgentEntries(TAB, [agentCall('b'), spawnEcho('b')])
    publishAgentReplay(TAB, gen)
    expect(panel().active.count).toBe(2)
    expect(snapshotNames()).toEqual(['a', 'b'])
  })

  it('ten finished agents arrive as one consistent history update', () => {
    const names = Array.from({ length: 10 }, (_, i) => `w${i}`)
    const transcript = names.flatMap((n) => [agentCall(n), spawnEcho(n)]).concat(names.map((n) => idle(n)))
    const gen = beginAgentReplay(TAB)
    for (let i = 0; i < transcript.length; i += 7) stageAgentEntries(TAB, transcript.slice(i, i + 7))
    expect(tabAgentTrees.value.get(TAB)).toBeUndefined()
    publishAgentReplay(TAB, gen)
    expect(panel().completed.count).toBe(10)
    expect(panel().active.count).toBe(0)
    markAllAgentsExited(TAB) // replay→live hand-off moves them to history
    expect((tabAgentHistory.value.get(TAB) ?? []).length).toBe(10)
    expect(panel().completed.count).toBe(10) // still one consistent view
  })

  it('a live entry between the last chunk and completion is kept exactly once', () => {
    const gen = beginAgentReplay(TAB)
    stageAgentEntries(TAB, [agentCall('a'), spawnEcho('a')])
    stageAgentEntries(TAB, [idle('a')]) // live entry, lands before replayComplete
    expect(stagedAgentEntryCount(TAB)).toBe(3)
    publishAgentReplay(TAB, gen)
    expect(statusOf('a')).toBe('done')
    expect(panel().completed.count).toBe(1)
    expect(panel().active.count).toBe(0)
  })

  it('an older generation cannot publish over a newer replay', () => {
    const gen1 = beginAgentReplay(TAB)
    stageAgentEntries(TAB, [agentCall('old'), spawnEcho('old')])
    const gen2 = beginAgentReplay(TAB) // reconnect restarted the replay
    expect(stagedAgentEntryCount(TAB)).toBe(0)
    stageAgentEntries(TAB, [agentCall('new'), spawnEcho('new')])
    expect(publishAgentReplay(TAB, gen1)).toBe(false)
    expect(tabAgentTrees.value.get(TAB)).toBeUndefined()
    expect(publishAgentReplay(TAB, gen2)).toBe(true)
    expect(snapshotNames()).toEqual(['new'])
  })

  it('a genuinely empty session publishes an empty snapshot', () => {
    const gen = beginAgentReplay(TAB)
    publishAgentReplay(TAB, gen)
    expect(tabAgentTrees.value.get(TAB)?.teams.size).toBe(0)
    expect(panel().active.count + panel().completed.count).toBe(0)
  })

  it('entries received while no replay is open are parsed live', () => {
    expect(stageAgentEntries(TAB, [agentCall('a')])).toBe(false)
    parseAgentEntries(TAB, [agentCall('a'), spawnEcho('a')])
    expect(statusOf('a')).toBe('running')
  })
})
