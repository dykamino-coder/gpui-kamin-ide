import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import http from 'node:http'
import type { AddressInfo } from 'node:net'
import { CliModTransport } from './cli-mod-transport'
import { CliSubmissions } from './cli-submissions'
import { register } from '../../cli-mod/hooks/bridge.js'
import { SubmissionHandoff } from '../../../../extension/src/main/ws/submission-handoff'
import {
  clearSessionInputState,
  getSessionInputSnapshot,
  requestMaintenanceSubmission,
} from './session-input-coordinator'
import type { PtySession } from './types'

const cleanup: Array<() => void> = []
beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'] }))
afterEach(() => {
  cleanup.splice(0).forEach((dispose) => dispose())
  vi.clearAllTimers()
  vi.useRealTimers()
})

async function fixture(sessionId = 'recovery') {
  const transport = new CliModTransport(sessionId)
  cleanup.push(() => transport.dispose())
  // Production uses a Linux UDS; a loopback listener exercises the same HTTP
  // handler on Windows too, without changing the production transport API.
  const server = (transport as unknown as { server: http.Server }).server
  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const port = (server.address() as AddressInfo).port
  const poll = (extra: object = {}, route = '/poll') =>
    new Promise<any>((resolve, reject) => {
      const req = http.request({ hostname: '127.0.0.1', port, path: route, method: 'POST' }, (res) => {
        let body = ''
        res.on('data', (data) => {
          body += data
        })
        res.on('end', () => resolve(JSON.parse(body)))
      })
      req.on('error', reject)
      req.end(
        JSON.stringify({
          protocol: 1,
          session: sessionId,
          instance: 'cli',
          version: '2.1.322',
          turnId: 'long-turn',
          ...extra,
        }),
      )
    })
  const writes: string[] = [],
    frames: any[] = []
  let echo: ((data: string) => void) | undefined
  const session = {
    id: sessionId,
    state: 'running',
    detachedAt: null,
    lastActivityAt: new Date(),
    ws: { readyState: 1, bufferedAmount: 0, send: (frame: string) => frames.push(JSON.parse(frame)) },
    pty: {
      write: (data: string) => writes.push(data),
      onData: (cb: (data: string) => void) => {
        echo = cb
        return {
          dispose() {
            echo = undefined
          },
        }
      },
    },
  } as unknown as PtySession
  cleanup.push(() => clearSessionInputState(session))
  const queue = new CliSubmissions(
    session,
    transport,
    () => true,
    () => true,
  )
  const hooks = new Map<string, (...args: any[]) => any>()
  register((event: string, hook: (...args: any[]) => any) => hooks.set(event, hook))
  let tick: () => Promise<void> = async () => {}
  let loss: 'request' | 'response' | undefined
  const engine = {
    env: { get: async () => sessionId },
    session: { version: async () => ({ version: '2.1.322' }) },
    clock: {
      now: async () => 0,
      every: (_: number, cb: () => Promise<void>) => {
        tick = cb
      },
    },
    http: {
      fetch: async (url: string, options: { body: string }) => {
        const lose = loss
        loss = undefined
        if (lose === 'request') throw new Error('lost request')
        const response = await poll({ ...JSON.parse(options.body), instance: 'cli' }, new URL(url).pathname)
        if (lose === 'response') throw new Error('lost response')
        return { ok: true, text: JSON.stringify(response) }
      },
    },
  }
  await hooks.get('session.start')!(engine, { isInteractive: true, surface: 'terminal' }, async () => ({}))
  return {
    queue,
    transport,
    poll,
    writes,
    frames,
    session,
    tick: () => tick(),
    lose: (where: typeof loss) => {
      loss = where
    },
    finish: () => {
      vi.advanceTimersByTime(50)
      echo?.('echo')
      vi.advanceTimersByTime(160)
    },
  }
}

describe('submission delivery recovery through the real PTY coordinator', () => {
  it('rebinds during a lost arm ack and delivers both pending items exactly once in the new CLI', async () => {
    const old = await fixture('old'), next = await fixture('new')
    const rows: any[] = []
    const outbox = new SubmissionHandoff(update => rows.push(update))
    for (const [id, text] of [['one', 'first'], ['two', 'second']]) {
      outbox.remember(id!, text!)
      old.queue.submit(id!, text!)
    }
    await old.tick() // CLI claimed the arm; its acknowledgement has not arrived.
    old.frames.forEach(frame => outbox.receive(frame))
    const safe = old.queue.handoff()
    expect(safe).toEqual(['one', 'two'])
    outbox.end(safe, true)
    await old.tick() // A late acknowledgement cannot write into the retired PTY.
    old.finish()
    expect(old.writes).toEqual([])
    outbox.resume((text, id) => next.queue.submit(id, text))
    outbox.resume(() => { throw new Error('duplicate handoff') })
    for (let i = 0; i < 2; i++) {
      await next.tick(); await next.tick()
      next.finish()
      vi.advanceTimersByTime(100)
    }
    for (const id of safe) { next.queue.action(id, 'push'); next.queue.submit(id, 'duplicate frame') }
    next.finish()
    expect(next.writes).toEqual(['\x15', '\x1b[200~first\x1b[201~', '\r', '\x15', '\x1b[200~second\x1b[201~', '\r'])
    expect(rows.every(row => row.state === 'queued')).toBe(true)
  })

  it('never transfers native or claimed-submit outcomes automatically', async () => {
    const f = await fixture()
    f.queue.submit('native', 'already handed off')
    await f.tick(); await f.tick()
    expect(f.queue.handoff()).toEqual([])
    const idle = await fixture('idle')
    idle.transport.submit('not polled', () => {}, 'unclaimed')
    expect(idle.transport.withdrawSubmission('unclaimed')).toBe(true)
    idle.transport.submit('claimed', () => {}, 'claimed')
    await idle.poll()
    expect(idle.transport.withdrawSubmission('claimed')).toBe(false)
  })
  it('delivers during a long turn while maintenance is waiting for idle', async () => {
    const f = await fixture()
    requestMaintenanceSubmission(f.session, 'reload-skills', '/reload-skills')
    f.queue.submit('message', 'continue this turn')
    const { commands } = await f.poll()
    expect(commands).toEqual([{ id: 1, kind: 'arm', submissionId: 'message' }])
    await f.poll({ acks: [{ id: 1 }] })
    f.finish()
    expect(f.writes).toEqual(['\x15', '\x1b[200~continue this turn\x1b[201~', '\r'])
    expect(getSessionInputSnapshot(f.session)).toMatchObject({
      promptReady: false,
      pendingMaintenance: ['reload-skills'],
    })
  })

  it.each(['arm-response', 'ack-request', 'ack-response'] as const)(
    'recovers a lost %s and delivers once across replay and Send now',
    async (where) => {
      const f = await fixture()
      f.queue.submit('message', 'once')
      if (where === 'arm-response') f.lose('response')
      await f.tick()
      if (where === 'arm-response') {
        expect(f.writes).toEqual([])
        await f.tick() // The server retries the lost arm with its original identity.
      } else f.lose(where === 'ack-request' ? 'request' : 'response')
      await f.tick()
      if (where !== 'ack-response') expect(f.writes).toEqual(where === 'ack-request' ? [] : ['\x15'])
      f.queue.action('message', 'push')
      f.frames.length = 0
      const reconnected: any[] = []
      f.session.ws = {
        readyState: 1,
        bufferedAmount: 0,
        send: (frame: string) => reconnected.push(JSON.parse(frame)),
      } as unknown as PtySession['ws']
      f.queue.replay() // A newly attached client sees the authoritative receipt.
      expect(reconnected.at(-1)).toMatchObject({
        id: 'message',
        state: where === 'ack-request' ? 'sending' : 'native',
      })
      await f.tick() // Restored acknowledgements must not invoke the handoff twice.
      f.queue.replay()
      expect(reconnected.at(-1)).toMatchObject({ id: 'message', state: 'native' })
      f.queue.submit('message', 'once')
      f.queue.action('message', 'push')
      f.finish()
      vi.advanceTimersByTime(1000)
      expect(f.writes).toEqual(['\x15', '\x1b[200~once\x1b[201~', '\r'])
      expect((await f.poll()).commands).toEqual([])
    },
  )
})
