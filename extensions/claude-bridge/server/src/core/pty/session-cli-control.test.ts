import { describe, expect, it, vi } from 'vitest'
import type { PtySession } from './types'
import { CLI_MOD_VERSION, type CliModTransport } from './cli-mod-transport'
import { SessionCliControl } from './session-cli-control'
import { submitTextToSession, writeInputToSession } from './session-io'
import { applyDeterministicStatus } from '../hooks/bridge-status-hooks'

function fixture() {
  const frames: any[] = []
  const write = vi.fn()
  const session = {
    id: 'control',
    state: 'running',
    pty: { write },
    ws: { readyState: 1, bufferedAmount: 0, send: (s: string) => frames.push(JSON.parse(s)) },
  } as unknown as PtySession
  const mod = {
    healthy: () => true,
    submit: vi.fn(() => true),
    abort: vi.fn(() => true),
    onTurn: (_: string | null) => {},
    onUnavailable: () => {},
    onUncertain: () => {},
  } as unknown as CliModTransport
  return { frames, write, session, mod, control: new SessionCliControl(session, mod) }
}
const storedPrompt = (content: string, uuid: string) =>
  ({ type: 'user', uuid, timestamp: new Date().toISOString(), message: { role: 'user', content } }) as any

describe('interactive CLI control', () => {
  it('silently retains native delivery and OSC when the mod is unavailable', () => {
    vi.useFakeTimers()
    vi.stubEnv('BRIDGE_CLI_MOD', '1')
    try {
      const f = fixture()
      const control = new SessionCliControl(f.session)
      expect(control.submit('Привет')).toBe(false)
      control.parser.feed('\x1b]7501;state=idle:app=claude-code\x07')
      expect(f.frames.at(-1)).toMatchObject({ cliMod: false, promptReady: true })
      vi.advanceTimersByTime(16_000)
      expect(f.frames.some((frame) => frame.type === 'session:output')).toBe(false)
    } finally {
      vi.clearAllTimers()
      vi.useRealTimers()
      vi.unstubAllEnvs()
    }
  })
  it('keeps CLI diagnostics and status with the engine that owns the terminal', () => {
    vi.useFakeTimers()
    try {
      const old = fixture()
      const live = fixture()
      vi.spyOn(old.mod, 'healthy').mockReturnValue(false)
      vi.spyOn(live.mod, 'healthy').mockReturnValue(false)
      old.session.isRestarting = true
      old.mod.onUnavailable()
      old.mod.onUncertain()
      old.mod.onTurn('previous-turn')
      vi.advanceTimersByTime(15_000)
      expect(old.frames).toEqual([])
      expect(live.frames.filter((frame) => frame.type === 'session:output')).toEqual([
        expect.objectContaining({ data: expect.stringContaining(CLI_MOD_VERSION) }),
      ])
    } finally {
      vi.clearAllTimers()
      vi.useRealTimers()
    }
  })
  it('submits text including editor context without PTY input and aborts by turn ID', () => {
    const f = fixture()
    const text = 'hello\n```json editor-context\n{"openedFile":"/a.ts","textInSelectedRange":"世界"}\n```'
    submitTextToSession(f.session, text)
    expect(f.mod.submit).toHaveBeenCalledWith(text, expect.any(Function))
    expect(f.write).not.toHaveBeenCalled()
    f.mod.onTurn('turn-a')
    writeInputToSession(f.session, '\x03')
    expect(f.mod.abort).toHaveBeenCalledWith('turn-a', expect.any(Function))
    expect(f.frames.at(-1).isWorking).toBe(true)
    f.mod.onTurn(null)
    expect(f.frames.at(-1)).toMatchObject({ isWorking: false, promptReady: true })
  })
  it('leaves native slash commands, mentions, and attachment paths on PTY', () => {
    const f = fixture()
    for (const text of ['/model', 'read @file.ts', 'image\n/tmp/image.png', 'file\nC:\\tmp\\a.pdf']) {
      expect(f.control.submit(text)).toBe(false)
    }
    expect(f.mod.submit).not.toHaveBeenCalled()
  })
  it('lets OSC refine waiting and dominate hooks, while main mod lifecycle wins working', () => {
    const f = fixture()
    f.mod.onTurn('turn')
    f.control.parser.feed('\x1b]7501;state=blocked:app=claude-code:kind=question\x07')
    expect(f.frames.at(-1)).toMatchObject({ isWorking: true, waiting: true, promptReady: false })
    const count = f.frames.length
    applyDeterministicStatus(f.session, 'Stop', {} as any)
    expect(f.frames).toHaveLength(count)
    f.control.parser.feed('\x1b]7501;state=idle:app=claude-code\x07')
    expect(f.frames.at(-1).isWorking).toBe(true)
    f.mod.onTurn(null)
    expect(f.frames.at(-1).isWorking).toBe(false)
  })
  it('accepts two submissions once the paired main OSC turn ends, even if the mod still reports it', () => {
    vi.useFakeTimers()
    try {
      const f = fixture()
      f.control.parser.feed('\x1b]7501;state=working:app=claude-code\x07')
      f.mod.onTurn('finished-turn')
      f.control.parser.feed('\x1b]7501;state=done:app=claude-code:id=background-task\x07')
      expect(f.frames.at(-1)).toMatchObject({ isWorking: true, promptReady: false })
      f.control.parser.feed('\x1b]7501;state=done:app=claude-code\x07')
      f.mod.onTurn('finished-turn')
      f.control.replay()
      expect(f.frames.findLast((frame) => frame.type === 'session:activity')).toMatchObject({
        isWorking: false,
        promptReady: true,
      })

      expect(f.control.submit('first follow-up', 'first')).toBe(true)
      expect(f.control.submit('second follow-up', 'second')).toBe(true)
      f.control.transcript([storedPrompt('first follow-up', 'first-row')])
      vi.advanceTimersByTime(100)
      f.control.transcript([storedPrompt('second follow-up', 'second-row')])
      vi.advanceTimersByTime(100)
      expect(vi.mocked(f.mod.submit).mock.calls.map(([prompt, , id]) => [prompt, id])).toEqual([
        ['first follow-up', 'first'],
        ['second follow-up', 'second'],
      ])
      expect(f.write).not.toHaveBeenCalled()
      expect(f.mod.abort).not.toHaveBeenCalled()
    } finally {
      vi.clearAllTimers()
      vi.useRealTimers()
    }
  })
  it('does not use a delayed main OSC completion to mark a newer turn ready', () => {
    const f = fixture()
    f.mod.onTurn('old')
    f.control.parser.feed('\x1b]7501;state=working:app=claude-code\x07')
    f.mod.onTurn(null)
    f.mod.onTurn('new')
    f.control.parser.feed('\x1b]7501;state=done:app=claude-code\x07')
    expect(f.frames.at(-1)).toMatchObject({ isWorking: true, promptReady: false })
    f.control.parser.feed('\x1b]7501;state=working:app=claude-code\x07')
    f.control.parser.feed('\x1b]7501;state=done:app=claude-code\x07')
    expect(f.frames.at(-1)).toMatchObject({ isWorking: false, promptReady: true })
    f.control.parser.feed('\x1b]7501;state=blocked:app=claude-code:kind=permission\x07')
    expect(f.frames.at(-1)).toMatchObject({ waiting: true, promptReady: false })
    expect(f.control.submit('wait for permission')).toBe(false)
  })
  it('uses main OSC without a mod and replies outside the input coordinator', () => {
    const f = fixture()
    const control = new SessionCliControl(f.session)
    control.parser.feed('\x1b]7501;?\x07')
    expect(f.write).toHaveBeenCalledWith('\x1b]7501;?\x1b\\')
    control.parser.feed('\x1b]7501;state=working:app=claude-code\x07')
    expect(control.authoritative()).toBe(true)
    control.parser.feed('\x1b]7501;state=done:app=claude-code:id=agent\x07')
    expect(f.frames.at(-1).isWorking).toBe(true)
    control.parser.feed('\x1b]7501;state=done:app=claude-code\x07')
    expect(f.frames.at(-1).promptReady).toBe(true)
    control.parser.feed('\x1b]7501;state=clear\x07')
    expect(control.authoritative()).toBe(false)
  })
  it('tells a reattached client which receipts survive and answers a stopped session', () => {
    const f = fixture()
    expect(f.control.submit('kept', 'kept-id')).toBe(true)
    f.frames.length = 0
    f.control.replay()
    expect(f.frames.filter((frame) => frame.type === 'session:submission').at(-1)).toEqual({
      type: 'session:submission',
      sessionId: 'control',
      id: '',
      known: ['kept-id'],
    })
    f.frames.length = 0
    ;(f.session as { state: string }).state = 'exited'
    submitTextToSession(f.session, 'too late', 'late-id')
    expect(f.frames).toEqual([
      expect.objectContaining({
        type: 'session:submission',
        id: 'late-id',
        state: 'uncertain',
        cancelable: false,
        createdAt: expect.any(Number),
      }),
    ])
  })
  it('sends attachment paths and commands through the terminal without a delivery warning', () => {
    const f = fixture()
    for (const [id, text] of <[string, string][]>[
      ['att', 'what is in it?\n/tmp/bridge-upload-x/attach.txt'],
      ['win', 'and this?\nC:\\Users\\u\\AppData\\Local\\Temp\\bridge-upload-y\\attach.txt'],
      ['cmd', '/model'],
    ]) {
      f.frames.length = 0
      submitTextToSession(f.session, text, id)
      const receipt = f.frames.find((frame) => frame.type === 'session:submission')
      expect(receipt).toMatchObject({ id, state: 'fallback' })
      expect(receipt).not.toHaveProperty('warning')
    }
    expect(f.mod.submit).not.toHaveBeenCalled()
  })
  it('still warns when an ordinary message falls back to the terminal', () => {
    const f = fixture()
    ;(f.mod as { healthy: () => boolean }).healthy = () => false
    submitTextToSession(f.session, 'plain words', 'plain-id')
    expect(f.frames.find((frame) => frame.type === 'session:submission')).toMatchObject({
      id: 'plain-id',
      state: 'fallback',
      warning: 'Using native terminal delivery for this message.',
    })
  })
})
