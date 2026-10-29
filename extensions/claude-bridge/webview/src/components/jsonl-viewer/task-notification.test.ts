import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { parseTaskNotification, taskNotificationText } from './task-notification'
import { groupConsecutiveToolCalls } from './utils'
import { entryIsVisible } from './prepare-jsonl-entries'

const xml = (fields = '', status = 'completed') => `<task-notification><task-id>task-42</task-id><status>${status}</status><summary>Build &amp; test</summary>${fields}</task-notification>`
const entry = (content: string, extra = {}) => ({ type: 'user', uuid: 'notice', message: { role: 'user', content }, ...extra }) as JsonlEntryData
const tool = (name: string) => ({ type: 'assistant', uuid: name, message: { id: `msg-${name}`, content: [{ type: 'tool_use', id: name, name, input: {} }] } }) as JsonlEntryData

describe('task notification payload', () => {
  it.each(['completed', 'failed', 'running'])('parses %s without a result', status => {
    expect(parseTaskNotification(xml('', status))).toEqual({ summary: 'Build & test', status, taskId: 'task-42', result: undefined, fields: [] })
  })
  it('preserves code indentation, literal tags and comparisons in result', () => {
    const code = '\n  if (a < b && b > 0) {\n    return "<div>result</div>"\n  }\n'
    expect(parseTaskNotification(xml(`<result>${code}</result>`))?.result).toBe(code)
  })
  it('decodes entities once, safely, including numeric Unicode', () => {
    expect(parseTaskNotification(xml('<result>&lt;script&gt;&amp;lt; &#x1f600; &#60; &quot; &apos;</result>'))?.result).toBe('<script>&lt; 😀 < " \'')
  })
  it('preserves CDATA and metadata', () => {
    const task = parseTaskNotification(xml('<result><![CDATA[a &lt; b]]></result><output-file>/tmp/out</output-file><tool-use-id>tool-1</tool-use-id><usage><tokens>24</tokens></usage>'))!
    expect(task.result).toBe('a &lt; b')
    expect(task.fields).toEqual([['output-file', '/tmp/out'], ['tool-use-id', 'tool-1'], ['usage', '<tokens>24</tokens>']])
  })
  it.each([xml('<unknown>value</unknown>'), xml('<result>broken'), xml('', 'unknown'), xml('<status>failed</status>'), '<other>text</other>'])('falls back for unknown or broken XML', text => {
    expect(parseTaskNotification(text)).toBeNull()
  })
  it('recognizes string and text-block service records, including malformed payloads', () => {
    expect(taskNotificationText(entry(xml()))).toBe(xml())
    expect(taskNotificationText(entry('', { message: { content: [{ type: 'text', text: xml() }] } }))).toBe(xml())
    expect(taskNotificationText(entry('<task-notification>broken'))).not.toBeNull()
    expect(taskNotificationText(entry(xml() + '\nHuman instruction'))).toBeNull()
  })
})

describe('tool bursts across CLI service records', () => {
  it('groups different API message ids across notifications, metadata and system events', () => {
    const notice = entry(xml(), { isMeta: true })
    const records = [tool('Read'), { type: 'system', subtype: 'turn_duration', message: { content: 'timing' } }, tool('Bash'), notice, { type: 'user', isMeta: true, message: { content: 'service hint' } }, tool('Edit'), { type: 'system', subtype: 'local_command', content: 'Background task completed' }, tool('TaskOutput') ] as JsonlEntryData[]
    const grouped = groupConsecutiveToolCalls(records)
    expect(grouped.map(e => e.type)).toEqual(['tool_group', 'user', 'system'])
    expect(grouped[0]._groupEntries?.filter(e => e.type === 'assistant').map(e => e.message?.id)).toEqual(['msg-Read', 'msg-Bash', 'msg-Edit', 'msg-TaskOutput'])
    expect(grouped[1]).toBe(notice)
    expect(entryIsVisible(notice, { uuidsInRange: null, range: undefined, entries: records, toolResults: undefined, isLatestSegment: true, chainUuids: null })).toBe(true)
  })
  it.each([entry('Human prompt'), { type: 'assistant', message: { content: [{ type: 'text', text: 'Explanation' }] } }, { type: 'system', subtype: 'compact_boundary' }])('preserves conversation boundaries', boundary => {
    expect(groupConsecutiveToolCalls([tool('Read'), boundary as JsonlEntryData, tool('Bash')]).filter(e => e.type === 'tool_group')).toHaveLength(2)
  })
  it('does not absorb a live streaming tool into a finalized burst', () => {
    const live = { ...tool('Bash'), __streaming: true }
    expect(groupConsecutiveToolCalls([tool('Read'), live])[1]).toBe(live)
  })
})
