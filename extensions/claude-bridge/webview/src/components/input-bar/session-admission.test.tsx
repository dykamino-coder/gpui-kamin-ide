// @vitest-environment jsdom
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { expect, it, vi } from 'vitest'
import type { KaminBridgeApi, TabInfo } from '../../../shared/types'
import { activeTabId, tabs } from '../../signals/tabs'
import { TerminalContainer } from '../terminal/TerminalContainer'
import { InputBar } from './InputBar'

vi.mock('../../hooks/useTerminal', () => ({
  useTerminal: () => ({ containerRef: { current: null }, terminalRef: { current: null }, fitAddonRef: { current: null }, hasOutput: false }),
}))
vi.mock('../../hooks/useTerminalResize', () => ({ useTerminalResize: () => {} }))

it('explains a refused slot in chat and console, keeps the draft, and unlocks the admitted tab without a turn', async () => {
  const root = document.createElement('div')
  document.body.append(root)
  const tab = { id: 'slot-test', engine: 'codex', status: 'connecting', refusal: 'session-limit' } as TabInfo
  window.kaminBridge = new Proxy({ getConfig: async () => ({}) }, {
    get: (obj, key) => key in obj ? Reflect.get(obj, key) : () => () => {},
  }) as unknown as KaminBridgeApi
  activeTabId.value = tab.id
  tabs.value = [tab]
  try {
    await act(() => render(<><InputBar /><TerminalContainer tabId={tab.id} active /></>, root))
    expect(root.querySelectorAll('[role="status"]').length).toBeGreaterThanOrEqual(2)
    expect(root.textContent).toContain('Close another connected chat')
    expect(root.textContent).not.toContain('Connecting to session')
    expect(root.querySelector('#input-controls')).toBeNull()
    expect(root.querySelector('[data-testid="usage-plan-no-data"]')).toBeNull()
    const input = root.querySelector('textarea')!
    input.value = 'saved draft'
    await act(() => { tabs.value = [{ ...tab, status: 'connected', refusal: undefined, sessionId: 'synthetic', model: 'gpt-test', effort: 'high' }] })
    expect(root.querySelector('#input-controls')).not.toBeNull()
    expect(root.textContent).not.toContain('Session limit reached')
    expect(root.textContent).toContain('Codex is connected')
    expect(input.value).toBe('saved draft')
  } finally {
    act(() => render(null, root))
    root.remove()
  }
})

it('follows the active turn when an independent MCP catalog is still loading', async () => {
  const { mcpLoading, tabPromptReady } = await import('../../signals/connection')
  const { tabActivity } = await import('../../signals/ui')
  const tab = { id: 'progress-test', engine: 'codex', status: 'connected', sessionId: 'fixture' } as TabInfo
  window.kaminBridge = new Proxy({ getConfig: async () => ({}) }, {
    get: (obj, key) => key in obj ? Reflect.get(obj, key) : () => () => {},
  }) as unknown as KaminBridgeApi
  const root = document.createElement('div')
  document.body.append(root)
  const setActivity = (working: boolean) => {
    tabActivity.value = new Map([[tab.id, { rawTitle: '', isWorking: working }]])
    tabPromptReady.value = new Map([[tab.id, !working]])
  }
  activeTabId.value = tab.id
  tabs.value = [tab]
  mcpLoading.value = true
  setActivity(true)
  try {
    await act(() => render(<InputBar />, root))
    // Final assistant text may have arrived while turn/completed is pending.
    expect(root.querySelector('.send-btn-stop')).not.toBeNull()
    await act(() => setActivity(false))
    expect(root.querySelector('.send-btn-stop')).toBeNull()
    expect(root.querySelector('[data-tooltip="Send (Enter)"]')).not.toBeNull()
    await act(() => setActivity(true))
    expect(root.querySelector('.send-btn-stop')).not.toBeNull()
  } finally {
    act(() => render(null, root))
    root.remove()
    mcpLoading.value = false
    tabActivity.value = new Map()
    tabPromptReady.value = new Map()
  }
})
