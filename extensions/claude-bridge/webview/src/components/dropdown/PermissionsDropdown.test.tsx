import { beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { TabInfo } from '../../../shared/types'
import type { JsonlEntryData } from '../../types/jsonl'
import { DropdownOption } from './DropdownOption'

const api = vi.hoisted(() => ({
  changeClaudePermissionMode: vi.fn(), changePermissionMode: vi.fn(), openPermissionConsole: vi.fn(), changeCodexCollaboration: vi.fn(),
}))
vi.mock('preact/hooks', () => ({ useState: () => [null, vi.fn()] }))
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => api }))

import { PermissionsDropdown } from './PermissionsDropdown'
import { WorkspaceApprovalsDropdown } from './WorkspaceApprovalsDropdown'
import { activeTabId, tabs } from '../../signals/tabs'
import { jsonlEntriesByTab } from '../../signals/jsonl'

function show(overrides: Partial<TabInfo> = {}, component = PermissionsDropdown) {
  tabs.value = [{ id: 'tab', conversationId: 'chat', sessionId: 'session', engine: 'claude',
    status: 'connected', workspacePermission: 'ask', ...overrides } as TabInfo]
  const root = component() as VNode<any>
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
  expect(label).toBe('Accept edits')
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
  const { options } = show({ engine: 'codex' }, WorkspaceApprovalsDropdown)
  options.get('Read tools only')!.onClick()
  expect(api.changePermissionMode).toHaveBeenCalledWith('tab', 'readOnly')
  expect(show({ workspacePermission: 'readOnly' }, WorkspaceApprovalsDropdown).options.get('Read tools only')?.selected).toBe(true)
  expect(show({ workspacePermission: undefined }, WorkspaceApprovalsDropdown).options.get('Allow workspace tools')!.disabled).toBe(true)
  expect(show({ status: 'connecting' }).options.get('Change mode in Console')!.disabled).toBe(true)
})

it('lets the user clear a saved Plan override when no presets are available', () => {
  const { options } = show({ engine: 'codex', codexCollaboration: { available: [], selected: 'plan' } })
  expect(options.has('Plan')).toBe(false)
  options.get('Reset to engine default')!.onClick()
  expect(api.changeCodexCollaboration).toHaveBeenCalledWith('tab', 'session', 'default')
})

it('switches the engine control on the same tab and keeps host approvals in their own menu', () => {
  jsonlEntriesByTab.value = new Map([['tab', [
    { type: 'permission-mode', permissionMode: 'bypassPermissions', sessionId: 'chat' },
    { type: 'permission-mode', permissionMode: 'plan', sessionId: 'chat', isSidechain: true },
  ] as unknown as JsonlEntryData[]]])
  const host = { workspacePermission: 'readOnly' } as const
  const claude = show(host)
  expect(claude.label).toBe('Bypass permissions')
  const codex = show({ ...host, engine: 'codex', codexCollaboration: { available: ['default', 'plan'], selected: 'plan' } })
  expect(codex.label).toBe('Codex · Plan')
  expect(codex.options.has('Change mode in Console')).toBe(false)
  const returned = show({ ...host, engine: 'claude' })
  expect(returned.label).toBe(claude.label)
  expect(returned.options.has('Default')).toBe(false)
  for (const menu of [claude, codex, returned]) expect(menu.options.has('Allow workspace tools')).toBe(false)
  expect(show(host, WorkspaceApprovalsDropdown).options.get('Read tools only')!.selected).toBe(true)
  expect(show({ ...host, conversationId: 'fresh' }).label).toBe('Claude permissions')
})

it('offers only Claude-advertised modes and waits for a native mode receipt', () => {
  const native = { available: ['default', 'acceptEdits', 'plan'], selected: 'default', requested: 'plan' }
  const { options, label } = show({ claudePermissions: native })
  expect(label).toBe('Manual')
  expect(options.get('Manual')?.selected).toBe(true)
  expect(options.get('Plan')?.selected).toBe(false)
  expect(options.has('Bypass permissions')).toBe(false)
  options.get('Plan')!.onClick()
  expect(api.changeClaudePermissionMode).toHaveBeenCalledWith('tab', 'session', 'plan')
  expect(api.changePermissionMode).not.toHaveBeenCalled()
  expect(show({ engine: 'codex', claudePermissions: native }).options.has('Manual')).toBe(false)
})
