import { afterEach, expect, it, vi } from 'vitest'
import { register } from '../../cli-mod/hooks/bridge.js'
import { register as registerWithConditions } from '../../cli-mod/hooks/bridge-conditions.js'
import { CliSubmissions } from './cli-submissions'
import type { CliModTransport } from './cli-mod-transport'
import type { PtySession } from './types'

// Read receipts come from the stored rows of CLI 2.1.322 transcripts. In
// particular, abort drains native queued prompts as prompt rows without a
// second submit, and a mid-turn delivery is a queued_command attachment.
const input = vi.hoisted(() => ({
  rawInputDirty: false,
  activeKind: null,
  queuedSubmissions: 0,
  pendingMaintenance: [],
}))
vi.mock('./session-input-coordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./session-input-coordinator')>()),
  getSessionInputSnapshot: () => input,
  submitCoordinatedText: vi.fn(),
}))
vi.mock('./session-io', () => ({ sendToClient: (ws: any, frame: any) => ws.send(frame) }))

function fixture() {
  vi.useFakeTimers()
  const frames: any[] = []
  const mod = {
    healthy: () => true,
    submit: vi.fn(() => true),
    arm: vi.fn((_: string, accepted: () => void) => {
      accepted()
      return true
    }),
  } as unknown as CliModTransport
  const session = { id: 's', state: 'running', ws: { send: (f: any) => frames.push(f) } } as unknown as PtySession
  const queue = new CliSubmissions(
    session,
    mod,
    () => true,
    () => true,
  )
  return {
    queue,
    async send(id: string, text: string) {
      queue.submit(id, text)
      await vi.advanceTimersByTimeAsync(100)
    },
    reads: () => frames.filter((f) => f.state === 'read').map((f) => ({ submissionId: f.id, uuid: f.uuid })),
  }
}
const row = (uuid: string, fields: object) => ({ uuid, timestamp: new Date().toISOString(), ...fields }) as any
const user = (uuid: string, content: unknown, extra: object = {}) =>
  row(uuid, { type: 'user', message: { role: 'user', content }, ...extra })
const queued = (uuid: string, prompt: string) =>
  row(uuid, { type: 'attachment', attachment: { type: 'queued_command', prompt } })
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})

it('registers no handler on transcript appends or prompt submits: each costs every tool call', () => {
  for (const entry of [register, registerWithConditions]) {
    const events: string[] = []
    entry((event: string) => events.push(event))
    expect(events).not.toContain('session.append')
    expect(events).not.toContain('prompt.submit')
    expect(events).toEqual(expect.arrayContaining(['session.start', 'turn.start', 'turn.complete']))
  }
})

it('binds identical native prompts by ID across mid-turn delivery and abort promotion', async () => {
  const h = fixture()
  for (const id of ['first', 'second', 'third']) await h.send(id, 'same text')
  // Bookkeeping, sub-agent, meta and tool-result rows must not consume a receipt.
  h.queue.transcript([
    row('queue-op', { type: 'queue-operation', content: 'same text' }),
    user('subagent', 'same text', { isSidechain: true, agentId: 'a1' }),
    user('meta', 'same text', { isMeta: true }),
    user('tool', [{ type: 'tool_result', tool_use_id: 't', content: 'same text' }]),
    queued('mid-turn', 'same text'),
  ])
  h.queue.transcript([user('after-abort-1', 'same text'), user('after-abort-2', [{ type: 'text', text: 'same text' }])])
  expect(h.reads()).toEqual([
    { submissionId: 'first', uuid: 'mid-turn' },
    { submissionId: 'second', uuid: 'after-abort-1' },
    { submissionId: 'third', uuid: 'after-abort-2' },
  ])
})

it('keeps console prompts, re-read ranges and older rows from shifting Bridge IDs', async () => {
  const h = fixture()
  const before = user('before-handoff', 'from bridge')
  await vi.advanceTimersByTimeAsync(5000)
  await h.send('bridge', 'from bridge')
  h.queue.transcript([before, user('console-row', 'typed in the console')])
  expect(h.reads()).toEqual([])
  const stored = user('bridge-row', '  from\n bridge ')
  await h.send('next', 'from bridge')
  // The watcher repeats a range whose delivery failed; one row is one receipt.
  h.queue.transcript([stored])
  h.queue.transcript([stored, user('next-row', 'from bridge')])
  expect(h.reads()).toEqual([
    { submissionId: 'bridge', uuid: 'bridge-row' },
    { submissionId: 'next', uuid: 'next-row' },
  ])
})

it('matches a prompt the CLI stored before the hand-off returned, and none after receipts are disabled', async () => {
  const h = fixture()
  await h.send('early', 'stored quickly')
  h.queue.transcript([user('early-row', 'stored quickly')])
  await h.send('late', 'after a console queue edit')
  h.queue.disableReceipts()
  h.queue.transcript([user('late-row', 'after a console queue edit')])
  expect(h.reads()).toEqual([{ submissionId: 'early', uuid: 'early-row' }])
})
