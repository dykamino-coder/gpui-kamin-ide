import { describe, expect, it, vi } from 'vitest'
import type { Terminal } from '@xterm/xterm'
import { configureTerminalClipboard } from './terminal-clipboard'

// Minimal document surface: exercises listener ownership and menu actions without
// emulating xterm's renderer (buffer/protocol coverage uses real headless xterm).
class Node {
  children: Node[] = []
  parent?: Node
  className = ''; style: Record<string, string> = {}; disabled = false
  onclick?: () => void
  listeners = new Map<string, EventListener>()
  append(...nodes: Node[]) { nodes.forEach(node => this.appendChild(node)) }
  appendChild(node: Node) { node.parent = this; this.children.push(node); return node }
  remove() { if (this.parent) this.parent.children = this.parent.children.filter(node => node !== this) }
  contains(node: Node): boolean { return this === node || this.children.some(child => child.contains(node)) }
  setAttribute() {}
  addEventListener(name: string, handler: EventListener) { this.listeners.set(name, handler) }
  removeEventListener(name: string, handler: EventListener) { if (this.listeners.get(name) === handler) this.listeners.delete(name) }
  getBoundingClientRect() { return { width: 150, height: 90 } }
}
function fixture(readText = vi.fn(async () => 'clipboard')) {
  const document = Object.assign(new Node(), {
    body: new Node(), createElement: () => new Node(), createTextNode: () => new Node(),
    defaultView: { innerWidth: 800, innerHeight: 600, navigator: {} },
  })
  const container = Object.assign(new Node(), { ownerDocument: document })
  let key!: (event: KeyboardEvent) => boolean
  let selection = ''
  const term = {
    getSelection: () => selection, hasSelection: () => !!selection,
    clearSelection: vi.fn(() => { selection = '' }), selectAll: vi.fn(), focus: vi.fn(), paste: vi.fn(),
    attachCustomKeyEventHandler: (handler: typeof key) => { key = handler },
  }
  const writeText = vi.fn()
  const dispose = configureTerminalClipboard(term as unknown as Terminal, container as unknown as HTMLElement, { writeText, readText })
  const emit = (node: Node, type: string, fields = {}) => {
    const event = { type, preventDefault: vi.fn(), stopPropagation: vi.fn(), stopImmediatePropagation: vi.fn(), ...fields }
    node.listeners.get(type)!(event as unknown as Event)
    return event
  }
  const context = () => emit(container, 'contextmenu', { clientX: 790, clientY: 590 })
  return { document, container, term, writeText, readText, dispose, emit, context,
    key: (fields = {}) => {
      const e = { type: 'keydown', code: 'KeyC', key: 'с', ctrlKey: true, shiftKey: false, altKey: false, metaKey: false, preventDefault: vi.fn(), stopPropagation: vi.fn(), ...fields }
      return { allowed: key(e as unknown as KeyboardEvent), event: e }
    },
    select: (text: string) => { selection = text },
  }
}

describe('console clipboard and context menu', () => {
  it.each(['c', 'с'])('copies native Ctrl+C in layout %s without forwarding interrupt', key => {
    const f = fixture()
    f.select('exact\r\n  selected text ')
    expect(f.key({ code: '', key, keyCode: 67 }).allowed).toBe(false)
    expect(f.writeText).toHaveBeenCalledExactlyOnceWith('exact\r\n  selected text ')
    expect(f.key({ code: '', key, keyCode: 67 }).allowed).toBe(true)
    f.dispose()
  })

  it('takes Copy from the terminal selection, preserving its exact bytes across menu focus changes', () => {
    const f = fixture()
    f.select('  terminal\r\nselection  ')
    f.context()
    const copy = f.document.body.children[0].children[0]
    expect(copy.disabled).toBe(false)
    f.select('')
    copy.onclick!()
    expect(f.writeText).toHaveBeenCalledExactlyOnceWith('  terminal\r\nselection  ')
    f.dispose()
  })

  it.each(['', 'text'])('enables Paste only after reading nonempty host clipboard (%j), without rereading on click', async text => {
    const read = vi.fn(async () => text)
    const f = fixture(read)
    f.context()
    const paste = f.document.body.children[0].children[1]
    expect(paste.disabled).toBe(true)
    await vi.waitFor(() => expect(paste.disabled).toBe(!text))
    expect(read).toHaveBeenCalledOnce()
    paste.onclick!()
    expect(f.term.paste.mock.calls).toEqual(text ? [[text]] : [])
    expect(read).toHaveBeenCalledOnce()
    f.dispose()
  })

  it('keeps Paste disabled on read failure and ignores a closed menu result', async () => {
    const f = fixture(vi.fn().mockRejectedValue(new Error('unavailable')))
    f.context()
    const paste = f.document.body.children[0].children[1]
    await Promise.resolve(); await Promise.resolve()
    expect(paste.disabled).toBe(true)
    f.dispose()
    let finish!: (text: string) => void
    const g = fixture(vi.fn(() => new Promise<string>(resolve => { finish = resolve })))
    g.context()
    const stale = g.document.body.children[0].children[1]
    g.dispose(); finish('stale')
    await Promise.resolve(); await Promise.resolve()
    expect(stale.disabled).toBe(true)
    expect(g.term.paste).not.toHaveBeenCalled()
  })
  it('uses the virtual key for CEF input without a scan code, across layouts', () => {
    const f = fixture()
    f.select('native CEF selection')
    expect(f.key({ code: '', key: 'с', keyCode: 67 }).allowed).toBe(false)
    expect(f.writeText).toHaveBeenCalledExactlyOnceWith('native CEF selection')
    expect(f.key({ code: '', key: 'с', keyCode: 67 }).allowed).toBe(true)
    expect(f.key({ code: '', key: 'м', keyCode: 86 }).allowed).toBe(false)
    f.select('unrelated shortcut')
    expect(f.key({ code: 'KeyA', keyCode: 67 }).allowed).toBe(true)
    expect(f.writeText).toHaveBeenCalledOnce()
    f.dispose()
  })

  it('copies Ctrl+C selection using the physical key, and otherwise leaves ^C to xterm', () => {
    const f = fixture()
    f.select('selected text')
    const copied = f.key()
    expect(copied.allowed).toBe(false)
    expect(copied.event.preventDefault).toHaveBeenCalledOnce()
    expect(f.writeText).toHaveBeenCalledWith('selected text')
    expect(f.term.clearSelection).toHaveBeenCalledOnce()
    expect(f.key().allowed).toBe(true)
    expect(f.key({ shiftKey: true }).allowed).toBe(false)
    f.dispose()
  })

  it('captures right-click and exposes Copy, Paste, Select All without clipping at the edge', async () => {
    const f = fixture()
    const e = f.context()
    expect(e.preventDefault).toHaveBeenCalledOnce()
    expect(e.stopImmediatePropagation).toHaveBeenCalledOnce()
    const menu = f.document.body.children[0]
    expect(menu.className).toBe('terminal-context-menu')
    expect(menu.children).toHaveLength(3)
    expect(menu.children[0].disabled).toBe(true)
    expect(menu.children[2].disabled).toBe(false)
    expect(menu.style).toEqual({ left: '650px', top: '510px' })
    menu.children[2].onclick!()
    expect(f.term.selectAll).toHaveBeenCalledOnce()
    expect(f.document.body.children).toHaveLength(0)
    f.select('menu selection'); f.context()
    f.document.body.children[0].children[0].onclick!()
    expect(f.writeText).toHaveBeenCalledWith('menu selection')
    f.context()
    const paste = f.document.body.children[0].children[1]
    await vi.waitFor(() => expect(paste.disabled).toBe(false))
    paste.onclick!()
    expect(f.term.paste).toHaveBeenCalledWith('clipboard')
    f.dispose()
  })

  it('sends native keyboard paste once, with no concurrent clipboard read', () => {
    const f = fixture()
    expect(f.key({ code: 'KeyV', key: 'v' }).allowed).toBe(false)
    expect(f.readText).not.toHaveBeenCalled()
    const event = f.emit(f.container, 'paste', { clipboardData: { getData: () => 'native paste' } })
    expect(event.stopImmediatePropagation).toHaveBeenCalledOnce()
    expect(f.term.paste).toHaveBeenCalledExactlyOnceWith('native paste')
    expect(f.readText).not.toHaveBeenCalled()
    f.dispose()
  })

  it('dismisses on Escape/outside click, replaces menus and removes listeners on unmount', () => {
    const f = fixture()
    f.context(); f.context()
    expect(f.document.body.children).toHaveLength(1)
    f.emit(f.document, 'keydown', { code: '', key: 'Escape' })
    expect(f.document.body.children).toHaveLength(0)
    expect(f.document.listeners.size).toBe(0)
    f.context(); f.emit(f.document, 'mousedown', { target: new Node() })
    expect(f.document.body.children).toHaveLength(0)
    f.context(); f.dispose()
    expect(f.document.body.children).toHaveLength(0)
    expect(f.document.listeners.size).toBe(0)
    expect(f.container.listeners.size).toBe(0)
  })

  it('does not paste an asynchronous clipboard result after unmount', async () => {
    const f = fixture()
    f.context(); f.document.body.children[0].children[1].onclick!()
    f.dispose()
    await Promise.resolve()
    expect(f.term.paste).not.toHaveBeenCalled()
  })
})
