// Гонки страниц и потеря ответа не должны затирать текст или повторять записи.
import { describe, expect, it } from 'vitest'
import { WorkspaceDraftClient } from './workspace-draft-client'
import type { WorkspaceDraftReply, WorkspaceDraftState } from '../../shared/workspace-draft'

const value = (text: string) => ({ text, attachments: [] })
const state = (revision: number, text: string) => ({ revision, value: value(text) })
const tick = async () => { for (let i = 0; i < 5; i++) await Promise.resolve() }
function fixture() {
  let saved: WorkspaceDraftState = state(0, '')
  const calls: { base: number; text: string; resolve: (reply: WorkspaceDraftReply) => void; reject: (error: Error) => void }[] = []
  const rendered: string[] = []
  const client = new WorkspaceDraftClient({ get: async () => saved,
    update: (_id, base, draft) => new Promise((resolve, reject) => calls.push({ base, text: draft.text, resolve, reject })) })
  client.watch('tab', draft => rendered.push(draft.text))
  return { client, calls, rendered, saved: (next: WorkspaceDraftState) => { saved = next } }
}
describe('host-owned composer drafts', () => {
  it('coalesces local edits with CAS while accepted events never overwrite newer typing', async () => {
    const { client, calls, rendered } = fixture()
    await tick()
    client.save('tab', value('first'))
    client.save('tab', value('newer'))
    client.observe('tab', state(1, 'first'))
    expect(rendered).toEqual([''])
    calls[0].resolve({ accepted: true, state: state(1, 'first') })
    await tick()
    expect(calls.map(call => [call.base, call.text])).toEqual([[0, 'first'], [1, 'newer']])
    calls[1].resolve({ accepted: true, state: state(2, 'newer') })
    await tick()
    expect(client.status('tab')).toBe('saved')
    client.observe('tab', state(3, 'another page'))
    expect(rendered.at(-1)).toBe('another page')
  })
  it('retains conflicting local text and requires an explicit choice before overwriting another page', async () => {
    const { client, calls, rendered } = fixture()
    await tick()
    client.save('tab', value('local'))
    client.observe('tab', state(1, 'remote'))
    calls[0].resolve({ accepted: false, state: state(1, 'remote') })
    await tick()
    expect(client.status('tab')).toBe('conflict')
    expect(rendered).toEqual([''])
    client.save('tab', value('local edited'))
    expect(calls).toHaveLength(1)
    client.resolve('tab', true)
    expect(calls[1].base).toBe(1)
    calls[1].resolve({ accepted: true, state: state(2, 'local edited') })
    await tick()
    expect(client.status('tab')).toBe('saved')
    client.save('tab', value('conflicting again'))
    calls[2].resolve({ accepted: false, state: state(3, 'remote latest') })
    await tick()
    client.resolve('tab', false)
    expect(rendered.at(-1)).toBe('remote latest')
    expect(calls).toHaveLength(3)
  })
  it('reads after disconnect without replaying uncertain writes, including an already committed clear', async () => {
    const { client, calls, saved, rendered } = fixture()
    await tick()
    client.save('tab', value('uncertain'))
    client.disconnected()
    calls[0].reject(new Error('lost reply'))
    await tick()
    expect(client.status('tab')).toBe('unsaved')
    saved(state(1, 'other page'))
    client.refresh(); await tick()
    expect(client.status('tab')).toBe('conflict')
    expect(calls).toHaveLength(1)
    client.resolve('tab', false)
    client.save('tab', value(''))
    calls[1].reject(new Error('lost reply after commit'))
    await tick()
    saved(state(2, ''))
    client.refresh(); await tick()
    expect(client.status('tab')).toBe('saved')
    expect(rendered.at(-1)).toBe('')
    expect(calls).toHaveLength(2)
  })
})
