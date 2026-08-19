import 'reflect-metadata'
import { spawn } from 'node:child_process'
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'
import { afterEach, describe, expect, it } from 'vitest'
import { buildHookCommandRelay, rewriteHooksForCli } from './proxy-rewriter'
import { clearSession, listSession, registerSessionHooks } from './registry'
import { cancelSessionLocalExecs, dispatchHook, handleLocalHookResponse } from './dispatcher'
import { buildClaudeArgs, buildSessionEnv } from '../pty/session-env'
import type { HookSettings } from './types'
import { findOwnedLiveSession } from './routes'

const sessions = ['plugin-async-test', 'plugin-sync-test', 'cancel-a', 'cancel-b']
afterEach(() => sessions.forEach(clearSession))

describe('plugin hook proxy', () => {
  it('never borrows a live client socket from another token owner', () => {
    const foreign = { tokenId: 'foreign', ws: { send() {} } }
    const ownedOffline = { tokenId: 'owner', ws: null }
    const ownedLive = { tokenId: 'owner', ws: { send() {} } }
    expect(findOwnedLiveSession([foreign, ownedOffline, ownedLive], 'owner')).toBe(ownedLive)
    expect(findOwnedLiveSession([foreign], 'owner')).toBeUndefined()
  })

  it('keeps async lifecycle flags and routes plugin commands to client host', () => {
    const hooks: HookSettings = {
      PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'guard', async: true, asyncRewake: true }] }],
    }
    const rewritten = rewriteHooksForCli('plugin-async-test', hooks, { kind: 'plugin', pluginId: 'guard@corp', manifestPath: '/plugin.json' }, 'token')
    const handler = rewritten.PreToolUse?.[0]?.hooks[0] as any
    expect(handler.type).toBe('command')
    expect(handler.command).toBe('node')
    expect(handler.args[0]).toBe('-e')
    expect(handler.args[1]).toContain('X-Bridge-Hook-Relay')
    expect(handler.async).toBe(true)
    expect(handler.asyncRewake).toBe(true)
    expect(listSession('plugin-async-test')[0]?.effectiveHost).toBe('local')
  })

  it('rewrites synchronous plugin commands to the authenticated command relay', async () => {
    const hooks: HookSettings = {
      PreToolUse: [{ matcher: 'Read', hooks: [{ type: 'command', command: 'guard-secrets' }] }],
    }
    const rewritten = rewriteHooksForCli('plugin-sync-test', hooks, { kind: 'plugin', pluginId: 'guard@corp', manifestPath: '/plugin.json' }, 'token')
    const handler = rewritten.PreToolUse?.[0]?.hooks[0] as any
    expect(handler.type).toBe('command')
    expect(handler.command).toBe('node')
    expect(handler.args[0]).toBe('-e')
    expect(handler.args[1]).toContain('/api/hooks/plugin-sync-test/PreToolUse/')
    expect(handler.args[1]).toContain('Bearer token')

    const registered = listSession('plugin-sync-test')[0]!
    let sent: any
    const ws = { send(raw: string) { sent = JSON.parse(raw) } } as any
    const pending = dispatchHook('plugin-sync-test', registered.id, { hook_event_name: 'PreToolUse', cwd: '/repo' } as any, ws)
    expect(sent.pluginId).toBe('guard@corp')
    handleLocalHookResponse(sent.requestId, {
      stdout: '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}', stderr: '', exitCode: 0,
      outcome: 'success', jsonOutput: { hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny' } }, durationMs: 1,
    })
    expect((await pending).jsonOutput).toEqual({ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny' } })
  })

  it('replays stdout, stderr, and exit code without HTTP-hook translation', async () => {
    let receivedBody = ''
    let receivedAuth = ''
    let receivedRelay = ''
    const server = createServer((req, res) => {
      req.setEncoding('utf8')
      req.on('data', chunk => { receivedBody += chunk })
      req.on('end', () => {
        receivedAuth = String(req.headers.authorization ?? '')
        receivedRelay = String(req.headers['x-bridge-hook-relay'] ?? '')
        res.setHeader('Content-Type', 'application/json')
        res.end(JSON.stringify({
          result: {
            stdout: '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}',
            stderr: 'blocked by guard',
            exitCode: 2,
            outcome: 'error',
            durationMs: 1,
          },
        }))
      })
    })
    await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
    const address = server.address() as AddressInfo
    const script = buildHookCommandRelay(`http://127.0.0.1:${address.port}/hook`, 'secret')
    try {
      const result = await new Promise<{ stdout: string; stderr: string; exitCode: number | null }>((resolve, reject) => {
        const child = spawn(process.execPath, ['-e', script], { stdio: ['pipe', 'pipe', 'pipe'] })
        let stdout = ''
        let stderr = ''
        child.stdout.on('data', chunk => { stdout += chunk.toString() })
        child.stderr.on('data', chunk => { stderr += chunk.toString() })
        child.on('error', reject)
        child.on('close', exitCode => resolve({ stdout, stderr, exitCode }))
        child.stdin.end('{"hook_event_name":"PreToolUse"}')
      })
      expect(receivedBody).toBe('{"hook_event_name":"PreToolUse"}')
      expect(receivedAuth).toBe('Bearer secret')
      expect(receivedRelay).toBe('command')
      expect(result).toEqual({
        stdout: '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny"}}',
        stderr: 'blocked by guard',
        exitCode: 2,
      })
    } finally {
      await new Promise<void>((resolve, reject) => server.close(err => err ? reject(err) : resolve()))
    }
  })

  it('cancels only hooks owned by the closing session', async () => {
    const hooks: HookSettings = { PreToolUse: [{ hooks: [{ type: 'command', command: 'guard' }] }] }
    registerSessionHooks('cancel-a', hooks, { kind: 'user' })
    registerSessionHooks('cancel-b', hooks, { kind: 'user' })
    const first = listSession('cancel-a')[0]!
    const second = listSession('cancel-b')[0]!
    let secondRequestId = ''
    const wsA = { send() {} } as any
    const wsB = { send(raw: string) { secondRequestId = JSON.parse(raw).requestId } } as any
    const payload = { cwd: '/tmp', hook_event_name: 'PreToolUse' } as any
    const pendingA = dispatchHook('cancel-a', first.id, payload, wsA)
    const pendingB = dispatchHook('cancel-b', second.id, payload, wsB)
    cancelSessionLocalExecs('cancel-a')
    expect((await pendingA).outcome).toBe('cancelled')
    handleLocalHookResponse(secondRequestId, { stdout: 'ok', stderr: '', exitCode: 0, outcome: 'success', durationMs: 1 })
    expect((await pendingB).outcome).toBe('success')
  })

  it('passes every materialized proxy plugin root to Claude CLI', () => {
    const args = buildClaudeArgs({ cwd: '/repo' } as any, ['/sync/a', '/sync/b'])
    expect(args.filter(arg => arg === '--plugin-dir')).toHaveLength(2)
    expect(args).toContain('/sync/a')
    expect(args).toContain('/sync/b')
  })

  it('keeps the CLI workflow scheduler enabled for proxy plugins', () => {
    const env = buildSessionEnv('workflow-test', 'tester')
    expect(env.CLAUDE_CODE_DISABLE_WORKFLOWS).toBeUndefined()
  })
})
