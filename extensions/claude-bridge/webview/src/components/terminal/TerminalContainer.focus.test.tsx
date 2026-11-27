// @vitest-environment jsdom
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { expect, it, vi } from 'vitest'
import type { TabInfo } from '../../../shared/types'
import { activeTabId, tabs } from '../../signals/tabs'
import { TerminalContainer } from './TerminalContainer'

const messages = vi.hoisted(() => ({
  listeners: new Map<string, (id: string, runtime: string) => void>(),
  ready: vi.fn(),
  focus: vi.fn(),
}))
vi.mock('../../lib/bridge-transport', () => ({
  snd: messages.ready,
  sub(channel: string, fn: (id: string, runtime: string) => void) {
    messages.listeners.set(channel, fn)
    return () => messages.listeners.delete(channel)
  },
}))
vi.mock('../../hooks/useTerminal', () => ({
  useTerminal: () => ({
    containerRef: { current: null }, terminalRef: { current: { focus: messages.focus } },
    fitAddonRef: { current: null }, hasOutput: true,
  }),
}))
vi.mock('../../hooks/useTerminalResize', () => ({ useTerminalResize() {} }))

it('keeps keyboard focus on normal mount and gives it only to the requested active Claude terminal', async () => {
  const root = document.createElement('div')
  const composer = document.createElement('input')
  const terminalInput = document.createElement('textarea')
  document.body.append(root, composer, terminalInput)
  messages.focus.mockImplementation(() => terminalInput.focus())
  composer.focus()
  activeTabId.value = 'ours'
  tabs.value = [{ id: 'ours', sessionId: 'runtime', engine: 'claude', status: 'connected' } as TabInfo]
  try {
    await act(() => render(<TerminalContainer tabId="ours" active fill />, root))
    expect(messages.ready).toHaveBeenCalledWith('permission-console:ready', 'ours', 'runtime')
    expect(document.activeElement).toBe(composer)
    const focus = messages.listeners.get('permission-console:focus')!
    focus('sibling', 'runtime')
    expect(document.activeElement).toBe(composer)
    focus('ours', 'old-runtime')
    expect(document.activeElement).toBe(composer)
    focus('ours', 'runtime')
    expect(document.activeElement).toBe(terminalInput)
    composer.focus()
    tabs.value = [{ ...tabs.value[0]!, sessionId: 'returned' }]
    focus('ours', 'runtime')
    expect(document.activeElement).toBe(composer)
    activeTabId.value = 'sibling'
    focus('ours', 'returned')
    expect(document.activeElement).toBe(composer)
  } finally {
    await act(() => render(null, root))
    root.remove()
    composer.remove()
    terminalInput.remove()
    activeTabId.value = null
    tabs.value = []
  }
})
