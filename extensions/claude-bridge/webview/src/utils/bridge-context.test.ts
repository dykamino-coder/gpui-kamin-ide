import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../types/jsonl'
import { stripBridgeContext, withoutBridgeContext } from './bridge-context'
import { appendJsonlEntries, jsonlEntriesByTab } from '../signals/jsonl'
import { receiveSubmission, submissions } from '../signals/submissions'

const block = '<bridge-context>\nBridge note…\n<conversation-history>\nUser: old\n</conversation-history>\n</bridge-context>'

describe('bridge context in the chat', () => {
  it('shows only what the user wrote', () => {
    expect(withoutBridgeContext(`what was it?\n\n${block}`)).toBe('what was it?')
    expect(withoutBridgeContext('plain <b>text</b>')).toBe('plain <b>text</b>')
  })

  it('cleans user rows with string and block content, leaves assistants alone', () => {
    const str = { type: 'user', uuid: 'u1', message: { role: 'user', content: `hi\n\n${block}` } } as JsonlEntryData
    const blocks = {
      type: 'user',
      uuid: 'u2',
      message: { role: 'user', content: [{ type: 'text', text: `hi\n\n${block}` }] },
    } as unknown as JsonlEntryData
    const reply = { type: 'assistant', uuid: 'a1', message: { role: 'assistant', content: block } } as JsonlEntryData
    for (const e of [str, blocks, reply]) stripBridgeContext(e)
    expect(str.message!.content).toBe('hi')
    expect((blocks.message!.content as Array<{ text: string }>)[0]!.text).toBe('hi')
    expect(reply.message!.content).toBe(block)
  })

  it('the store and the receipts never hold the hidden block', () => {
    appendJsonlEntries('t-bc', [
      { type: 'user', uuid: 'u1', parentUuid: null, message: { role: 'user', content: `hi\n\n${block}` } } as unknown as JsonlEntryData,
    ])
    expect(jsonlEntriesByTab.value.get('t-bc')?.[0]?.message?.content).toBe('hi')
    receiveSubmission('t-bc', { id: 's1', state: 'read', text: `hi\n\n${block}` } as never)
    expect(submissions.value.get('t-bc')?.[0]?.text).toBe('hi')
  })
})
