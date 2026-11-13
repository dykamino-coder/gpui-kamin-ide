import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

vi.mock('vscode', () => ({}))
vi.mock('../../plugin-helpers', async importOriginal => ({
  ...await importOriginal<typeof import('../../plugin-helpers')>(),
  readEffectivePluginManifest: async () => ({
    hooks: { PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'echo synthetic-review' }] }] },
  }),
}))

import { BrowserWindow, registerShimWindow, unregisterShimWindow } from '@kaminide/host-compat'
import { initBridgeState } from '../../bridge-state'
import { getPendingHookApproval, requestHookApproval } from './handlers-install'

let root: string
beforeEach(() => {
  root = fs.mkdtempSync(path.join(process.env.KAMIN_TEST_EVIDENCE_ROOT ?? os.tmpdir(), 'hook-delivery-'))
  initBridgeState(root)
})
afterEach(() => {
  for (const sink of [...BrowserWindow.getAllWindows()]) unregisterShimWindow(sink)
  fs.rmSync(root, { recursive: true, force: true })
})

function sink(destroyed = false) {
  const window = { isDestroyed: () => destroyed, webContents: { send: vi.fn() } }
  registerShimWindow(window)
  return window.webContents.send
}

it('delivers repeated approval requests to Chat even when Customize registered first', async () => {
  const customize = sink()
  const disposed = sink(true)
  const chat = sink()
  const anotherChat = sink()
  const pending = await getPendingHookApproval('sample@local', root)
  expect(pending?.hooks).toHaveLength(1)
  await requestHookApproval('sample@local', root)
  await requestHookApproval('sample@local', root)
  for (const send of [customize, chat, anotherChat]) {
    expect(send).toHaveBeenCalledTimes(2)
    expect(send).toHaveBeenLastCalledWith('plugin-hooks:awaiting-approval', pending)
  }
  expect(disposed).not.toHaveBeenCalled()
})

it('does not prompt any sink for an already reviewed hook', async () => {
  const chat = sink()
  const pending = await getPendingHookApproval('sample@local', root)
  fs.writeFileSync(path.join(root, 'plugin-hook-approvals.json'), JSON.stringify({
    'sample@local': pending!.hooks.map(hook => hook.hash),
  }))
  await requestHookApproval('sample@local', root)
  expect(chat).not.toHaveBeenCalled()
})
