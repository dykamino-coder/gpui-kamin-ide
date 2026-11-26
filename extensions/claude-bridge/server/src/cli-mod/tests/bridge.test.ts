import { expect, mock, test } from 'claude-code/testing'

test('no headless surface starts a transport', async ($, on) => {
  let calls = 0
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('http.fetch', () => {
    calls++
    return { value: { status: 200, ok: true, headers: {}, text: '{"commands":[]}' } }
  })
  await $.session.start({ cwd: '/', surface: null, isInteractive: false })
  expect(calls).toBe(0)
})

test('interactive transport queues submit without blocking polls or abort', async ($, on) => {
  const clock = mock.clock(on)
  mock.env(on, { BRIDGE_CLI_MOD_SOCKET: '/tmp/probe.sock', BRIDGE_CLI_MOD_SESSION: 'probe' })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.version', () => ({ value: { version: '2.1.328' } }))
  let polls = 0
  const submits: unknown[] = [],
    aborts: unknown[] = []
  on('http.fetch', (_, e) => {
    const commands = e.url.endsWith('/hello')
      ? []
      : ++polls === 1
        ? [{ id: 1, kind: 'submit', text: 'Привет\n世界' }]
        : polls === 2
          ? [{ id: 2, kind: 'abort', turnId: 'turn-a' }]
          : []
    return { value: { status: 200, ok: true, headers: {}, text: JSON.stringify({ commands }) } }
  })
  on('prompt.submit', async (_, e) => {
    submits.push(e)
    await clock.sleep(1000)
    return { turnId: 'turn-a' }
  })
  on('turn.abort', (_, e) => {
    aborts.push(e)
    return { value: undefined }
  })
  await $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true })
  await clock.advance(350)
  expect(submits).toHaveLength(1)
  expect(submits[0]).toMatchObject({ text: 'Привет\n世界', origin: { kind: 'plugin', asUser: true } })
  expect(aborts).toHaveLength(1)
  expect(polls).toBeGreaterThanOrEqual(2)
})

test('an idle transport long-polls and still reports acks at once', async ($, on) => {
  const clock = mock.clock(on)
  mock.env(on, { BRIDGE_CLI_MOD_SOCKET: '/tmp/probe.sock', BRIDGE_CLI_MOD_SESSION: 'probe' })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.version', () => ({ value: { version: '2.1.328' } }))
  let requests = 0
  let waits = 0
  const acks: unknown[] = []
  on('http.fetch', async (_, e) => {
    const body = JSON.parse(String(e.init?.body ?? '{}'))
    let commands: unknown[] = []
    if (!e.url.endsWith('/hello')) requests++
    if (body.acks?.length) acks.push(...body.acks)
    if (body.wait) {
      // The first held poll is answered by a command, later ones stay held.
      if (++waits === 1) commands = [{ id: 1, kind: 'submit', text: 'held' }]
      else await clock.sleep(2000)
    }
    return { value: { status: 200, ok: true, headers: {}, text: JSON.stringify({ commands }) } }
  })
  on('prompt.submit', (_, e) => ({ text: e.text }))
  await $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true })
  await clock.advance(5000)
  expect(waits).toBeGreaterThanOrEqual(2)
  expect(acks).toEqual([{ id: 1 }])
  // 100 ms polling would have made 50 requests in 5 s.
  expect(requests).toBeLessThan(20)
})
test('unsupported CLI version stays on the legacy path', async ($, on) => {
  mock.env(on, { BRIDGE_CLI_MOD_SOCKET: '/tmp/probe.sock', BRIDGE_CLI_MOD_SESSION: 'probe' })
  let calls = 0
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.version', () => ({ value: { version: '2.1.328' } }))
  on('http.fetch', () => {
    calls++
    return { deny: 'must not call' }
  })
  await $.session.start({ cwd: '/', surface: 'terminal', isInteractive: true })
  expect(calls).toBe(0)
})
