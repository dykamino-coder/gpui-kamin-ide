import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { STORAGE_REFUSED_WARNING, WorkspacePresentation } from '../src/presentation.js'
import { WorkspaceSubmissionState, type SubmissionStorage } from '../src/submission-state.js'

function fixture(t: any) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-submissions-'))
  t.after(() => {
    assert.equal(path.dirname(root), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(root).startsWith('bridgeweb-submissions-'))
    fs.rmSync(root, { recursive: true })
  })
  const storage = new WorkspaceSubmissionState(root, root, 'ws://fixture', 'synthetic-journal-token')
  const file = path.join(root, 'workspace-submissions.json')
  function host(tabIds?: string[]) {
    const state = new WorkspacePresentation((channel, ...args) => state.observe({ kind: 'event', channel, args }), () => [], storage)
    if (tabIds) state.restore(new Set(tabIds))
    return state
  }
  return { root, storage, file, host }
}
const send = (state: WorkspacePresentation, id: string, text = 'same text', tab = 'tab') =>
  state.accept({ kind: 'send', channel: 'pty-submit-text', args: [tab, text, id] })
const update = (state: WorkspacePresentation, receipt: any) => state.observe({ kind: 'event', channel: 'session:submission', args: ['tab', receipt] })
const rows = (state: WorkspacePresentation, uuid: string) => state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [
  { type: 'user', uuid, _ord: 1, message: { role: 'user', content: 'same text' } },
]] })
const receipt = (state: WorkspacePresentation, id: string): any =>
  (state.snapshot().find(frame => frame.channel === 'session:submission' && (frame.args as any)[1].id === id)?.args as any)?.[1]

test('durable identities recover uncertainty, retain exact metadata and never resend', t => {
  const { storage, file, host } = fixture(t)
  const before = host()
  for (const id of ['pending', 'committed', 'consumed', 'dismissed', 'cancelled']) send(before, id)
  update(before, { id: 'committed', state: 'read', uuid: 'native-uuid' })
  rows(before, 'native-uuid')
  update(before, { id: 'consumed', state: 'fallback' })
  rows(before, 'legacy-uuid')
  update(before, { id: 'dismissed', state: 'fallback' })
  before.accept({ kind: 'invoke', id: 1, generation: 'fixture', channel: 'workspace:queue-dismiss', args: ['tab', ['dismissed']] })
  update(before, { id: 'cancelled', state: 'cancelled' })
  send(before, 'other', 'other prompt', 'closed-tab')
  const disk = fs.readFileSync(file, 'utf8')
  assert.equal(disk.includes('synthetic-journal-token'), false)
  if (process.platform !== 'win32') assert.equal(fs.statSync(file).mode & 0o777, 0o600)
  const after = host(['tab'])
  assert.equal(receipt(after, 'pending').state, 'uncertain')
  assert.equal(receipt(after, 'pending').cancelable, false)
  assert.match(receipt(after, 'pending').warning, /restarted/)
  assert.equal(receipt(after, 'committed').committed, true)
  assert.equal(receipt(after, 'consumed').fallbackQueue.consumed, true)
  assert.equal(receipt(after, 'dismissed').fallbackQueue.dismissed, true)
  assert.equal(receipt(after, 'dismissed').state, 'uncertain')
  assert.equal(receipt(after, 'cancelled').state, 'cancelled')
  assert.equal(receipt(after, 'other'), undefined)
  assert.equal(send(after, 'pending'), false)
  assert.throws(() => send(after, 'pending', 'different text'), /identity/)
  send(after, 'new')
  update(after, { id: 'new', state: 'fallback' })
  rows(after, 'legacy-uuid')
  assert.equal(receipt(after, 'new').fallbackQueue.consumed, undefined, 'Consumed rows must not consume a later equal prompt after restart')
  update(after, { id: 'pending', uuid: 'late-uuid' })
  rows(after, 'late-uuid')
  assert.equal(receipt(after, 'pending').committed, true, 'Exact upstream UUID can resolve recovered uncertainty')
  after.observe({ kind: 'event', channel: 'tab:closed', args: ['tab'] })
  assert.deepEqual(storage.load(), { submissions: [], consumedRows: [] })
  send(after, 'disconnect')
  after.forgetSessions()
  assert.deepEqual(storage.load(), { submissions: [], consumedRows: [] })
})

test('scope changes isolate identities; corrupt or truncated state fails closed without credential output', t => {
  const { root, file, host, storage } = fixture(t)
  send(host(), 'identity')
  for (const [workspace, server, token] of [[root + '-other', 'ws://fixture', 'synthetic-journal-token'],
    [root, 'ws://other', 'synthetic-journal-token'], [root, 'ws://fixture', 'other-token']]) {
    assert.deepEqual(new WorkspaceSubmissionState(root, workspace!, server!, token!).load(), { submissions: [], consumedRows: [] })
  }
  const valid = fs.readFileSync(file, 'utf8')
  const cases = [valid.slice(0, -1), JSON.stringify({ ...JSON.parse(valid), version: 99 })]
  for (const change of [(s: any) => s.submissions[0].digest = 'incorrect',
    (s: any) => s.submissions[0].bytes++, (s: any) => s.submissions.push(s.submissions[0]),
    (s: any) => s.submissions[0].receipt.state = 'invented',
    (s: any) => s.submissions[0].receipt.fallbackQueue.afterOrd = -2,
    (s: any) => s.consumedRows = [['tab', ['uuid:a', 'uuid:a']]]]) {
    const state = JSON.parse(valid); change(state); cases.push(JSON.stringify(state))
  }
  for (const invalid of cases) {
    fs.writeFileSync(file, invalid)
    assert.throws(() => storage.load(), /^Error: Workspace submission state is unreadable; restore storage before starting$/)
    assert.equal(fs.readFileSync(file, 'utf8'), invalid, 'Recovery must not erase the evidence or reset duplicate protection')
  }
})

test('preforward storage failure refuses that send visibly; the next send retries the save', t => {
  const { storage, host } = fixture(t)
  let fail = true
  const published: any[] = [], restored: unknown[] = []
  const broken: SubmissionStorage = { load: () => storage.load(), save: value => { if (fail) throw new Error('private filesystem detail'); storage.save(value) } }
  const state: WorkspacePresentation = new WorkspacePresentation((channel, ...args) => {
    published.push(args[1]); state.observe({ kind: 'event', channel, args })
  }, () => [], broken, (tab, text) => restored.push([tab, text]))
  assert.equal(send(state, 'never-forwarded'), false, 'Without the duplicate barrier on disk nothing is dispatched')
  assert.deepEqual(published, [{ id: 'never-forwarded', text: 'same text', state: 'cancelled', rejected: true,
    cancelable: false, warning: STORAGE_REFUSED_WARNING }])
  assert.deepEqual(restored, [['tab', 'same text']], 'The refused text goes back to the input')
  assert.equal(receipt(state, 'never-forwarded'), undefined, 'A refused send keeps nothing in memory')
  assert.equal(send(state, 'never-forwarded-again'), false, 'Still failing storage still refuses')
  fail = false
  assert.equal(send(state, 'later'), true, 'Storage that works again accepts without a restart')
  assert.deepEqual(storage.load().submissions.map(item => item.receipt.id), ['later'])
  assert.equal(send(state, 'never-forwarded'), true, 'A refused identity was never used and may be sent again')
  const before = host()
  send(before, 'safe')
  const recovered = new WorkspacePresentation((channel, ...args) => recovered.observe({ kind: 'event', channel, args }), () => [], broken)
  recovered.restore(new Set(['tab']))
  fail = true
  assert.doesNotThrow(() => update(recovered, { id: 'safe', state: 'read', uuid: 'uuid-safe' }))
  assert.equal(send(recovered, 'refused'), false)
  fail = false
  assert.equal(send(recovered, 'accepted'), true, 'An observer save failure does not block later sends')
})

test('an actual failed atomic write leaves the last durable duplicate barrier intact', t => {
  const { storage, file, host } = fixture(t)
  const state = host()
  send(state, 'durable')
  const before = fs.readFileSync(file, 'utf8')
  fs.mkdirSync(`${file}.tmp`)
  assert.equal(send(state, 'not-forwarded'), false)
  assert.equal(fs.readFileSync(file, 'utf8'), before)
  assert.equal(storage.load().submissions.length, 1)
  assert.equal(send(host(['tab']), 'durable'), false)
  fs.rmdirSync(`${file}.tmp`)
  assert.equal(send(state, 'not-forwarded'), true)
  assert.equal(storage.load().submissions.length, 2)
})

test('REAL119: a receipt with the engine input (text plus hidden history) keeps the user text and its digest', t => {
  const { storage, host } = fixture(t)
  const block = '<bridge-context>\n<conversation-history>\nUser: earlier\n</conversation-history>\n</bridge-context>'
  const state = host()
  for (const [id, text] of [['to-codex', 'what did we decide?'], ['to-claude', 'and now? see report.pdf']] as const) {
    assert.equal(send(state, id, text), true)
    // Older servers (<= 6.3.196) echo the engine input in every receipt.
    update(state, { id, text: `${text}\n\n${block}`, state: 'read', uuid: `uuid-${id}` })
    assert.equal(receipt(state, id).text, text)
    assert.equal(receipt(state, id).uuid, `uuid-${id}`)
  }
  const saved = storage.load().submissions
  assert.deepEqual(saved.map(item => [item.receipt.id, item.receipt.text, item.receipt.state]),
    [['to-codex', 'what did we decide?', 'read'], ['to-claude', 'and now? see report.pdf', 'read']])
  assert.equal(send(state, 'next', 'next message'), true, 'The send after a handoff is accepted')
  assert.equal(send(state, 'next', 'next message'), false, 'and is dispatched exactly once')
  const restarted = host(['tab'])
  assert.equal(receipt(restarted, 'to-codex').text, 'what did we decide?')
  assert.equal(send(restarted, 'next', 'next message'), false, 'A restarted host still refuses the duplicate')
  assert.equal(send(restarted, 'after-restart', 'after restart'), true)
})

test('a short queued message is confirmed by the row that stores it with the hidden history', t => {
  const { host } = fixture(t)
  const state = host()
  send(state, 'short', 'short one')
  update(state, { id: 'short', state: 'fallback' })
  state.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [{ type: 'user', uuid: 'stored', _ord: 1,
    message: { role: 'user', content: 'short one\n\n<bridge-context>\n<conversation-history>\nUser: x\n</conversation-history>\n</bridge-context>' } }]] })
  assert.equal(receipt(state, 'short').fallbackQueue.consumed, true)
})

test('a receipt recovered as uncertain keeps server time and settles from a later transcript row', t => {
  const { host } = fixture(t)
  const t0 = Date.parse('2026-11-10T10:00:00Z')
  const before = host()
  send(before, 'pending')
  update(before, { id: 'pending', state: 'native', createdAt: t0 })
  const after = host(['tab'])
  assert.equal(receipt(after, 'pending').state, 'uncertain')
  assert.equal(receipt(after, 'pending').createdAt, t0)
  const row = (uuid: string, at: number) => after.observe({ kind: 'event', channel: 'jsonl-entries', args: ['tab', [
    { type: 'user', uuid, timestamp: new Date(at).toISOString(), message: { role: 'user', content: 'same text' } }]] })
  row('earlier', t0 - 60_000)
  assert.equal(receipt(after, 'pending').committed, undefined)
  row('answered', t0 + 1_000)
  assert.equal(receipt(after, 'pending').committed, true)
  assert.equal(receipt(host(['tab']), 'pending').committed, true, 'Settlement is durable')
})
