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
    { id: 'identity', text: 'once', state: 'native', cancelable: true })
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
