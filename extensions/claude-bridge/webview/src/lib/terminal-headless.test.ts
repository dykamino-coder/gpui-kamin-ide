import { afterEach, describe, expect, it } from 'vitest'
import { Terminal } from '@xterm/headless'
import { SerializeAddon } from '@xterm/addon-serialize'
import { CONSOLE_SCROLLBACK } from './terminal-interaction'

const terminals: Terminal[] = []
function create(): Terminal {
  const terminal = new Terminal({ cols: 80, rows: 24, scrollback: CONSOLE_SCROLLBACK, allowProposedApi: true })
  terminals.push(terminal)
  return terminal
}
function write(terminal: Terminal, data: string): Promise<void> {
  return new Promise(resolve => terminal.write(data, resolve))
}
afterEach(() => terminals.splice(0).forEach(t => t.dispose()))

describe('console normal and alternate buffers (real xterm 6)', () => {
  it('retains bounded normal history, reaches its ends and keeps viewport fitting after resize', async () => {
    const terminal = create()
    await write(terminal, Array.from({ length: CONSOLE_SCROLLBACK + 100 }, (_, i) => `line-${i}\r\n`).join(''))
    expect(terminal.buffer.normal.length).toBe(CONSOLE_SCROLLBACK + terminal.rows)
    expect(terminal.buffer.active.baseY).toBe(CONSOLE_SCROLLBACK)
    terminal.scrollToTop()
    expect(terminal.buffer.active.viewportY).toBe(0)
    expect(terminal.buffer.active.getLine(0)?.translateToString(true)).toBe('line-77')
    terminal.scrollToBottom()
    expect(terminal.buffer.active.viewportY).toBe(terminal.buffer.active.baseY)
    terminal.resize(60, 18)
    expect(terminal.buffer.active.length).toBeLessThanOrEqual(CONSOLE_SCROLLBACK + 18)
    expect(terminal.rows).toBe(18)
  })

  it('parses split DECSET/DECRST and preserves app mouse ownership without alternate history', async () => {
    const terminal = create()
    await write(terminal, 'normal-history\r\n\x1b[?1049h\x1b[?10')
    await write(terminal, '00;1006h')
    expect(terminal.buffer.active.type).toBe('alternate')
    expect(terminal.modes.mouseTrackingMode).toBe('vt200')
    await write(terminal, 'alternate\r\n'.repeat(200))
    expect(terminal.buffer.active.length).toBe(24)
    expect(terminal.buffer.active.baseY).toBe(0)
    await write(terminal, '\x1b[?1000;1006l\x1b[?1049l')
    expect(terminal.buffer.active.type).toBe('normal')
    expect(terminal.modes.mouseTrackingMode).toBe('none')
    expect(terminal.buffer.normal.getLine(0)?.translateToString(true)).toBe('normal-history')
  })

  it('encodes wheel events with the declared SGR protocol, rather than PageUp/PageDown', async () => {
    const terminal = create()
    await write(terminal, '\x1b[?1049h\x1b[?1000;1006h')
    const output: string[] = []
    terminal.onData(data => output.push(data))
    // The headless API has no DOM wheel. Exercise its actual protocol encoder
    // after the real parser enabled it; native CEF delivery is the lead's gate.
    const mouse = (terminal as unknown as {
      _core: { coreMouseService: { triggerMouseEvent(event: object): boolean } }
    })._core.coreMouseService
    for (const action of [0, 1]) {
      expect(mouse.triggerMouseEvent({ col: 3, row: 4, x: 30, y: 40, button: 4, action, ctrl: false, alt: false, shift: false })).toBe(true)
    }
    expect(output).toEqual(['\x1b[<64;4;5M', '\x1b[<65;4;5M'])
  })

  it('restores alternate screen and mouse modes from a real serialized snapshot', async () => {
    const source = create()
    const serialize = new SerializeAddon()
    source.loadAddon(serialize)
    await write(source, 'history\r\n\x1b[?1049h\x1b[H\x1b[?1002;1006happ-screen')
    const restored = create()
    await write(restored, serialize.serialize({ scrollback: 1000 }))
    expect(restored.buffer.active.type).toBe('alternate')
    expect(restored.modes.mouseTrackingMode).toBe('drag')
    expect(restored.buffer.active.getLine(0)?.translateToString(true)).toContain('app-screen')
    restored.reset()
    expect(restored.buffer.active.type).toBe('normal')
    expect(restored.modes.mouseTrackingMode).toBe('none')
  })
})
