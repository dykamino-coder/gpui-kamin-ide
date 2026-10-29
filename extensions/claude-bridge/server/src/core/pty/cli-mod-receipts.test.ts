import { expect, it } from 'vitest'
import { register } from '../../cli-mod/hooks/bridge.js'

// Replay the public event shapes observed on CLI 2.1.312. In particular, abort
// drains native queued prompts through door:prompt without a second submit.
async function harness() {
  const hooks = new Map<string, (...args: any[]) => any>()
  register((event: string, hook: (...args: any[]) => any) => hooks.set(event, hook))
  let tick: () => Promise<void> = async () => {}
  let commands: unknown[] = []
  const reads: unknown[] = []
  const engine = {
    env: { get: async () => 'test' },
    session: { version: async () => ({ version: '2.1.312' }) },
    clock: {
      now: async () => 0,
      every: (_: number, callback: () => Promise<void>) => {
        tick = callback
      },
    },
    http: {
      fetch: async (_: string, options: { body: string }) => {
        reads.push(...JSON.parse(options.body).reads)
        const response = { commands }
        commands = []
        return { ok: true, text: JSON.stringify(response) }
      },
    },
  }
  const emit = (event: string, input: object, result: object = {}) =>
    hooks.get(event)!(engine, input, async () => result)
  await emit('session.start', { isInteractive: true, surface: 'terminal' })
  return {
    emit,
    reads,
    async arm(submissionId: string) {
      commands = [{ kind: 'arm', id: 1, submissionId }]
      await tick()
    },
    flush: () => tick(),
  }
}

it('binds identical native prompts by ID across mid-turn delivery and abort promotion', async () => {
  const h = await harness()
  for (const id of ['first', 'second', 'third']) {
    await h.arm(id)
    await h.emit('prompt.submit', { text: 'same text', turnId: 'running', origin: { kind: 'composer' } })
  }
  // Engine bookkeeping and subagent appends must not consume a receipt.
  await h.emit('session.append', { door: 'queue', origin: { kind: 'engine' }, message: { type: 'user' } })
  await h.emit('session.append', {
    agentId: 'subagent',
    door: 'prompt',
    origin: { kind: 'composer' },
    message: { type: 'user' },
  })
  await h.emit(
    'session.append',
    { door: 'delivery', origin: { kind: 'composer' }, message: { name: 'queued_command' } },
    { uuid: 'mid-turn' },
  )
  for (const uuid of ['after-abort-1', 'after-abort-2']) {
    await h.emit(
      'session.append',
      { door: 'prompt', origin: { kind: 'composer' }, message: { type: 'user' } },
      { uuid },
    )
  }
  await h.flush()
  expect(h.reads).toEqual([
    { submissionId: 'first', uuid: 'mid-turn' },
    { submissionId: 'second', uuid: 'after-abort-1' },
    { submissionId: 'third', uuid: 'after-abort-2' },
  ])
})

it('keeps unarmed composer and dropped prompts from shifting Bridge IDs', async () => {
  const h = await harness()
  await h.emit('prompt.submit', { origin: { kind: 'composer' } })
  await h.arm('dropped')
  await h.emit('prompt.submit', { origin: { kind: 'composer' } }, { drop: 'gate' })
  await h.arm('bridge')
  await h.emit('prompt.submit', { origin: { kind: 'composer' }, turnId: 'running' })
  for (const uuid of ['console-row', 'bridge-row']) {
    await h.emit(
      'session.append',
      { door: 'prompt', origin: { kind: 'composer' }, message: { type: 'user' } },
      { uuid },
    )
  }
  await h.flush()
  expect(h.reads).toEqual([{ submissionId: 'bridge', uuid: 'bridge-row' }])
})

it('reuses a queued slot when abort promotes it through another idle composer submit', async () => {
  const h = await harness()
  await h.arm('promoted')
  await h.emit('prompt.submit', { text: 'same text', origin: { kind: 'composer' }, turnId: 'running' })
  // CLI may run the submission gate again when starting the preserved queue.
  await h.emit('prompt.submit', { text: 'same text', origin: { kind: 'composer' } })
  await h.emit(
    'session.append',
    { door: 'prompt', origin: { kind: 'composer' }, message: { type: 'user' } },
    { uuid: 'promoted-row' },
  )
  // A later unarmed mid-turn console prompt still needs its own queue slot.
  await h.emit('prompt.submit', { text: 'same text', origin: { kind: 'composer' }, turnId: 'next' })
  await h.arm('next-bridge')
  await h.emit('prompt.submit', { text: 'same text', origin: { kind: 'composer' }, turnId: 'next' })
  for (const uuid of ['console-row', 'next-bridge-row']) {
    await h.emit(
      'session.append',
      { door: 'delivery', origin: { kind: 'composer' }, message: { name: 'queued_command' } },
      { uuid },
    )
  }
  await h.flush()
  expect(h.reads).toEqual([
    { submissionId: 'promoted', uuid: 'promoted-row' },
    { submissionId: 'next-bridge', uuid: 'next-bridge-row' },
  ])
})
