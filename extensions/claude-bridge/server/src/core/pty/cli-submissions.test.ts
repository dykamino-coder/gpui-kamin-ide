import { afterEach, describe, expect, it, vi } from 'vitest'
import { CliSubmissions } from './cli-submissions'
import type { CliModTransport } from './cli-mod-transport'
import type { PtySession } from './types'

const input = vi.hoisted(() => ({
  rawInputDirty: false,
  activeKind: null,
  queuedSubmissions: 0,
  pendingMaintenance: [],
}))
const deliver = vi.hoisted(() => vi.fn())
vi.mock('./session-input-coordinator', async (importOriginal) => ({
  ...(await importOriginal<typeof import('./session-input-coordinator')>()),
  getSessionInputSnapshot: () => input,
  submitCoordinatedText: deliver,
}))
vi.mock('./session-io', () => ({ sendToClient: (ws: any, frame: any) => ws.send(frame) }))

function fixture(busy = true) {
  vi.useFakeTimers()
  input.rawInputDirty = false
  deliver.mockClear()
  const frames: any[] = [],
    armed: (() => void)[] = []
  const mod = {
    healthy: () => true,
    submit: vi.fn(() => true),
    arm: vi.fn((_: string, accepted: () => void) => {
      armed.push(accepted)
      return true
    }),
  } as unknown as CliModTransport
  const session = { id: 's', state: 'running', ws: { send: (f: any) => frames.push(f) } } as unknown as PtySession
  const abort = vi.fn(() => true)
  const queue = new CliSubmissions(session, mod, () => busy, abort)
  return { queue, mod, frames, armed, abort }
}
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})

describe('hybrid message receipts', () => {
  it('serializes identical native messages and associates each by ID and stored UUID', () => {
    const f = fixture()
    for (let i = 1; i <= 3; i++) f.queue.submit('id-' + i, 'same text')
    expect(f.armed).toHaveLength(1)
    for (let i = 1; i <= 3; i++) {
      f.armed[i - 1]!()
      vi.advanceTimersByTime(100)
      f.mod.onRead('id-' + i, 'uuid-' + i)
    }
    expect(deliver).toHaveBeenCalledTimes(3)
    expect(f.frames.filter((f) => f.state === 'read').map((f) => [f.id, f.uuid])).toEqual([
      ['id-1', 'uuid-1'],
      ['id-2', 'uuid-2'],
      ['id-3', 'uuid-3'],
    ])
    f.queue.submit('id-1', 'same text')
    expect(deliver).toHaveBeenCalledTimes(3)
    expect(f.frames.at(-1).uuid).toBe('uuid-1')
  })
  it('preserves a dirty composer, permits cancellation, and never cancels after handoff', () => {
    const f = fixture()
    input.rawInputDirty = true
    f.queue.submit('cancel', 'keep draft')
    expect(f.frames.at(-1)).toMatchObject({ cancelable: true, warning: expect.stringContaining('draft') })
    f.queue.action('cancel', 'cancel')
    input.rawInputDirty = false
    vi.advanceTimersByTime(100)
    expect(f.armed).toHaveLength(0)
    f.queue.submit('native', 'send')
    f.armed[0]!()
    f.queue.action('native', 'cancel')
    expect(f.frames.at(-1)).toMatchObject({ state: 'native', cancelable: false })
    f.queue.action('native', 'interrupt')
    expect(f.abort).toHaveBeenCalledOnce()
    expect(f.mod.submit).not.toHaveBeenCalled()
  })
  it('uses submit while idle and replays receipts after reconnect', () => {
    const f = fixture(false)
    f.queue.submit('idle', 'Unicode 世界\nsecond line')
    expect(f.mod.submit).toHaveBeenCalledWith('Unicode 世界\nsecond line', expect.any(Function), 'idle')
    expect(deliver).not.toHaveBeenCalled()
    f.mod.onRead('idle', 'stored')
    f.frames.length = 0
    f.queue.replay()
    expect(f.frames).toEqual([expect.objectContaining({ id: 'idle', uuid: 'stored', state: 'read' })])
  })
  it('stops correlating after a manual queue edit rather than assigning the next receipt incorrectly', () => {
    const f = fixture()
    f.queue.submit('old', 'first')
    f.armed[0]!()
    f.queue.disableReceipts()
    f.mod.onRead('old', 'potentially-wrong-row')
    expect(f.frames.some((f) => f.state === 'read')).toBe(false)
    expect(f.queue.submit('next', 'second')).toBe(false)
    expect(f.frames.at(-1)).toMatchObject({ capable: false })
  })
  it('keeps the receipt handshake through automatic terminal replies', () => {
    const f = fixture()
    f.queue.submit('pending', 'text')
    f.queue.rawInput('\x1b[O\x1b[?1;2c')
    expect(f.queue.capable).toBe(true)
    f.armed[0]!()
    f.mod.onRead('pending', 'stored')
    expect(f.frames.at(-1)).toMatchObject({ state: 'read', uuid: 'stored' })
  })
})
