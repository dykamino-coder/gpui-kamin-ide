import assert from 'node:assert/strict'
import { test } from 'node:test'
import { WorkspacePresentation } from '../src/presentation.js'

test('host retains prompts and receipts, resolves them across views and never repeats accepted text', () => {
  const events: any[] = []
  const state = new WorkspacePresentation((channel, ...args) => {
    const frame = { kind: 'event', channel, args }
    events.push(frame)
    state.observe(frame)
  })
  const send = { kind: 'send', channel: 'pty-submit-text', args: ['tab', 'once', 'identity'] } as const
  assert.equal(state.accept({ ...send, args: [...send.args] }), true)
  assert.equal(state.accept({ ...send, args: [...send.args] }), false)
  assert.throws(() => state.accept({ ...send, args: ['tab', 'different', 'identity'] }), /identity/)
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'identity', state: 'native' }] })
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { capable: true }] })
  state.observe({ kind: 'event', channel: 'permission-request', args: ['tab', { requestId: 'permission', toolName: 'Bash' }] })
  state.observe({ kind: 'event', channel: 'mcp-activity', args: ['tab', { requestId: 'permission', status: 'pending' }] })
  assert.ok(state.snapshot().some(frame => frame.channel === 'permission-request'))
  assert.deepEqual((state.snapshot().find(frame => frame.channel === 'session:submission' && (frame.args as any)[1].id)?.args as any)[1],
    { id: 'identity', text: 'once', state: 'native', cancelable: true,
      fallbackQueue: { afterOrd: -1, enqueuedAt: (events[0].args[1] as any).fallbackQueue.enqueuedAt } })
  assert.equal(state.accept({ kind: 'send', channel: 'permission-response', args: ['permission', 'allow'] }), true)
  assert.equal(state.accept({ kind: 'send', channel: 'permission-response', args: ['permission', 'deny'] }), false)
  assert.equal(state.snapshot().some(frame => frame.channel === 'permission-request'), false)
  assert.equal(events.filter(frame => frame.channel === 'workspace:question-resolved').length, 1)
  state.observe({ kind: 'event', channel: 'mcp-activity', args: ['tab', { requestId: 'permission', status: 'completed' }] })
  assert.equal(state.snapshot().some(frame => frame.channel === 'mcp-activity'), false)
  state.observe({ kind: 'event', channel: 'tab:closed', args: ['tab'] })
  assert.deepEqual(state.snapshot(), [])
  state.observe({ kind: 'event', channel: 'permission-request', args: ['another-tab', { requestId: 'disconnected' }] })
  state.forgetSessions()
  assert.deepEqual(state.snapshot(), [])
})

test('fallback receipts retain ingress cursor and consumption with zero views and trimmed history', () => {
  let entries: any[] = [{ type: 'user', uuid: 'old', _ord: 10, message: { content: 'repeat this message' } }]
  const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }), () => entries)
  const receipt = (id: string) => (state.snapshot().find(frame => frame.channel === 'session:submission' && (frame.args as any)[1].id === id)!.args as any)[1]
  for (const id of ['first', 'second']) {
    state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'repeat this message', id] })
    state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id, state: 'fallback' }] })
    assert.equal(receipt(id).fallbackQueue.afterOrd, 10)
    assert.equal(receipt(id).fallbackQueue.consumed, undefined, 'Historical matching prose must not consume a new queue item')
  }
  entries = [{ type: 'user', uuid: 'new-one', _ord: 11, message: { content: 'repeat this message' } }]
  const observe = () => state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', entries] })
  observe()
  assert.equal(receipt('first').fallbackQueue.consumed, true)
  assert.equal(receipt('second').fallbackQueue.consumed, undefined)
  observe()
  assert.equal(receipt('second').fallbackQueue.consumed, undefined, 'A repeated JSONL batch cannot accept identical prose twice')
  entries = [{ type: 'attachment', uuid: 'new-two', _ord: 12, attachment: { type: 'queued_command', prompt: 'repeat this message' } }]
  observe()
  assert.equal(receipt('second').fallbackQueue.consumed, true)
  entries = [] // A later page may have no matching transcript row left to replay.
  assert.ok(['first', 'second'].every(id => receipt(id).fallbackQueue.consumed))
  assert.equal(state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'repeat this message', 'first'] }), false)
  state.observe({ kind: 'event', channel: 'tab:closed', args: ['tab'] })
  assert.deepEqual(state.snapshot(), [])
})

test('JSONL arriving before fallback is reconciled from the shared cache', () => {
  let entries: any[] = []
  const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }), () => entries)
  state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'late fallback receipt', 'id'] })
  entries = [{ type: 'user', uuid: 'stored', _ord: 1, message: { content: 'late fallback receipt' } }]
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', entries] })
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'id', state: 'fallback' }] })
  assert.equal((state.snapshot()[0].args as any)[1].fallbackQueue.consumed, true)
})
