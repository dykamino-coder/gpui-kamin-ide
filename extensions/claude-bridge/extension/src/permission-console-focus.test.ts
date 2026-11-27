import { expect, it, vi } from 'vitest'
import type { WebviewView } from 'vscode'
import { PermissionConsoleFocus } from './permission-console-focus'

function viewFixture() {
  let message: (message: unknown) => void = () => {}
  let visibility = () => {}
  const disposable = { dispose() {} }
  const view = {
    visible: true,
    webview: {
      postMessage: vi.fn(),
      onDidReceiveMessage(fn: typeof message) { message = fn; return disposable },
    },
    onDidChangeVisibility(fn: typeof visibility) { visibility = fn; return disposable },
    onDidDispose: () => disposable,
  }
  return {
    view: view as unknown as WebviewView,
    ready: (id: string, runtime = 'first') => message({ kind: 'send', channel: 'permission-console:ready', args: [id, runtime] }),
    setVisible: (visible: boolean) => { view.visible = visible; visibility() },
    posted: view.webview.postMessage,
  }
}

it('hands focus to the requested mounted Claude terminal once, including a cold reveal', () => {
  let active = 'a'
  const reveal = vi.fn()
  const focus = vi.fn()
  const action = new PermissionConsoleFocus(id => id === active ? 'first' : undefined, reveal, focus)
  const console = viewFixture()
  action.request('a')
  expect(reveal).toHaveBeenCalledTimes(1)
  action.attach(console.view)
  console.ready('b')
  expect(focus).not.toHaveBeenCalled()
  console.ready('a')
  console.ready('a')
  expect(focus).toHaveBeenCalledTimes(1)
  expect(console.posted).toHaveBeenLastCalledWith({ kind: 'event', channel: 'permission-console:focus', args: ['a', 'first'] })

  action.request('a')
  console.ready('a')
  expect(focus).toHaveBeenCalledTimes(2)
  action.request('a')
  active = 'b'
  console.ready('a')
  console.ready('b')
  expect(focus).toHaveBeenCalledTimes(2)
  action.request('b')
  console.setVisible(false)
  console.setVisible(true)
  console.ready('b')
  expect(focus).toHaveBeenCalledTimes(2)
})

it('discards a late terminal acknowledgement after a Claude to Codex to Claude round trip in one tab', () => {
  let runtime: string | undefined = 'original'
  const focus = vi.fn()
  const action = new PermissionConsoleFocus(() => runtime, vi.fn(), focus)
  const console = viewFixture()
  action.attach(console.view)
  action.request('same-tab')
  runtime = undefined // Codex owns the tab.
  action.request('same-tab')
  runtime = 'returned'
  console.ready('same-tab', 'original')
  console.ready('same-tab', 'returned')
  expect(focus).not.toHaveBeenCalled()
  action.request('same-tab')
  console.ready('same-tab', 'original')
  expect(focus).not.toHaveBeenCalled()
  console.ready('same-tab', 'returned')
  expect(focus).toHaveBeenCalledOnce()
})
