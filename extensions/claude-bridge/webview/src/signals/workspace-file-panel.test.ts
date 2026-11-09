import { afterEach, expect, it, vi } from 'vitest'
import { batch } from '@preact/signals'
import type { FilePanelState } from '../../shared/file-panel-state'

const host = vi.hoisted(() => ({ receivers: new Set<(frame: unknown) => void>(), postMessage: vi.fn() }))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
vi.mock('../lib/webview-api', () => ({ vscodeApi: {
  postMessage: host.postMessage,
  onHostMessage: (fn: (frame: unknown) => void) => { host.receivers.add(fn); return () => host.receivers.delete(fn) },
} }))
import { installWorkspaceFilePanel } from './workspace-file-panel'
import { activeTabId } from './tabs'
import { activeFileByTab, activeSelectionByTab, attachActiveFile, closeFileInTab, openFileInTab,
  openFilesByTab, previewModeByPath, setActiveSelection, setPreviewMode } from './file-viewer'

let cleanup = () => {}
afterEach(() => {
  cleanup(); cleanup = () => {}
  batch(() => {
    openFilesByTab.value = new Map(); activeFileByTab.value = new Map(); activeSelectionByTab.value = new Map()
    previewModeByPath.value = new Map(); attachActiveFile.value = false; activeTabId.value = null
  })
  vi.restoreAllMocks(); vi.clearAllMocks()
})
const state = (): FilePanelState => ({ version: 1, tabs: {
  one: { files: ['/workspace/b.html', '/workspace/a.txt'], active: '/workspace/a.txt',
    selection: { path: '/workspace/a.txt', startLine: 2, endLine: 2, text: 'selection', anchor: 12, head: 3 } },
  two: { files: [], active: null, selection: null },
}, previewModes: [['/workspace/b.html', 'preview']], attachActiveFile: true })
function snapshot(panel?: FilePanelState) {
  for (const fn of host.receivers) fn({ kind: 'event', channel: 'workspace:files-state', args: [{ panel, files: ['/workspace/watched-but-closed'] }] })
}
function sent(): FilePanelState { return host.postMessage.mock.lastCall![0].args[0] }

it('restores all control maps before publishing changes, without feeding sync back to the host', () => {
  cleanup = installWorkspaceFilePanel()
  openFileInTab('stale', '/workspace/old')
  expect(host.postMessage).not.toHaveBeenCalled()
  snapshot(state())
  expect([...openFilesByTab.value]).toEqual([
    ['one', [{ path: '/workspace/b.html', mtimeMs: 0, dirty: false }, { path: '/workspace/a.txt', mtimeMs: 0, dirty: false }]], ['two', []],
  ])
  expect(activeFileByTab.value.get('one')).toBe('/workspace/a.txt')
  expect(activeSelectionByTab.value.get('one')).toEqual(state().tabs.one.selection)
  expect([...previewModeByPath.value]).toEqual(state().previewModes)
  expect(attachActiveFile.value).toBe(true)
  expect(host.postMessage).not.toHaveBeenCalled()
  // No debounce: an explicit close/toggle is retained before the page closes.
  closeFileInTab('one', '/workspace/a.txt')
  expect(sent().tabs.one.files).toEqual(['/workspace/b.html'])
  expect(sent().tabs.one.active).toBe('/workspace/b.html')
  expect(sent().tabs.one.selection).toBe(null)
  attachActiveFile.value = false
  expect(sent().attachActiveFile).toBe(false)
  setPreviewMode('one', '/workspace/b.html', 'code')
  expect(sent().previewModes).toEqual([['/workspace/b.html', 'code']])
  const selection = { path: '/workspace/b.html', startLine: 1, endLine: 1, text: 'x', anchor: 1, head: 0 }
  setActiveSelection('one', selection)
  expect(sent().tabs.one.selection).toEqual(selection)
  const before = host.postMessage.mock.calls.length
  snapshot(state())
  expect(host.postMessage).toHaveBeenCalledTimes(before)
  expect(activeSelectionByTab.value.get('one')).toEqual(state().tabs.one.selection)
  cleanup(); cleanup = () => {}
  expect(host.receivers.size).toBe(0)
  openFileInTab('two', '/workspace/after-cleanup')
  expect(host.postMessage).toHaveBeenCalledTimes(before)
})

it('keeps legacy watch-list recovery but honors an explicitly empty retained panel', () => {
  activeTabId.value = 'one'
  cleanup = installWorkspaceFilePanel()
  snapshot()
  expect(openFilesByTab.value.get('one')?.map(f => f.path)).toEqual(['/workspace/watched-but-closed'])
  const empty = state(); empty.tabs.one = { files: [], active: null, selection: null }
  snapshot(empty)
  expect(openFilesByTab.value.get('one')).toEqual([])
  expect(activeFileByTab.value.get('one')).toBe(null)
  expect(host.postMessage).not.toHaveBeenCalled()
})

it('reports an offline or oversized update without crashing or pretending it was retained', () => {
  const warning = vi.spyOn(console, 'warn').mockImplementation(() => {})
  cleanup = installWorkspaceFilePanel(); snapshot(state())
  host.postMessage.mockImplementationOnce(() => { throw new Error('offline') })
  attachActiveFile.value = false
  expect(warning).toHaveBeenCalledTimes(1)
  // A subsequent valid update retries the full current projection.
  setPreviewMode('one', '/workspace/b.html', 'code')
  expect(sent().attachActiveFile).toBe(false)
  setActiveSelection('one', { path: '/workspace/a.txt', startLine: 1, endLine: 1, text: 'я'.repeat(1024 * 1024) })
  expect(warning).toHaveBeenCalledTimes(2)
  expect(warning.mock.calls.flat().join(' ')).not.toContain('я')
})
