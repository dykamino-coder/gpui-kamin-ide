import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Terminal } from '@xterm/xterm'
import { configureTerminalInteraction, CONSOLE_OVERVIEW_RULER } from '../../../server/src/ui/src/lib/terminal-console/interaction'

function fixture(mode = 'none', screen = 'normal', platform = 'Win32') {
  const callbacks: Record<string, () => void> = {}
  const disposals = Array.from({ length: 4 }, () => ({ dispose: vi.fn() }))
  const container = {
    dataset: {} as Record<string, string>, classList: { add: vi.fn(), remove: vi.fn() },
    ownerDocument: { defaultView: { navigator: { platform } }, addEventListener: vi.fn(), removeEventListener: vi.fn() },
    addEventListener: vi.fn(), removeEventListener: vi.fn(),
  }
  const subscribe = (name: string, index: number) => (callback: () => void) => {
    callbacks[name] = callback
    return disposals[index]
  }
  const term = {
    modes: { mouseTrackingMode: mode },
    buffer: { active: { type: screen, baseY: 0 }, onBufferChange: subscribe('buffer', 0) },
    onWriteParsed: subscribe('write', 1), onScroll: subscribe('scroll', 2), onResize: subscribe('resize', 3),
    hasSelection: vi.fn(() => false), focus: vi.fn(), attachCustomWheelEventHandler: vi.fn(),
  }
  const dispose = configureTerminalInteraction(term as unknown as Terminal, container as unknown as HTMLElement)
  return {
    term, container, dispose, callbacks, disposals,
    mouse: container.addEventListener.mock.calls[0][1] as (e: MouseEvent) => void,
    mouseUp: container.addEventListener.mock.calls[1][1] as (e: MouseEvent) => void,
    mouseMove: container.addEventListener.mock.calls[2][1] as (e: MouseEvent) => void,
    documentMove: container.ownerDocument.addEventListener.mock.calls[0][1] as (e: MouseEvent) => void,
    documentUp: container.ownerDocument.addEventListener.mock.calls[1][1] as (e: MouseEvent) => void,
    wheel: term.attachCustomWheelEventHandler.mock.calls[0][0] as (e: WheelEvent) => boolean,
  }
}
function mouseEvent(shiftKey: boolean, onScreen = true, button = 0) {
  return {
    button, buttons: 1, shiftKey, detail: 1, clientX: 12, clientY: 34,
    preventDefault: vi.fn(), stopImmediatePropagation: vi.fn(),
    target: { isConnected: true, closest: vi.fn(() => onScreen ? {} : null), dispatchEvent: vi.fn() },
  }
}
afterEach(() => vi.unstubAllGlobals())

describe('console mouse ownership', () => {
  it.each(['normal', 'alternate'])('preserves %s selection on hover without changing application modes or drag/wheel routing', screen => {
    const f = fixture('any', screen)
    f.term.hasSelection.mockReturnValue(true)
    const hover = { buttons: 0, stopImmediatePropagation: vi.fn() }
    f.mouseMove(hover as unknown as MouseEvent)
    expect(hover.stopImmediatePropagation).toHaveBeenCalledOnce()
    const drag = { buttons: 1, stopImmediatePropagation: vi.fn() }
    f.mouseMove(drag as unknown as MouseEvent)
    expect(drag.stopImmediatePropagation).not.toHaveBeenCalled()
    expect(f.wheel({} as WheelEvent)).toBe(true)
    f.term.hasSelection.mockReturnValue(false)
    hover.stopImmediatePropagation.mockClear()
    f.mouseMove(hover as unknown as MouseEvent)
    expect(hover.stopImmediatePropagation).not.toHaveBeenCalled()
    expect(f.term.modes.mouseTrackingMode).toBe('any')
    f.dispose()
    expect(f.container.removeEventListener).toHaveBeenCalledWith('mousemove', f.mouseMove, true)
  })
  it('allows normal-screen selection without Shift and leaves native Shift extension intact', () => {
    const f = fixture()
    for (const shift of [false, true]) {
      const e = mouseEvent(shift)
      f.mouse(e as unknown as MouseEvent)
      expect(e.stopImmediatePropagation).not.toHaveBeenCalled()
      expect(e.target.dispatchEvent).not.toHaveBeenCalled()
    }
  })

  it.each(['normal', 'alternate'])('reserves secondary down/up for our menu in %s, even with reporting', screen => {
    const f = fixture('any', screen)
    for (const handler of [f.mouse, f.mouseUp]) {
      const e = mouseEvent(false, true, 2)
      handler(e as unknown as MouseEvent)
      expect(e.preventDefault).toHaveBeenCalledOnce()
      expect(e.stopImmediatePropagation).toHaveBeenCalledOnce()
    }
  })

  it('allows ordinary selection in a non-reporting alternate screen', () => {
    const f = fixture('none', 'alternate')
    const ordinary = mouseEvent(false)
    f.mouse(ordinary as unknown as MouseEvent)
    expect(ordinary.stopImmediatePropagation).not.toHaveBeenCalled()
    expect(ordinary.target.dispatchEvent).not.toHaveBeenCalled()
  })

  it.each(['Win32', 'MacIntel'])('forces normal-screen selection on %s without resetting application modes', platform => {
    vi.stubGlobal('MouseEvent', class {
      constructor(public type: string, init: MouseEventInit) { Object.assign(this, init) }
    })
    const f = fixture('vt200', 'normal', platform)
    const e = mouseEvent(false)
    f.mouse(e as unknown as MouseEvent)
    expect(e.target.dispatchEvent.mock.calls[0][0]).toMatchObject({ shiftKey: platform === 'Win32', altKey: platform === 'MacIntel' })
    expect(f.term.modes.mouseTrackingMode).toBe('vt200')
  })

  it.each(['x10', 'vt200', 'drag', 'any'])('selects before %s can receive a drag, including release outside the console', mode => {
    vi.stubGlobal('MouseEvent', class {
      constructor(public type: string, init: MouseEventInit) { Object.assign(this, init) }
    })
    for (const screen of ['normal', 'alternate']) {
      const f = fixture(mode, screen)
      const e = mouseEvent(false)
      f.mouse(e as unknown as MouseEvent)
      expect(e.stopImmediatePropagation).toHaveBeenCalledOnce()
      expect(e.target.dispatchEvent.mock.calls[0][0]).toMatchObject({ type: 'mousedown', shiftKey: true, clientX: 12, clientY: 34 })
      f.documentMove({ clientX: 90, clientY: 34 } as MouseEvent)
      f.documentUp({ button: 0 } as MouseEvent)
      expect(e.target.dispatchEvent).toHaveBeenCalledOnce()
      expect(f.term.modes.mouseTrackingMode).toBe(mode)
      f.dispose()
      expect(f.container.ownerDocument.removeEventListener).toHaveBeenCalledTimes(2)
    }
  })

  it('replays a short click to the application, but never replays a double click', () => {
    vi.stubGlobal('MouseEvent', class {
      constructor(public type: string, init: MouseEventInit) { Object.assign(this, init) }
    })
    const f = fixture('any', 'alternate')
    const e = mouseEvent(false)
    f.mouse(e as unknown as MouseEvent)
    f.documentUp({ button: 0, buttons: 0, clientX: 12, clientY: 34 } as MouseEvent)
    expect(e.target.dispatchEvent.mock.calls.map(([event]) => [event.type, event.shiftKey])).toEqual([
      ['mousedown', true], ['mousedown', false], ['mouseup', false],
    ])
    e.target.dispatchEvent.mockClear()
    e.detail = 2
    f.mouse(e as unknown as MouseEvent)
    f.documentUp({ button: 0 } as MouseEvent)
    expect(e.target.dispatchEvent).toHaveBeenCalledOnce()
  })

  it.each(['Win32', 'MacIntel'])('routes Alt gestures to the application on %s without the routing modifier', platform => {
    vi.stubGlobal('MouseEvent', class {
      constructor(public type: string, init: MouseEventInit) { Object.assign(this, init) }
    })
    const f = fixture('drag', 'alternate', platform)
    const e = { ...mouseEvent(false), altKey: true }
    f.mouse(e as unknown as MouseEvent)
    const move = { ...mouseEvent(false), altKey: true, clientX: 50 }
    f.documentMove(move as unknown as MouseEvent)
    f.documentUp({ ...move, buttons: 0 } as unknown as MouseEvent)
    expect(e.target.dispatchEvent.mock.calls.map(([event]) => [event.type, event.altKey, event.shiftKey])).toEqual([
      ['mousedown', false, false], ['mousemove', false, false], ['mouseup', false, false],
    ])
  })

  it('never intercepts scrollbar drags', () => {
    const f = fixture()
    const e = mouseEvent(false, false)
    f.mouse(e as unknown as MouseEvent)
    expect(e.preventDefault).not.toHaveBeenCalled()
  })

  it('tracks screen/history on parsed writes, switches, scrolling, resizing, and cleans up', () => {
    const f = fixture()
    expect(f.container.dataset).toEqual({ terminalScreen: 'normal', terminalHistory: 'false' })
    f.term.buffer.active.baseY = 20
    f.callbacks.write()
    expect(f.container.dataset.terminalHistory).toBe('true')
    f.term.buffer.active = { type: 'alternate', baseY: 0 }
    f.callbacks.buffer()
    expect(f.container.dataset).toEqual({ terminalScreen: 'alternate', terminalHistory: 'false' })
    f.term.buffer.active = { type: 'normal', baseY: 20 }
    f.callbacks.scroll()
    expect(f.container.dataset.terminalHistory).toBe('true')
    f.term.buffer.active.baseY = 0
    f.callbacks.resize()
    expect(f.container.dataset.terminalHistory).toBe('false')
    f.dispose()
    for (const d of f.disposals) expect(d.dispose).toHaveBeenCalledOnce()
    expect(f.container.dataset).toEqual({})
    expect(f.container.removeEventListener).toHaveBeenCalledWith('mousedown', f.mouse, true)
  })

  it.each(['vt200', 'drag', 'any'])('lets xterm encode alternate wheel input for %s', mode => {
    const f = fixture(mode, 'alternate')
    expect(f.wheel({} as WheelEvent)).toBe(true)
  })

  it.each(['none', 'x10'])('does not synthesize keys in alternate mode %s', mode => {
    const f = fixture(mode, 'alternate')
    const e = { preventDefault: vi.fn(), stopPropagation: vi.fn() }
    expect(f.wheel(e as unknown as WheelEvent)).toBe(false)
    f.term.buffer.active.type = 'normal'
    expect(f.wheel(e as unknown as WheelEvent)).toBe(true)
  })

  it('allocates one narrow scrollbar through the shared public xterm/FitAddon width', () => {
    expect(CONSOLE_OVERVIEW_RULER.width).toBe(8)
  })
})
