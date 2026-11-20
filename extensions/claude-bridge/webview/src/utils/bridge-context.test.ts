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

  it('hides a block whose history quotes an earlier block', () => {
    const quoted = `<bridge-context>\nBridge note…\n<conversation-history>\nUser: hi\nAssistant: echo: hi ${block} [model=gpt]\n</conversation-history>\n</bridge-context>`
    expect(withoutBridgeContext(`next question\n\n${quoted}`)).toBe('next question')
  })

  it('cleans user rows and answers that quote the block, with string and block content', () => {
    const str = { type: 'user', uuid: 'u1', message: { role: 'user', content: `hi\n\n${block}` } } as JsonlEntryData
    const blocks = {
      type: 'user',
      uuid: 'u2',
      message: { role: 'user', content: [{ type: 'text', text: `hi\n\n${block}` }] },
    } as unknown as JsonlEntryData
    const reply = {
      type: 'assistant',
      uuid: 'a1',
      message: { role: 'assistant', content: [{ type: 'text', text: `Your message was:\n${block}\nDone.` }] },
    } as unknown as JsonlEntryData
    const plain = { type: 'assistant', uuid: 'a2', message: { role: 'assistant', content: block } } as JsonlEntryData
    const tool = {
      type: 'assistant',
      uuid: 'a3',
      message: { role: 'assistant', content: [{ type: 'tool_use', id: 't', name: 'Read', input: { note: block } }] },
    } as unknown as JsonlEntryData
    for (const e of [str, blocks, reply, plain, tool]) stripBridgeContext(e)
    expect(str.message!.content).toBe('hi')
    expect((blocks.message!.content as Array<{ text: string }>)[0]!.text).toBe('hi')
    expect((reply.message!.content as Array<{ text: string }>)[0]!.text).toBe('Your message was:\n\n\nDone.')
    expect(plain.message!.content).toBe('')
    // Tool input is data, not chat text: left as the model wrote it.
    expect((tool.message!.content as Array<{ input: { note: string } }>)[0]!.input.note).toBe(block)
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
