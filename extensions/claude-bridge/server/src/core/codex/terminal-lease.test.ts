import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { expect, it, vi } from 'vitest'
import { CodexRuntime } from './codex-runtime'
import { CodexHost } from './codex-host'
import { CODEX_HOME_LOCK_FILE } from './codex-home-lock'
import { FakeCodexProcess } from './fixtures/fake-process'

it('leases the real home lock, excludes account writers, and publishes a fresh account after CLI exit', async () => {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-terminal-'))
  let account: { type: string; email: string } | null = null
  const processes: FakeCodexProcess[] = []
  const runtime = new CodexRuntime({
    home,
    env: {},
    startHost: async () => {
      const proc = new FakeCodexProcess({
        'account/read': () => ({ account }),
        'account/logout': () => {
          account = null
        },
      })
      processes.push(proc)
      return new CodexHost(proc)
    },
  })
  const changed = vi.fn()
  runtime.onAccountChange(changed)
  try {
    await runtime.accountStatus()
    const release = await runtime.acquireTerminal()
    expect(processes[0]!.exited).toBeDefined()
    expect(fs.existsSync(path.join(home, CODEX_HOME_LOCK_FILE))).toBe(true)
    await expect(runtime.startDeviceLogin()).rejects.toThrow('Codex terminal')
    await expect(runtime.logout()).rejects.toThrow('Codex terminal')
    await expect(runtime.acquireTerminal()).rejects.toThrow('Codex terminal')
    account = { type: 'chatgpt', email: 'test@example.test' }
    await release()
    await release()
    expect(fs.existsSync(path.join(home, CODEX_HOME_LOCK_FILE))).toBe(false)
    expect(changed).toHaveBeenCalledOnce()
    expect(changed).toHaveBeenCalledWith(expect.objectContaining({ loggedIn: true, email: 'test@example.test' }))
    expect(processes).toHaveLength(2)
    await runtime.logout()
    expect(await runtime.accountStatus()).toMatchObject({ loggedIn: false })
    expect(processes[1]!.requests('account/read')).toHaveLength(2)
  } finally {
    await runtime.close()
    fs.rmSync(home, { recursive: true, force: true })
  }
})

it('prepares the runtime home before the first CLI and honors its container environment', async () => {
  const operator = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-terminal-env-'))
  const home = path.join(operator, '.codex')
  const homeProbe = vi.spyOn(os, 'homedir').mockReturnValue(operator)
  const startHost = vi.fn(
    async () => new CodexHost(new FakeCodexProcess({ 'account/read': () => ({ account: null }) })),
  )
  const runtime = new CodexRuntime({ home, env: { BRIDGE_CONTAINER: '1', BRIDGE_CODEX_HOME: home }, startHost })
  try {
    const release = await runtime.acquireTerminal()
    expect(startHost).toHaveBeenCalledOnce()
    expect(startHost).toHaveBeenCalledWith(home)
    expect(fs.existsSync(path.join(home, CODEX_HOME_LOCK_FILE))).toBe(true)
    await release()
    expect(startHost).toHaveBeenCalledTimes(2)
  } finally {
    await runtime.close()
    homeProbe.mockRestore()
    fs.rmSync(operator, { recursive: true, force: true })
  }
})
