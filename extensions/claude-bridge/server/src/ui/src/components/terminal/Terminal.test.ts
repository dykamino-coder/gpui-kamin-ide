import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const h = vi.hoisted(() => {
  class Node {
    children: Node[] = []
    parent?: Node
    dataset = {}
    classList = { add: vi.fn(), remove: vi.fn() }
    style = {}
    className = ''
    disabled = false
    onclick?: () => void
    listeners = new Map<string, (event: any) => void>()
    append(...nodes: Node[]) {
      nodes.forEach((node) => this.appendChild(node))
    }
    appendChild(node: Node) {
      node.parent = this
      this.children.push(node)
    }
    remove() {
      if (this.parent) this.parent.children = this.parent.children.filter((child) => child !== this)
    }
    setAttribute() {}
    contains(node: Node): boolean {
      return node === this || this.children.some((child) => child.contains(node))
    }
    addEventListener(name: string, callback: (event: any) => void) {
      this.listeners.set(name, callback)
    }
    removeEventListener(name: string, callback: (event: any) => void) {
      if (this.listeners.get(name) === callback) this.listeners.delete(name)
    }
    getBoundingClientRect() {
      return { width: 150, height: 90 }
    }
  }
  const clipboard = { readText: vi.fn(async () => 'pasted'), writeText: vi.fn(async () => {}) }
  const document = Object.assign(new Node(), {
    body: new Node(),
    createElement: () => new Node(),
    createTextNode: () => new Node(),
    defaultView: { innerWidth: 800, innerHeight: 600, navigator: { clipboard, platform: 'Win32' } },
  })
  const container = Object.assign(new Node(), { ownerDocument: document })
  return {
    Node,
    document,
    container,
    clipboard,
    refs: 0,
    effects: [] as Array<() => void>,
    term: null as any,
    window: new Node(),
    ws: null as any,
    disconnect: vi.fn(),
    resize: null as null | (() => void),
    disposeSubscription: vi.fn(),
    options: null as any,
  }
})
vi.mock('preact/hooks', () => ({
  useRef: () => ({ current: h.refs++ === 0 ? h.container : null }),
  useState: (value: unknown) => [value, vi.fn()],
  useEffect: (fn: () => () => void) => {
    h.effects.push(fn())
  },
}))
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    cols = 80
    rows = 24
    selection = ''
    input?: (data: string) => void
    modes = { mouseTrackingMode: 'none' }
    buffer = { active: { type: 'normal', baseY: 0 }, onBufferChange: () => ({ dispose: h.disposeSubscription }) }
    key?: (event: KeyboardEvent) => boolean
    wheel?: (event: WheelEvent) => boolean
    constructor(options: unknown) {
      h.options = options
      h.term = this
    }
    loadAddon() {}
    open() {}
    focus() {}
    dispose = vi.fn()
    onData(callback: (data: string) => void) {
      this.input = callback
    }
    onWriteParsed() {
      return { dispose: h.disposeSubscription }
    }
    onScroll() {
      return { dispose: h.disposeSubscription }
    }
    onResize() {
      return { dispose: h.disposeSubscription }
    }
    attachCustomKeyEventHandler(callback: typeof this.key) {
      this.key = callback
    }
    attachCustomWheelEventHandler(callback: typeof this.wheel) {
      this.wheel = callback
    }
    getSelection() {
      return this.selection
    }
    hasSelection() {
      return !!this.selection
    }
    clearSelection() {
      this.selection = ''
    }
    selectAll = vi.fn()
    paste(text: string) {
      this.input?.(text)
    }
    reset() {}
    clear() {}
  },
}))
vi.mock('@xterm/addon-fit', () => ({
  FitAddon: class {
    fit() {}
  },
}))
import { Terminal } from './Terminal'

beforeEach(() => {
  vi.useFakeTimers()
  h.refs = 0
  h.term = null
  h.ws = null
  h.disposeSubscription.mockClear()
  h.disconnect.mockClear()
  h.clipboard.writeText.mockClear()
  vi.stubGlobal('window', Object.assign(h.window, { setTimeout }))
  vi.stubGlobal('location', { protocol: 'http:', host: 'localhost:9999' })
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(callback: () => void) {
        h.resize = callback
      }
      observe() {}
      disconnect = h.disconnect
    },
  )
  vi.stubGlobal(
    'WebSocket',
    class {
      static OPEN = 1
      readyState = 1
      send = vi.fn()
      onclose?: () => void
      constructor() {
        h.ws = this
      }
      close() {
        this.onclose?.()
      }
    },
  )
})
afterEach(() => {
  h.effects.splice(0).forEach((cleanup) => cleanup())
  vi.clearAllTimers()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})
const event = (fields = {}) => ({
  preventDefault: vi.fn(),
  stopImmediatePropagation: vi.fn(),
  stopPropagation: vi.fn(),
  ...fields,
})
async function mount() {
  Terminal()
  await vi.dynamicImportSettled()
}

describe('dashboard terminal shared console integration', () => {
  it.each(['c', '\u0441'])(
    'installs the bounded console, pastes once and copies native Ctrl+C in layout %s',
    async (key) => {
      await mount()
      expect(h.options).toMatchObject({ scrollback: 5000, overviewRuler: { width: 8 }, allowProposedApi: true })
      h.window.listeners.get('terminal:command')!({ detail: { command: 'echo synthetic' } })
      h.container.listeners.get('paste')!(event({ clipboardData: { getData: () => 'native' } }))
      expect(h.ws.send).toHaveBeenCalledExactlyOnceWith(JSON.stringify({ type: 'input', data: 'native' }))
      h.term.selection = 'selection'
      expect(h.term.key(event({ type: 'keydown', ctrlKey: true, code: '', key, keyCode: 67 }))).toBe(false)
      expect(h.clipboard.writeText).toHaveBeenCalledWith('selection')
      expect(h.term.key(event({ type: 'keydown', ctrlKey: true, code: '', key, keyCode: 67 }))).toBe(true)
    },
  )

  it('opens the same three-action menu and cleans it, subscriptions and resize timer on unmount', async () => {
    await mount()
    const secondary = event({ button: 2 })
    h.container.listeners.get('mousedown')!(secondary)
    expect(secondary.stopImmediatePropagation).toHaveBeenCalledOnce()
    h.container.listeners.get('contextmenu')!(event({ clientX: 10, clientY: 20 }))
    expect(h.document.body.children[0].className).toBe('terminal-context-menu')
    expect(h.document.body.children[0].children).toHaveLength(3)
    h.resize!()
    h.effects.splice(0).forEach((cleanup) => cleanup())
    expect(h.document.body.children).toHaveLength(0)
    expect(h.container.listeners.size).toBe(0)
    expect(h.document.listeners.size).toBe(0)
    expect(h.disposeSubscription).toHaveBeenCalledTimes(4)
    expect(h.disconnect).toHaveBeenCalledOnce()
    expect(h.term.dispose).toHaveBeenCalledOnce()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('does not initialize a console if unmounted during dynamic imports', async () => {
    Terminal()
    h.effects.splice(0).forEach((cleanup) => cleanup())
    await vi.dynamicImportSettled()
    expect(h.term).toBeNull()
    expect(h.container.listeners.size).toBe(0)
  })
})
