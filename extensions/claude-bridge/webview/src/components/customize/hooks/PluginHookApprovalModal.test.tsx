// @vitest-environment jsdom
import { render } from 'preact'
import { act } from 'preact/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { PluginHookApprovalModal } from './PluginHookApprovalModal'

vi.mock('../../../signals/toasts', () => ({ showToast: vi.fn() }))

const approval = (pluginId: string) => ({
  pluginId,
  hooks: [{ event: 'PreToolUse', matcher: 'Read', handler: { type: 'command', command: 'echo synthetic' }, hash: pluginId }],
  approvedHashes: [],
})
type Approval = ReturnType<typeof approval>
let container: HTMLDivElement
let composer: HTMLTextAreaElement
let enqueue: (item: Approval) => void
let resolvePending: (items: Approval[]) => void
let save: ReturnType<typeof vi.fn>
let off: ReturnType<typeof vi.fn>

beforeEach(() => {
  vi.useFakeTimers()
  // Keep real Preact effects/DOM, including the old deferred focus frame.
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => setTimeout(() => callback(0), 16))
  vi.stubGlobal('cancelAnimationFrame', (id: number) => clearTimeout(id))
  vi.spyOn(HTMLElement.prototype, 'offsetParent', 'get').mockImplementation(function (this: HTMLElement) {
    return this.parentElement
  })
  composer = document.createElement('textarea')
  container = document.createElement('div')
  document.body.append(composer, container)
  composer.focus()
  save = vi.fn(async () => ({ ok: true }))
  off = vi.fn()
  window.kaminBridge = {
    onPluginHooksAwaitingApproval: (listener: typeof enqueue) => { enqueue = listener; return off },
    hooksListPendingPluginApprovals: () => new Promise<Approval[]>(resolve => { resolvePending = resolve }),
    hooksSetPluginApproval: save,
  } as unknown as Window['kaminBridge']
  act(() => { render(<PluginHookApprovalModal />, container) })
})
afterEach(() => {
  act(() => { render(null, container) })
  document.body.replaceChildren()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  vi.useRealTimers()
})

const dialog = () => container.querySelector<HTMLElement>('[role="dialog"]')!
async function settle() {
  await act(async () => { await vi.advanceTimersByTimeAsync(20) })
}
async function pending(items: Approval[]) {
  await act(async () => { resolvePending(items) })
  await settle()
}
async function reject() {
  const button = Array.from(container.querySelectorAll('button')).find(button => button.textContent === 'Reject all')!
  button.focus()
  await act(async () => { button.click() })
  await settle()
}
function tab(shiftKey = false) {
  const event = new KeyboardEvent('keydown', { key: 'Tab', shiftKey, bubbles: true, cancelable: true })
  document.activeElement!.dispatchEvent(event)
  expect(event.defaultPrevented).toBe(true)
}

it('focuses mount-time pending approvals as soon as the dialog is committed', async () => {
  await act(async () => { resolvePending([approval('first')]) })
  expect(document.activeElement).toBe(dialog())
})

it('keeps late composer autofocus and Tab navigation inside the dialog', async () => {
  await pending([approval('first')])
  expect(document.activeElement).toBe(dialog())
  // The composer retries autofocus while startup state is still settling.
  composer.focus()
  expect(document.activeElement).toBe(dialog())
  tab()
  expect(document.activeElement).toBe(container.querySelector('input'))
  tab(true)
  expect(document.activeElement?.textContent).toBe('Select all')
  tab()
  expect(document.activeElement).toBe(container.querySelector('input'))
})

it('restores the focus from before the FIRST modal after rejecting a two-plugin queue', async () => {
  await pending([approval('first'), approval('second')])
  expect(document.activeElement).toBe(dialog())
  const restored = vi.fn()
  composer.addEventListener('focus', restored)
  await reject()
  expect(dialog().textContent).toContain('second')
  expect(restored).not.toHaveBeenCalled()
  expect(document.activeElement).toBe(dialog())
  await reject()
  expect(dialog()).toBeNull()
  expect(document.activeElement).toBe(composer)
  expect(restored).toHaveBeenCalledOnce()
  expect(save.mock.calls).toEqual([['first', []], ['second', []]])
})

it('takes a new return target for a later event and restores it on unmount', async () => {
  await pending([approval('first')])
  await reject()
  expect(document.activeElement).toBe(composer)
  const nextTarget = document.createElement('button')
  document.body.append(nextTarget)
  nextTarget.focus()
  act(() => { enqueue(approval('later')) })
  await settle()
  expect(document.activeElement).toBe(dialog())
  act(() => { render(null, container) })
  expect(document.activeElement).toBe(nextTarget)
  expect(off).toHaveBeenCalledOnce()
})
