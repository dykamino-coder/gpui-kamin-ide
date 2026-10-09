import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
const fake = vi.hoisted(() => ({
  fail: false,
  processes: [] as Array<{
    data?: (text: string) => void
    exit?: () => void
    kill: ReturnType<typeof vi.fn>
    args: string[]
  }>,
}))
vi.mock('node-pty', () => ({
  spawn: (_file: string, args: string[]) => {
    if (fake.fail) throw new Error('synthetic-private-spawn-error')
    const proc = {
      data: undefined as ((text: string) => void) | undefined,
      exit: undefined as (() => void) | undefined,
      kill: vi.fn(),
      args,
    }
    fake.processes.push(proc)
    return {
      write: vi.fn(),
      kill: proc.kill,
      onData: (cb: (text: string) => void) => {
        proc.data = cb
        return { dispose: vi.fn() }
      },
      onExit: (cb: () => void) => {
        proc.exit = cb
        return { dispose: vi.fn() }
      },
    }
  },
}))
vi.mock('../../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn() }))
vi.mock('../../config/settings', () => ({ injectProxyEnv: vi.fn() }))

beforeEach(() => {
  vi.resetModules()
  fake.processes = []
  fake.fail = false
  vi.useFakeTimers()
  vi.setSystemTime(new Date('2026-10-09T12:00:00Z'))
})
afterEach(() => {
  vi.clearAllTimers()
  vi.useRealTimers()
})
const common = (session = 23, week = 42, model = 'Fable', percent = 17) =>
  `Current session\r\n${session}% used\r\nResets 6pm (UTC)\r\nCurrent week (all models)\r\n${week}% used\r\nResets Oct 12 at 6pm (UTC)\r\nPromotion: bonus usage until next month\r\nCurrent week (${model})\r\n${percent}% used\r\nResets Oct 12 at 6pm (UTC)\r\nExtra usage\r\nNot enabled\r\nEsc to cancel`
async function emit(index: number, text: string) {
  await vi.waitFor(() => expect(fake.processes[index]?.data).toBeTypeOf('function'))
  const proc = fake.processes[index]!
  proc.data!(text)
  proc.exit!()
  await vi.advanceTimersByTimeAsync(30)
}
describe('BR-20 actual capture contract', () => {
  it('includes all common and arbitrary model-specific windows despite promo sections', async () => {
    const { captureUsage } = await import('./usage-capture')
    const promise = captureUsage(true)
    await emit(0, common())
    const data = (await promise) as any
    expect(data.windows).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: 'five_hour', percent: 23 }),
        expect.objectContaining({ id: 'seven_day', percent: 42 }),
        expect.objectContaining({ model: 'Fable', percent: 17 }),
      ]),
    )
    expect(data.observedAt).toBeTypeOf('string')
    expect(data.source).toBe('tui-compatibility')
  })
  it('parses final terminal state instead of the first concatenated redraw frame', async () => {
    const { captureUsage } = await import('./usage-capture')
    const promise = captureUsage(true)
    await emit(0, common(12, 31, 'Sonnet only') + '\x1b[2J\x1b[H' + common(26, 48))
    const data = (await promise) as any
    expect(data.session?.percent).toBe(26)
    expect(data.windows).toEqual(expect.arrayContaining([expect.objectContaining({ id: 'seven_day', percent: 48 })]))
  })
  it('joins overlapping forced refreshes instead of returning an older cached snapshot as fresh', async () => {
    const { captureUsage } = await import('./usage-capture')
    const first = captureUsage(true)
    await emit(0, common(23))
    await first
    const second = captureUsage(true)
    await vi.waitFor(() => expect(fake.processes[1]).toBeDefined())
    const joined = captureUsage(true)
    await emit(1, common(29))
    expect((await second).session?.percent).toBe(29)
    expect((await joined).session?.percent).toBe(29)
    expect(fake.processes).toHaveLength(2)
  })
  it('retains missing rows as stale on partial/error refresh and isolates cached data from caller mutation', async () => {
    const { captureUsage } = await import('./usage-capture')
    const first = captureUsage(true)
    await emit(0, common())
    const previous = await first
    previous.windows[0]!.percent = 99
    expect((await captureUsage()).session?.percent).toBe(23)
    const partial = captureUsage(true)
    await emit(1, 'Current session\r\n29% used\r\nResets 6pm (UTC)')
    expect((await partial).windows.find((window) => window.model === 'Fable')).toMatchObject({
      percent: 17,
      freshness: 'stale',
    })
    fake.fail = true
    const unavailable = await captureUsage(true)
    expect(unavailable).toMatchObject({ state: 'unavailable', reason: 'capture-error' })
    expect(unavailable.windows.every((window) => window.freshness === 'stale')).toBe(true)
    expect(JSON.stringify(unavailable)).not.toContain('private-spawn-error')
  })
  it('bounds PTY output and kills only the disposable capture once', async () => {
    const { captureUsage } = await import('./usage-capture')
    const promise = captureUsage(true)
    await emit(0, 'x'.repeat(1024 * 1024 + 1))
    expect(await promise).toMatchObject({
      state: 'unavailable',
      reason: 'output-limit',
      diagnostics: { outputLimited: true },
    })
    expect(fake.processes[0]!.kill).toHaveBeenCalledTimes(1)
  })
  it('finishes a non-exiting capture on its render deadline without retaining kill timers', async () => {
    const { captureUsage } = await import('./usage-capture')
    const promise = captureUsage(true)
    await vi.waitFor(() => expect(fake.processes[0]?.data).toBeTypeOf('function'))
    fake.processes[0]!.data!(common())
    await vi.advanceTimersByTimeAsync(8100)
    expect((await promise).session?.percent).toBe(23)
    expect(fake.processes[0]!.kill).toHaveBeenCalledTimes(1)
    expect(vi.getTimerCount()).toBe(0)
  })
})
