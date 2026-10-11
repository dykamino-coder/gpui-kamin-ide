// Exercise the renderer lifecycle without requiring a browser or GPU. Real
// terminal painting and context loss still need the Windows runtime gate.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Terminal } from '@xterm/xterm'
import { enableTerminalRenderer } from './terminal-renderer'

const mock = vi.hoisted(() => ({
  constructorFails: false,
  loss: null as (() => void) | null,
  dispose: vi.fn(),
  unsubscribe: vi.fn(),
}))
vi.mock('@xterm/addon-webgl', () => ({
  WebglAddon: class {
    constructor() { if (mock.constructorFails) throw new Error('GPU unavailable') }
    onContextLoss(callback: () => void) {
      mock.loss = callback
      return { dispose: mock.unsubscribe }
    }
    dispose() { mock.dispose() }
  },
}))

function terminal(loadFails = false): Terminal {
  return {
    rows: 24,
    loadAddon: vi.fn(() => { if (loadFails) throw new Error('activation failed') }),
    refresh: vi.fn(),
  } as unknown as Terminal
}

describe('xterm 6 WebGL to DOM fallback', () => {
  beforeEach(() => {
    mock.constructorFails = false
    mock.loss = null
    vi.clearAllMocks()
  })

  it('keeps the default renderer when creating WebGL fails', () => {
    mock.constructorFails = true
    const term = terminal()
    const dispose = enableTerminalRenderer(term)
    expect(term.loadAddon).not.toHaveBeenCalled()
    expect(term.refresh).toHaveBeenCalledWith(0, 23)
    expect(() => dispose()).not.toThrow()
  })

  it('releases a partly activated addon and repaints with the default renderer', () => {
    const term = terminal(true)
    enableTerminalRenderer(term)
    expect(mock.dispose).toHaveBeenCalledOnce()
    expect(mock.unsubscribe).toHaveBeenCalledOnce()
    expect(term.refresh).toHaveBeenCalledWith(0, 23)
  })

  it('restores the default renderer on context loss and tears down only once', () => {
    const term = terminal()
    const dispose = enableTerminalRenderer(term)
    mock.loss?.()
    expect(mock.dispose).toHaveBeenCalledOnce()
    expect(term.refresh).toHaveBeenCalledWith(0, 23)
    dispose()
    expect(mock.dispose).toHaveBeenCalledOnce()
  })

  it('unsubscribes and releases WebGL before terminal teardown without repainting', () => {
    const term = terminal()
    const dispose = enableTerminalRenderer(term)
    dispose()
    expect(mock.unsubscribe).toHaveBeenCalledOnce()
    expect(mock.dispose).toHaveBeenCalledOnce()
    expect(term.refresh).not.toHaveBeenCalled()
  })
})
