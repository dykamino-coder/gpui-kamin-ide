import { afterEach, expect, it, vi } from 'vitest'
import { batch } from '@preact/signals'

const fixture = vi.hoisted(() => ({
  effects: [] as Array<() => unknown>, refs: [] as Array<{ current: any }>,
  read: vi.fn(() => new Promise(() => {})), setState: vi.fn(),
}))
vi.mock('preact/hooks', () => ({
  useEffect: (fn: () => unknown) => fixture.effects.push(fn),
  useRef: (initial: unknown) => {
    const ref = { current: fixture.refs.length === 1 ? { setState: fixture.setState } : initial }
    fixture.refs.push(ref)
    return ref
  },
}))
vi.mock('@bridge/storage', () => ({ storage: { getItem: () => null, setItem: vi.fn(), removeItem: vi.fn() } }))
vi.mock('../../hooks/useBridge', () => ({ useBridge: () => ({ fileViewerRead: fixture.read }) }))
import { FileEditor } from './FileEditor'
import { activeTabId } from '../../signals/tabs'
import { activeFileByTab, activeSelectionByTab, openFilesByTab } from '../../signals/file-viewer'

afterEach(() => {
  fixture.effects = []; fixture.refs = []
  batch(() => {
    activeTabId.value = null; activeFileByTab.value = new Map()
    activeSelectionByTab.value = new Map(); openFilesByTab.value = new Map()
  })
  vi.clearAllMocks()
})

it.each([null, '/workspace/previous.txt'])('ignores the delayed file effect for %s after host recovery', previous => {
  activeTabId.value = 'chat'
  activeFileByTab.value = new Map([['chat', previous]])
  FileEditor()
  const delayedEffect = fixture.effects.at(-1)!
  const restored = { path: '/workspace/current.txt', startLine: 2, endLine: 2,
    text: 'second', anchor: 11, head: 17 }
  // Reproduce the host snapshot arriving after render, before passive effects.
  batch(() => {
    activeFileByTab.value = new Map([['chat', restored.path]])
    activeSelectionByTab.value = new Map([['chat', restored]])
  })
  delayedEffect()
  expect(activeSelectionByTab.value.get('chat')).toEqual(restored)
  expect(fixture.setState).not.toHaveBeenCalled()
  expect(fixture.read).not.toHaveBeenCalled()
})

it('still clears the editor and context for a current empty-file render', () => {
  activeTabId.value = 'chat'
  activeSelectionByTab.value = new Map([['chat', { path: '/workspace/closed.txt', startLine: 1, endLine: 1, text: '' }]])
  FileEditor()
  fixture.effects.at(-1)!()
  expect(fixture.setState).toHaveBeenCalledOnce()
  expect(activeSelectionByTab.value.get('chat')).toBeNull()
  expect(fixture.read).not.toHaveBeenCalled()
})
