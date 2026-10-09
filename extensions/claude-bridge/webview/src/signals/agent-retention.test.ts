import { describe, expect, it } from 'vitest'
import { AGENT_RETENTION, boundAgentTranscript, boundAgentSlots } from './agent-retention'
import { agentTranscriptDownloadId, subagentTileState, type SubagentTileState } from './agents'
describe('recent agent cache budget and full-export identity', () => {
  it('bounds oversized payloads and total bytes without truncating a record', () => {
    const states: SubagentTileState[] = []
    for (let i = 0; i < 20; i++) {
      const state = { tileKey: '', entries: [{ uuid: String(i), text: 'x'.repeat(3 * 1024 * 1024) }] }
      boundAgentTranscript(state)
      states.push(state)
    }
    const map = boundAgentSlots(new Map(states.map((state, i) => [String(i), state])))
    expect([...map.values()].reduce((sum, state) => sum + (state.retainedBytes ?? 0), 0)).toBeLessThanOrEqual(AGENT_RETENTION.totalBytes)
    const huge: SubagentTileState = { tileKey: '', entries: [{ uuid: 'huge', text: 'x'.repeat(AGENT_RETENTION.slotBytes) }, { uuid: 'small' }] }
    boundAgentTranscript(huge)
    expect(huge.entries).toEqual([{ uuid: 'small' }])
    expect(huge.seen?.size).toBe(1)
  })
  it('exports full history by file ID after cache eviction and rejects ambiguous mailbox aliases', () => {
    subagentTileState.value = new Map()
    expect(agentTranscriptDownloadId('a', 'worker', undefined, 'agent-known-file')).toBe('known-file')
    subagentTileState.value = new Map([
      ['one', { tileKey: '', tabId: 'a', agentName: 'worker', agentId: 'agent-one', entries: [] }],
      ['other-tab', { tileKey: '', tabId: 'b', agentName: 'worker', agentId: 'agent-other', entries: [] }],
    ])
    expect(agentTranscriptDownloadId('a', 'worker', undefined, 'worker@team')).toBe('one')
    subagentTileState.value = new Map([...subagentTileState.value, ['two', { tileKey: '', tabId: 'a', agentName: 'worker', agentId: 'agent-two', entries: [] }]])
    expect(agentTranscriptDownloadId('a', 'worker', undefined, 'worker@team')).toBeUndefined()
  })
})
