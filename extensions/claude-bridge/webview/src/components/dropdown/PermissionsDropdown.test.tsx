import { beforeEach, expect, it, vi } from 'vitest'
import type { VNode } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'

vi.mock('preact/hooks', () => ({ useState: () => [false, vi.fn()] }))
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => ({ changePermissionMode: vi.fn() }) }))

import { PermissionsDropdown } from './PermissionsDropdown'
import { activeTabId } from '../../signals/tabs'
import { currentPermission } from '../../signals/ui'
import { jsonlEntriesByTab } from '../../signals/jsonl'

function label(entries: unknown[]): string {
  jsonlEntriesByTab.value = new Map([['tab', entries as JsonlEntryData[]]])
  return (PermissionsDropdown() as VNode<any>).props.trigger.props.label
}

beforeEach(() => {
  activeTabId.value = 'tab'
  currentPermission.value = 'bypassPermissions'
  jsonlEntriesByTab.value = new Map()
})

it('keeps the selected permission after /clear emits an ordinary conversation mode', () => {
  expect(label([{ type: 'mode', mode: 'normal' }])).toBe('Bypass permissions')
})

it('does not let a conversation mode hide the last transcript permission', () => {
  expect(label([
    { type: 'user', permissionMode: 'acceptEdits' },
    { type: 'mode', mode: 'normal' },
  ])).toBe('Accept edits')
})

it('ignores unsupported inline permission values', () => {
  expect(label([{ type: 'user', permissionMode: 'future-conversation-mode' }])).toBe('Bypass permissions')
})

it.each([
  ['default', 'Default'], ['acceptEdits', 'Accept edits'], ['plan', 'Plan mode'],
  ['auto', 'Auto'], ['dontAsk', "Don't ask"], ['bypassPermissions', 'Bypass permissions'],
])('still displays a fresh supported permission %s', (mode, expected) => {
  expect(label([{ type: 'permission-mode', mode }])).toBe(expected)
  expect(label([{ type: 'mode', mode }])).toBe(expected)
  expect(label([{ type: 'assistant', permissionMode: mode }])).toBe(expected)
})

it('prefers the latest supported permission over the selected fallback', () => {
  expect(label([
    { type: 'permission-mode', mode: 'bypassPermissions' },
    { type: 'user', permissionMode: 'plan' },
  ])).toBe('Plan mode')
})
