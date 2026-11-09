import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, expect, test, vi } from 'vitest'
import { ConfigStore } from './store'

const roots: string[] = []
function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-credential-'))
  roots.push(root)
  return { root, file: path.join(root, 'open-claude-bridge-config.json') }
}
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllEnvs()
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true })
})

test('memory credential survives every store mutation but never enters atomic disk writes', () => {
  const { root, file } = fixture()
  const store = new ConfigStore(root, false)
  const credential = 'synthetic-memory-only-token'
  const writes = vi.spyOn(fs, 'writeFileSync')
  store.set({ serverUrl: 'ws://fixture.invalid', token: credential, cwd: root })
  store.setLayout({ sidebarWidth: 321 })
  store.setNotifications({ backgroundToasts: false })
  store.setTerminalPrefs({ useConptyDll: true })
  store.setBasePrompt('standing instruction')
  store.addSavedSession({ conversationId: 'fixture', cwd: root, label: 'Fixture', folderName: 'test',
    model: 'fixture-model', lastActivity: '2026-11-09', messageCount: 1 })
  expect(store.getSavedSessions()).toHaveLength(1)
  store.removeSavedSession('fixture')
  expect(store.get().token).toBe(credential)
  expect(store.getBasePrompt()).toBe('standing instruction')
  expect(writes.mock.calls).toHaveLength(7)
  for (const [target, content] of writes.mock.calls) {
    expect(target).toBe(`${file}.tmp`)
    expect(String(content).includes(credential)).toBe(false)
    expect(JSON.parse(String(content)).config.token).toBe('')
  }
  const reopened = new ConfigStore(root, false)
  expect(reopened.get().token).toBe('')
  expect(reopened.getLayout().sidebarWidth).toBe(321)
  reopened.set({ ...reopened.get(), token: credential })
  expect(reopened.getBasePrompt()).toBe('standing instruction')
  expect(fs.existsSync(`${file}.tmp`)).toBe(false)
})

test('memory mode ignores stale disk credentials and never imports desktop migration data', () => {
  const { root, file } = fixture()
  fs.writeFileSync(file, JSON.stringify({ config: { serverUrl: 'ws://old.invalid', token: 'synthetic-old-token' } }))
  vi.stubEnv('APPDATA', root)
  const reads = vi.spyOn(fs, 'readFileSync')
  const store = new ConfigStore(root, false)
  expect(store.get().token).toBe('')
  store.migrateFromElectron()
  expect(reads.mock.calls.map(([target]) => target)).toEqual([file])
  store.setLayout({ filePanelVisible: true })
  expect(fs.readFileSync(file, 'utf8').includes('synthetic-old-token')).toBe(false)
})

test('failed atomic rename cannot leave a credential in a temporary config', () => {
  const { root, file } = fixture()
  const store = new ConfigStore(root, false)
  vi.spyOn(fs, 'renameSync').mockImplementation(() => { throw new Error('synthetic write failure') })
  store.set({ serverUrl: 'ws://fixture.invalid', token: 'synthetic-temporary-token' })
  expect(store.get().token).toBe('synthetic-temporary-token')
  expect(JSON.parse(fs.readFileSync(`${file}.tmp`, 'utf8')).config.token).toBe('')
})

test('desktop default retains existing persisted credential behavior', () => {
  const { root } = fixture()
  const store = new ConfigStore(root)
  store.set({ serverUrl: 'ws://fixture.invalid', token: 'synthetic-desktop-token' })
  expect(new ConfigStore(root).get().token).toBe('synthetic-desktop-token')
})
