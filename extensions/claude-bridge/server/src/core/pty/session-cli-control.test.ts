import { describe, expect, it, vi } from 'vitest'
import type { PtySession } from './types'
import type { CliModTransport } from './cli-mod-transport'
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
describe('interactive CLI control', () => {
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
})
