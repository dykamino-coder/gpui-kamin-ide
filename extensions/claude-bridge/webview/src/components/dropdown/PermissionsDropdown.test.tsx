import { beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { TabInfo } from '../../../shared/types'
import type { JsonlEntryData } from '../../types/jsonl'
import { DropdownOption } from './DropdownOption'

const api = vi.hoisted(() => ({
  changePermissionMode: vi.fn(), openPermissionConsole: vi.fn(), changeCodexCollaboration: vi.fn(),
}))
vi.mock('preact/hooks', () => ({ useState: () => [null, vi.fn()] }))
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => api }))

import { PermissionsDropdown } from './PermissionsDropdown'
import { activeTabId, tabs } from '../../signals/tabs'
import { jsonlEntriesByTab } from '../../signals/jsonl'

function show(overrides: Partial<TabInfo> = {}) {
  tabs.value = [{ id: 'tab', conversationId: 'chat', sessionId: 'session', engine: 'claude',
    status: 'connected', workspacePermission: 'ask', ...overrides } as TabInfo]
  const root = PermissionsDropdown() as VNode<any>
  const options = new Map<string, Parameters<typeof DropdownOption>[0]>()
  function visit(child: unknown): void {
    if (Array.isArray(child)) return child.forEach(visit)
    if (!child || typeof child !== 'object') return
    const node = child as VNode<any>
    if (node.type === DropdownOption) options.set(node.props.name, node.props)
    else visit(node.props?.children)
  }
  visit(root.props.children)
  return { label: root.props.trigger.props.label, options }
}

beforeEach(() => {
  vi.clearAllMocks()
  activeTabId.value = 'tab'
  jsonlEntriesByTab.value = new Map()
})

it('uses the active Claude transcript and opens the native permission selector in Console', () => {
  jsonlEntriesByTab.value = new Map([['tab', [
    { type: 'user', permissionMode: 'acceptEdits', sessionId: 'chat' },
    { type: 'mode', mode: 'normal' }, { type: 'user', permissionMode: 'plan', sessionId: 'other' },
    { type: 'user', permissionMode: 'bypassPermissions', engine: 'codex' },
  ] as unknown as JsonlEntryData[]]])
  const { options, label } = show()
  expect(label).toBe('Use approval rules')
  const action = options.get('Change mode in Console')!
  expect(action.description).toContain('Last reported: Accept edits')
  expect(action.disabled).toBe(false)
  action.onClick()
  expect(api.openPermissionConsole).toHaveBeenCalledWith('tab')
  expect(options.has('Plan')).toBe(false)
})

it('offers exactly the advertised Codex presets and waits for the server selection', () => {
  const { options } = show({ engine: 'codex', codexCollaboration: { available: ['default', 'plan'], selected: 'default' } })
  expect(options.has('Change mode in Console')).toBe(false)
  expect(options.get('Default')!.selected).toBe(true)
  expect(options.get('Plan')!.selected).toBe(false)
  expect(options.get('Plan')!.disabled).toBe(false)
  options.get('Plan')!.onClick()
  expect(api.changeCodexCollaboration).toHaveBeenCalledWith('tab', 'session', 'plan')
  expect(show({ engine: 'codex', codexCollaboration: { available: ['default'], selected: 'default' } }).options.has('Plan')).toBe(false)
  expect(show({ engine: 'codex' }).options.has('Default')).toBe(false)
})

it('changes workspace approval independently and displays the acknowledged policy', () => {
  const { options } = show({ engine: 'codex' })
  options.get('Read tools only')!.onClick()
  expect(api.changePermissionMode).toHaveBeenCalledWith('tab', 'readOnly')
  expect(show({ workspacePermission: 'readOnly' }).label).toBe('Read tools only')
  expect(show({ workspacePermission: undefined }).options.get('Allow workspace tools')!.disabled).toBe(true)
  expect(show({ status: 'connecting' }).options.get('Change mode in Console')!.disabled).toBe(true)
})

it('lets the user clear a saved Plan override when no presets are available', () => {
  const { options } = show({ engine: 'codex', codexCollaboration: { available: [], selected: 'plan' } })
  expect(options.has('Plan')).toBe(false)
  options.get('Reset to engine default')!.onClick()
  expect(api.changeCodexCollaboration).toHaveBeenCalledWith('tab', 'session', 'default')
})
