import { afterEach, describe, expect, it, vi } from 'vitest'
import type { Terminal } from '@xterm/xterm'
import { configureTerminalInteraction } from './terminal-interaction'

function fixture(mode = 'none', screen = 'normal') {
  const container = { addEventListener: vi.fn(), removeEventListener: vi.fn() }
  const term = {
    modes: { mouseTrackingMode: mode }, buffer: { active: { type: screen } },
    focus: vi.fn(), attachCustomWheelEventHandler: vi.fn(),
  }
  const dispose = configureTerminalInteraction(term as unknown as Terminal, container as unknown as HTMLElement)
  return {
    term, container, dispose,
    mouse: container.addEventListener.mock.calls[0][1] as (e: MouseEvent) => void,
    wheel: term.attachCustomWheelEventHandler.mock.calls[0][0] as (e: WheelEvent) => boolean,
  }
}

function mouseEvent(shiftKey: boolean, onScreen = true, button = 0) {
  return {
    button, buttons: 1, shiftKey, detail: 1, clientX: 12, clientY: 34,
    preventDefault: vi.fn(), stopImmediatePropagation: vi.fn(),
    target: { closest: vi.fn(() => onScreen ? {} : null), dispatchEvent: vi.fn() },
  }
}

afterEach(() => vi.unstubAllGlobals())

describe('console mouse ownership', () => {
  it('focuses a non-reporting terminal without starting unmodified selection', () => {
    const f = fixture()
    const e = mouseEvent(false)
    f.mouse(e as unknown as MouseEvent)
    expect(f.term.focus).toHaveBeenCalledOnce()
    expect(e.stopImmediatePropagation).toHaveBeenCalledOnce()
    expect(e.target.dispatchEvent).not.toHaveBeenCalled()
  })

  it('starts Shift selection without requiring an old anchor or recursing', () => {
    vi.stubGlobal('MouseEvent', class {
      constructor(public type: string, init: MouseEventInit) { Object.assign(this, init) }
    })
    const f = fixture()
    const e = mouseEvent(true)
    e.target.dispatchEvent.mockImplementation(initial => {
      f.mouse(initial)
      return true
    })
    f.mouse(e as unknown as MouseEvent)
    expect(e.target.dispatchEvent).toHaveBeenCalledOnce()
    const initial = e.target.dispatchEvent.mock.calls[0][0]
    expect(initial.shiftKey).toBeFalsy()
    expect(initial.clientX).toBe(12)
    expect(initial.detail).toBe(1)
  })

  it.each(['x10', 'vt200', 'drag', 'any'])('leaves %s mouse and modifier selection to xterm', mode => {
    const f = fixture(mode, 'alternate')
    for (const shift of [false, true]) {
      const e = mouseEvent(shift)
      f.mouse(e as unknown as MouseEvent)
      expect(e.stopImmediatePropagation).not.toHaveBeenCalled()
      expect(e.target.dispatchEvent).not.toHaveBeenCalled()
    }
  })

  it('does not intercept scrollbar drags or secondary clicks and removes the listener', () => {
    const f = fixture()
    for (const e of [mouseEvent(false, false), mouseEvent(false, true, 2)]) {
      f.mouse(e as unknown as MouseEvent)
      expect(e.preventDefault).not.toHaveBeenCalled()
    }
    f.dispose()
    expect(f.container.removeEventListener).toHaveBeenCalledWith('mousedown', f.mouse, true)
  })

  it.each(['vt200', 'drag', 'any'])('lets xterm encode wheel input for %s', mode => {
    const f = fixture(mode, 'alternate')
    expect(f.wheel({} as WheelEvent)).toBe(true)
  })

  it('allows native normal history but does not synthesize keys for a non-reporting alternate screen', () => {
    const f = fixture()
    const e = { preventDefault: vi.fn(), stopPropagation: vi.fn() }
    expect(f.wheel(e as unknown as WheelEvent)).toBe(true)
    f.term.buffer.active.type = 'alternate'
    expect(f.wheel(e as unknown as WheelEvent)).toBe(false)
    expect(e.preventDefault).toHaveBeenCalledOnce()
    expect(e.stopPropagation).toHaveBeenCalledOnce()
  })
})
