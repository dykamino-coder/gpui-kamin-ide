import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { Terminal } from '@xterm/headless'
import type { Terminal as BrowserTerminal } from '@xterm/xterm'
import type { FitAddon } from '@xterm/addon-fit'
import type { KaminBridgeApi } from '../../shared/types'

const effects = vi.hoisted(() => [] as Array<() => void>)
vi.mock('preact/hooks', () => ({ useRef: (value: unknown) => ({ current: value }), useEffect: (fn: () => (() => void)) => effects.push(fn()) }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
import { useBridgeListeners } from './useBridgeListeners'
import { terminalRegistry } from '../signals/terminal-registry'

let terminal: Terminal
let listeners: Map<string, (...args: any[]) => void>
let frames: FrameRequestCallback[]
beforeEach(() => {
  frames = []
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => frames.push(callback))
  listeners = new Map()
  terminal = new Terminal({ allowProposedApi: true })
  terminalRegistry.set('a', {
    terminal: terminal as unknown as BrowserTerminal, fitAddon: {} as FitAddon,
    needsClear: false, pendingOutput: [], rafHandle: null,
  })
  const bridge = new Proxy({}, { get: (_, name: string) => {
    if (name.startsWith('on')) return (callback: (...args: any[]) => void) => { listeners.set(name, callback); return () => {} }
    return () => Promise.resolve(name === 'listTabs' ? [] : null)
  } }) as KaminBridgeApi
  useBridgeListeners(bridge, { current: new Map() }, { value: false }, 'chat', true)
})
afterEach(() => {
  for (const cleanup of effects.splice(0)) cleanup()
  terminalRegistry.clear()
  terminal.dispose()
  vi.unstubAllGlobals()
})
async function output(data: string): Promise<void> {
  listeners.get('onOutput')!('a', data)
  for (const frame of frames.splice(0)) frame(0)
  await new Promise<void>(resolve => terminal.write('', resolve))
}

it('routes split mouse modes through actual output batching and clears them for a new PTY', async () => {
  await output('\x1b[?1049h\x1b[?10')
  await output('02;1006happ')
  expect(terminal.buffer.active.type).toBe('alternate')
  expect(terminal.modes.mouseTrackingMode).toBe('drag')
  listeners.get('onConsoleReset')!('a')
  await output('new PTY')
  expect(terminal.buffer.active.type).toBe('normal')
  expect(terminal.modes.mouseTrackingMode).toBe('none')
  expect(terminal.buffer.active.getLine(0)?.translateToString(true)).toBe('new PTY')
})
