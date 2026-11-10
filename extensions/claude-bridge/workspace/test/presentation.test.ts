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

test('native receipt commitment survives zero views and transcript eviction in either event order', () => {
  for (const order of ['receipt-first', 'row-first']) {
    let entries: any[] = []
    const events: any[] = []
    const state = new WorkspacePresentation((channel, ...args) => {
      events.push({ channel, args })
      state.observe({ kind: 'event', channel, args })
    }, () => entries)
    const receipt = () => (state.snapshot().find(frame => frame.channel === 'session:submission')!.args as any)[1]
    state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'same prose', 'id'] })
    const native = () => state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'id', state: 'read', uuid: 'stored' }] })
    if (order === 'receipt-first') native()
    // Equal prose and a different session's UUID cannot commit this receipt.
    state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['other', [{ type: 'user', uuid: 'stored' }]] })
    state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [{ type: 'user', uuid: 'different', message: { content: 'same prose' } }]] })
    assert.notEqual(receipt().committed, true)
    entries = [{ type: 'user', uuid: 'stored', message: { content: 'same prose' } }]
    state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', entries] })
    if (order === 'row-first') native()
    assert.equal(receipt().committed, true, order)
    entries = []
    native() // Late duplicate receipts must never undo host commitment.
    assert.equal(receipt().committed, true)
    assert.equal(events.filter(frame => frame.args[1]?.committed).length, 1)
    state.observe({ kind: 'event', channel: 'tab:closed', args: ['tab'] })
    assert.deepEqual(state.snapshot(), [])
  }
})

test('Escape dismisses only selected queue displays and retains delivery matching across pages', () => {
  const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }))
  const receipt = (tab: string, id: string) => (state.snapshot().find(frame => frame.channel === 'session:submission'
    && (frame.args as any)[0] === tab && (frame.args as any)[1].id === id)!.args as any)[1]
  for (const tab of ['tab', 'other']) for (const id of ['first', 'second']) {
    state.accept({ kind: 'send', channel: 'pty-submit-text', args: [tab, 'same queued text', id] })
    state.observe({ kind: 'event', channel: 'session:submission', args: [tab, { id, state: 'fallback' }] })
  }
  // A plain Stop interrupts without restoring any queue text.
  state.accept({ kind: 'send', channel: 'session:interrupt', args: ['tab'] })
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, undefined)
  state.accept({ kind: 'send', channel: 'session:interrupt', args: ['tab', ['first', 'unknown', 42]] })
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, true)
  assert.equal(receipt('tab', 'first').state, 'fallback', 'Display dismissal is not CLI delivery cancellation')
  assert.equal(receipt('tab', 'second').fallbackQueue.dismissed, undefined)
  assert.equal(receipt('other', 'first').fallbackQueue.dismissed, undefined)
  // A late duplicate cannot reset host-owned dismissal, even with old metadata.
  const queue = receipt('tab', 'first').fallbackQueue
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'first', state: 'fallback',
    fallbackQueue: { afterOrd: queue.afterOrd, enqueuedAt: queue.enqueuedAt } }] })
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, true)
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [{ type: 'user', uuid: 'accepted-first',
    _ord: 1, message: { content: 'same queued text' } }]] })
  assert.equal(receipt('tab', 'first').fallbackQueue.consumed, true)
  assert.equal(receipt('tab', 'second').fallbackQueue.consumed, undefined, 'Dismissal cannot give its row to a later identical prompt')
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, true)
  state.observe({ kind: 'event', channel: 'tab:closed', args: ['tab'] })
  assert.ok(state.snapshot().every(frame => (frame.args as any)[0] !== 'tab'))
})

test('queue dismissal survives fallback arriving after Escape', () => {
  const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }))
  state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'pending fallback', 'id'] })
  state.accept({ kind: 'send', channel: 'session:interrupt', args: ['tab', ['id']] })
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'id', state: 'fallback' }] })
  assert.equal((state.snapshot()[0].args as any)[1].fallbackQueue.dismissed, true)
  state.forgetSessions()
  assert.deepEqual(state.snapshot(), [])
})

test('individual queue removal validates atomically, isolates tabs and retains delivery matching', () => {
  const events: any[] = []
  const state = new WorkspacePresentation((channel, ...args) => {
    events.push({ kind: 'event', channel, args })
    state.observe({ kind: 'event', channel, args })
  })
  const remove = (tab: string, ids: unknown) => state.accept({ kind: 'invoke', id: 1, generation: 'test',
    channel: 'workspace:queue-dismiss', args: [tab, ids] })
  const receipt = (tab: string, id: string) => (state.snapshot().find(frame => frame.channel === 'session:submission'
    && (frame.args as any)[0] === tab && (frame.args as any)[1].id === id)!.args as any)[1]
  for (const tab of ['tab', 'other']) for (const id of ['first', 'second']) {
    state.accept({ kind: 'send', channel: 'pty-submit-text', args: [tab, 'same queued text', id] })
    state.observe({ kind: 'event', channel: 'session:submission', args: [tab, { id, state: 'fallback' }] })
  }
  remove('tab', ['first', 'first', 'unknown'])
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, true)
  assert.equal(receipt('tab', 'first').state, 'fallback')
  assert.equal(receipt('other', 'first').fallbackQueue.dismissed, undefined)
  assert.equal(receipt('tab', 'second').fallbackQueue.dismissed, undefined)
  for (const ids of [null, 'second', ['second', 42], ['second', 'x'.repeat(1025)], Array(257).fill('second')]) {
    assert.throws(() => remove('tab', ids), /queue item IDs/)
    assert.equal(receipt('tab', 'second').fallbackQueue.dismissed, undefined)
  }
  const count = events.length
  remove('tab', ['first'])
  assert.equal(events.length, count, 'Repeated removal must not rebroadcast or perform another action')
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: 'first', state: 'fallback' }] })
  assert.equal(receipt('tab', 'first').fallbackQueue.dismissed, true)
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [{ type: 'user', uuid: 'first-row',
    _ord: 1, message: { content: 'same queued text' } }]] })
  assert.equal(receipt('tab', 'first').fallbackQueue.consumed, true)
  assert.equal(receipt('tab', 'second').fallbackQueue.consumed, undefined)
  assert.ok(events.every(frame => frame.channel === 'session:submission'), 'Display removal must only publish receipts')
})

test('receipts the server forgot settle from the transcript instead of hanging forever', () => {
  const t0 = Date.parse('2026-11-10T10:00:00Z')
  let entries: any[] = [{ type: 'user', uuid: 'old', _ord: 3, timestamp: new Date(t0 - 60_000).toISOString(),
    message: { content: 'keep the role as it is' } }]
  const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }), () => entries)
  const receipt = (id: string) => (state.snapshot().find(frame => frame.channel === 'session:submission' && (frame.args as any)[1].id === id)!.args as any)[1]
  for (const id of ['lost', 'kept']) {
    state.accept({ kind: 'send', channel: 'pty-submit-text', args: ['tab', 'keep the role as it is', id] })
    state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id, state: 'queued', createdAt: t0 }] })
  }
  assert.equal(receipt('lost').committed, undefined, 'An older identical prompt must not confirm a new receipt')
  // Server restarted: its reattach snapshot no longer lists 'lost'.
  state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', { id: '', known: ['kept'] }] })
  assert.equal(receipt('lost').state, 'uncertain')
  assert.equal(receipt('lost').cancelable, false)
  assert.match(receipt('lost').warning, /no longer tracks/)
  assert.equal(receipt('kept').state, 'queued')
  entries = [{ type: 'user', uuid: 'live', timestamp: new Date(t0 + 5_000).toISOString(), message: { content: 'keep the role as it is' } }]
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', entries] })
  assert.equal(receipt('lost').committed, true)
  assert.equal(receipt('kept').committed, undefined, 'One transcript row confirms one receipt')
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', entries] })
  assert.equal(receipt('kept').committed, undefined, 'A repeated batch cannot confirm identical prose twice')
})
