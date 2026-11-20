// @vitest-environment jsdom
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

vi.mock('../../hooks/useBridge', () => ({ useBridge: () => ({ submitText: vi.fn() }) }))

import { UsageLimitsRow, ROW_HEIGHT } from './UsageLimitsRow'
import { activeTabId, tabs } from '../../signals/tabs'
import { setTabUsage, usageByTab } from '../../signals/session-usage'
import type { SessionUsage, TabInfo } from '../../../shared/types'

const claudeUsage = (): SessionUsage => ({
  windows: [{ id: 'seven_day', label: 'Week (all)', percent: 34, resetsAt: new Date(Date.now() + 86_400_000).toISOString() }],
  contextPercent: 12,
  contextWindowSize: 1_000_000,
  autoCompactAt: 900_000,
  model: 'claude-opus-5-5',
  effort: null,
  observedAt: new Date().toISOString(),
})

let container: HTMLDivElement
const row = (): HTMLElement => container.querySelector('[data-testid="usage-limits-row"]') as HTMLElement

function show(engine: 'claude' | 'codex'): void {
  tabs.value = [{ id: 't1', engine, model: engine === 'codex' ? 'gpt-5.5' : 'claude-opus-5-5' } as unknown as TabInfo]
  activeTabId.value = 't1'
  act(() => { render(<UsageLimitsRow />, container) })
}

describe('status row under the chat input', () => {
  beforeEach(() => {
    container = document.createElement('div')
    document.body.append(container)
    usageByTab.value = new Map()
  })
  afterEach(() => {
    act(() => { render(null, container) })
    container.remove()
  })

  it('reserves its height before any usage arrives, so late data cannot move the input', () => {
    expect(ROW_HEIGHT).toBe(23)
    show('claude')
    const before = row().getAttribute('style')!
    expect(before.replace(/\s+/g, '')).toContain(`min-height:${ROW_HEIGHT}px`)
    expect(row().textContent).toBe('')
    act(() => { setTabUsage('t1', claudeUsage()) })
    expect(row().textContent).toContain('7d')
    expect(row().getAttribute('style')).toBe(before)
  })

  it('a chat on ChatGPT does not show the Claude plan windows and says it has no plan data', () => {
    setTabUsage('t1', claudeUsage())
    show('codex')
    expect(row().textContent).not.toContain('7d')
    expect(row().textContent).not.toContain('34%')
    expect(container.querySelector('[data-testid="usage-plan-no-data"]')?.textContent).toBe('planno data')
  })

  it('shows ChatGPT plan windows once the server reports them for the codex agent', () => {
    setTabUsage('t1', { ...claudeUsage(), engine: 'codex', windows: [{ id: 'primary', label: '5 hours', percent: 20, resetsAt: new Date(Date.now() + 3_600_000).toISOString() }] })
    show('codex')
    expect(row().textContent).toContain('20%')
    expect(container.querySelector('[data-testid="usage-plan-no-data"]')).toBeNull()
  })
})
