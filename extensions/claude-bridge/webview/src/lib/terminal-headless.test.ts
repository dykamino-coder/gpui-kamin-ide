import { afterEach, describe, expect, it, vi } from 'vitest'
import { Terminal } from '@xterm/headless'
import { SerializeAddon } from '@xterm/addon-serialize'
import { CONSOLE_SCROLLBACK, configureTerminalInteraction } from '../../../server/src/ui/src/lib/terminal-console/interaction'
import type { Terminal as BrowserTerminal } from '@xterm/xterm'
import { createOsc52ClipboardProvider } from '../../../server/src/ui/src/lib/terminal-console/clipboard'

const terminals: Terminal[] = []
function create(): Terminal {
  const terminal = new Terminal({ cols: 80, rows: 24, scrollback: CONSOLE_SCROLLBACK, allowProposedApi: true })
  terminals.push(terminal)
  return terminal
}
function write(terminal: Terminal, data: string): Promise<void> {
  return new Promise(resolve => terminal.write(data, resolve))
}
afterEach(() => { terminals.splice(0).forEach(t => t.dispose()); vi.unstubAllGlobals() })

describe('console normal and alternate buffers (real xterm 6)', () => {
  it('decodes OSC 52 with the real clipboard addon into the native provider without exposing clipboard reads', async () => {
    vi.stubGlobal('self', globalThis)
    const { ClipboardAddon, Base64 } = await import('@xterm/addon-clipboard')
    const base64 = new Base64()
    const terminal = create()
    const writeText = vi.fn()
    const readText = vi.fn(async () => 'private clipboard')
    terminal.loadAddon(new ClipboardAddon(undefined, createOsc52ClipboardProvider({} as Document, { writeText, readText })) as never)
    const text = '  CLI копирует\nexact text  '
    await write(terminal, `\x1b]52;c;${base64.encodeText(text)}\x07`)
    expect(writeText).toHaveBeenCalledExactlyOnceWith(text)
    await write(terminal, `\x1b]52;;${base64.encodeText('default clipboard')}\x1b\\`)
    expect(writeText).toHaveBeenLastCalledWith('default clipboard')
    await write(terminal, '\x1b]52;c;?\x07')
    expect(readText).not.toHaveBeenCalled()
    await write(terminal, '\x1b]52;p;aWdub3JlZA==\x07')
    expect(writeText).toHaveBeenCalledTimes(2)
  })
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

  it('updates scrollbar visibility from parsed buffer switches, history clearing and reset', async () => {
    const terminal = create()
    const container = {
      dataset: {} as Record<string, string>, classList: { add() {}, remove() {} },
      addEventListener() {}, removeEventListener() {},
      ownerDocument: { addEventListener() {}, removeEventListener() {} },
    }
    const browser = Object.assign(terminal, { attachCustomWheelEventHandler() {} })
    const dispose = configureTerminalInteraction(browser as unknown as BrowserTerminal, container as unknown as HTMLElement)
    expect(container.dataset).toEqual({ terminalScreen: 'normal', terminalHistory: 'false' })
    await write(terminal, 'history\r\n'.repeat(100))
    expect(container.dataset.terminalHistory).toBe('true')
    terminal.scrollToTop()
    expect(terminal.buffer.active.viewportY).toBe(0)
    await write(terminal, '\x1b[?1049h\x1b[?1000;1006h')
    expect(container.dataset).toEqual({ terminalScreen: 'alternate', terminalHistory: 'false' })
    expect(terminal.modes.mouseTrackingMode).toBe('vt200')
    await write(terminal, '\x1b[?1049l')
    expect(container.dataset).toEqual({ terminalScreen: 'normal', terminalHistory: 'false' })
    await write(terminal, '\x1b[?1000l')
    expect(container.dataset).toEqual({ terminalScreen: 'normal', terminalHistory: 'true' })
    await write(terminal, '\x1b[3J')
    expect(container.dataset.terminalHistory).toBe('false')
    await write(terminal, '\x1b[?1049h')
    terminal.reset()
    expect(container.dataset.terminalScreen).toBe('normal')
    dispose()
    expect(container.dataset).toEqual({})
  })

  it('hides stale normal-buffer history while the application owns wheel reports and restores it afterwards', async () => {
    const terminal = create()
    const container = {
      dataset: {} as Record<string, string>, classList: { add() {}, remove() {} },
      addEventListener() {}, removeEventListener() {},
      ownerDocument: { addEventListener() {}, removeEventListener() {} },
    }
    const dispose = configureTerminalInteraction(Object.assign(terminal, { attachCustomWheelEventHandler() {} }) as unknown as BrowserTerminal, container as unknown as HTMLElement)
    await write(terminal, 'retained history\r\n'.repeat(80))
    const history = terminal.buffer.active.baseY
    expect(history).toBeGreaterThan(0)
    for (const mode of [1000, 1002, 1003]) {
      await write(terminal, `\x1b[?${mode}h\x1b[Hrepainted CLI`)
      expect(container.dataset.terminalHistory).toBe('false')
      expect(terminal.buffer.active.baseY).toBe(history)
      await write(terminal, `\x1b[?${mode}l`)
      expect(container.dataset.terminalHistory).toBe('true')
    }
    await write(terminal, '\x1b[?9h')
    expect(container.dataset.terminalHistory).toBe('true')
    dispose()
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
