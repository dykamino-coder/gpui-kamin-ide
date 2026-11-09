import assert from 'node:assert/strict'
import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { setTimeout as sleep } from 'node:timers/promises'
import type { TestContext } from 'node:test'
import { TerminalWorkspaceBackend } from '../src/terminals.js'
import type { HostFrame, WorkspaceBackend } from '../src/protocol.js'

export async function until(check: () => boolean | Promise<boolean>, message = 'Condition timed out'): Promise<void> {
  const deadline = Date.now() + 7000
  while (!await check()) {
    if (Date.now() > deadline) throw new Error(message)
    await sleep(25)
  }
}

export function alive(pid: number): boolean {
  try { process.kill(pid, 0); return true } catch { return false }
}

export async function fixture(t: TestContext) {
  const root = await mkdtemp(join(tmpdir(), 'bridgeweb-pty-'))
  const delegated: unknown[] = []
  let disposed = 0
  const next: WorkspaceBackend = {
    attach: () => ({ receive: async f => { delegated.push(f) }, dispose: () => { disposed++ } }),
  }
  const backend = new TerminalWorkspaceBackend(next, root)
  t.after(async () => {
    backend.dispose()
    assert.equal(resolve(root).startsWith(resolve(tmpdir()) + '\\') ||
      resolve(root).startsWith(resolve(tmpdir()) + '/'), true)
    await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 50 })
  })
  let request = 0
  const attach = () => {
    const frames: HostFrame[] = []
    let accepting = true
    const view = backend.attach({ postMessage: async frame => { frames.push(frame); return accepting } })
    const invoke = async (channel: string, ...args: unknown[]) => {
      const id = ++request
      await view.receive({ kind: 'invoke', id, generation: 'terminal-fixture', channel, args })
      return frames.find(f => f.kind === 'invoke-reply' && f.id === id)!
    }
    const send = (channel: string, value: unknown) => view.receive({ kind: 'send', channel, args: [value] })
    const spawn = async () => {
      const reply = await invoke('pty:spawn', { cwd: root, cols: 80, rows: 24, shellId: process.platform === 'win32' ? 'cmd' : 'sh' })
      assert.equal(reply.ok, true, String(reply.error))
      return (reply.result as { ptyId: string }).ptyId
    }
    return { view, frames, invoke, send, spawn, rejectOutput: () => { accepting = false } }
  }
  const read = async (name: string) => {
    try { return await readFile(join(root, name), 'utf8') } catch { return '' }
  }
  return { backend, root, attach, read, delegated, disposed: () => disposed }
}

export function nodeCommand(script: string): string {
  assert.equal(script.includes('"'), false, 'fixture script must contain no shell double quotes')
  return `"${process.execPath}" -e "${script}"\r`
}
