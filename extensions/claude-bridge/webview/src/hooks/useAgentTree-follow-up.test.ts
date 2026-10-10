import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createAgentTree, parseAgentEntries, parseAgentEntriesInto, markAllAgentsExited, scheduleCleanup, touchAgentAlive } from './useAgentTree'
import { tabAgentTrees, tabAgentHistory, findAgentByName } from '../signals/agents'
import { partitionAgents } from '../signals/agent-partition'
const stamp = (s: number) => new Date(1700000000000 + s * 1000).toISOString()
const use = (id: string, name: string, input: object, s: number) => ({type:'assistant', timestamp:stamp(s), message:{content:[{type:'tool_use', id, name, input}]}})
const result = (id: string, s: number, is_error = false, content = 'Message sent successfully') => ({type:'user', timestamp:stamp(s), message:{content:[{type:'tool_result', tool_use_id:id, content, is_error}]}})
const idle = (emitted: number, received = emitted) => ({type:'user', timestamp:stamp(received), message:{content:`<teammate-message teammate_id="worker">${JSON.stringify({type:'idle_notification', timestamp:stamp(emitted)})}</teammate-message>`}})
const report = (s: number) => ({type:'user', timestamp:stamp(s), message:{content:'<teammate-message teammate_id="worker">Synthetic follow-up report</teammate-message>'}})
const spawn = (team?: string) => [use('spawn', 'Agent', {name:'worker', team_name:team}, 1), result('spawn', 2, false, 'Spawned successfully.\nagent_id: worker@synthetic\nname: worker\nThe agent is now running and will receive instructions via mailbox.')]
const follow = (id = 'follow', s = 10) => [use(id, 'SendMessage', {recipient:'worker', type:'message', content:'Synthetic follow-up'}, s), result(id, s + 1)]
const agent = (tab = 'a') => findAgentByName(tabAgentTrees.value.get(tab), 'worker')
beforeEach(() => {vi.useFakeTimers(); tabAgentTrees.value = new Map(); tabAgentHistory.value = new Map()})
afterEach(() => {vi.clearAllTimers(); vi.useRealTimers()})
describe('persistent teammate follow-up (INC-2026-0056)', () => {
 for (const team of [undefined, 'synthetic-team']) {
  it(`represents spawn → idle → acknowledged follow-up → report → idle (${team ?? 'standalone'})`, () => {
   parseAgentEntries('a', spawn(team)); expect(agent()?.status).toBe('running')
   parseAgentEntries('a', [idle(3)]); expect(agent()?.status).toBe('done')
   parseAgentEntries('a', follow()); expect(agent()?.status).toBe('running')
   touchAgentAlive('a', 'worker'); expect(agent()?.status).toBe('running')
   parseAgentEntries('a', [report(12)]); expect(agent()?.status).toBe('running')
   parseAgentEntries('a', [idle(13)]); expect(agent()?.status).toBe('done')
  })
  it(`reactivates the original row after cleanup/replay completion (${team ?? 'standalone'})`, () => {
   parseAgentEntries('a', [...spawn(team), idle(3)]); const original = agent()
   scheduleCleanup('a'); vi.advanceTimersByTime(5000); markAllAgentsExited('a')
   expect(agent()).toBeUndefined(); parseAgentEntries('a', follow())
   expect(agent()).toBe(original); expect(agent()?.status).toBe('running')
   expect(partitionAgents(tabAgentTrees.value.get('a'), tabAgentHistory.value.get('a') ?? []).active.count).toBe(1)
  })
 }
 it('ignores an earlier emitted idle received after an acknowledged follow-up', () => {
  parseAgentEntries('a', [...spawn('team'), idle(3), ...follow(), idle(5, 20)])
  expect(agent()?.status).toBe('running')
  parseAgentEntries('a', [idle(21)]); expect(agent()?.status).toBe('done')
 })
 it('does not reopen on an unacknowledged/failed send or after shutdown', () => {
  parseAgentEntries('a', [...spawn('team'), idle(3), follow()[0]])
  expect(agent()?.status).toBe('done'); parseAgentEntries('a', [result('follow', 11, true)]); expect(agent()?.status).toBe('done')
  parseAgentEntries('a', [use('stop', 'SendMessage', {recipient:'worker', type:'shutdown_request', content:'shutdown_request'}, 12), ...follow('later', 13)])
  expect(agent()?.status).toBe('terminated')
 })
 it('keeps same-name teammates in different parent tabs separate', () => {
  parseAgentEntries('a', [...spawn('team'), idle(3)]); parseAgentEntries('b', [...spawn('team'), idle(3)])
  parseAgentEntries('a', follow()); expect(agent()?.status).toBe('running'); expect(agent('b')?.status).toBe('done')
 })
 it('reconnect replay and duplicate follow-up acknowledgements preserve the newest idle', () => {
  const entries = [...spawn('team'), idle(3), ...follow(), report(12), idle(13)]
  const tree = createAgentTree(); parseAgentEntriesInto(tree, entries); parseAgentEntriesInto(tree, follow())
  expect(findAgentByName(tree, 'worker')?.status).toBe('done')
  // Replay staging can receive newer transcript chunks before earlier ones.
  const staged = createAgentTree(); parseAgentEntriesInto(staged, entries.slice(3)); parseAgentEntriesInto(staged, entries.slice(0, 3))
  expect(findAgentByName(staged, 'worker')?.status).toBe('done')
 })
 it('handles an acknowledgement delivered before its tool call without reopening newer idle', () => {
  parseAgentEntries('a', [...spawn('team'), idle(3), result('follow', 11)])
  expect(agent()?.status).toBe('done'); parseAgentEntries('a', [follow()[0]])
  expect(agent()?.status).toBe('running'); parseAgentEntries('a', [idle(13), result('follow', 11)])
  expect(agent()?.status).toBe('done')
 })
 it('bounds unresolved call metadata and retains the newest follow-up', () => {
  parseAgentEntries('a', [...spawn('team'), idle(3), follow('old')[0]])
  parseAgentEntries('a', Array.from({length: 300}, (_, i) => result(`unrelated-${i}`, 12)))
  parseAgentEntries('a', [result('old', 11)]); expect(agent()?.status).toBe('done')
  parseAgentEntries('a', follow('new', 20)); expect(agent()?.status).toBe('running')
 })

})
