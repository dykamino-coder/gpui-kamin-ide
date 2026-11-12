import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { describe, expect, it, vi } from 'vitest'
import type { CodexRuntime } from '../codex/codex-runtime'
import {
  chooseEngineForNewChat,
  ClaudeAuthMonitor,
  codexEngineAuth,
  engineAuthState,
  mainAgentRefusal,
  type ClaudeAuthResult,
} from './engine-auth'

function claude(loggedIn: boolean | 'fails') {
  const probe = vi.fn(async (): Promise<ClaudeAuthResult> => {
    if (loggedIn === 'fails') throw new Error('claude auth status timed out')
    return { loggedIn }
  })
  return { monitor: new ClaudeAuthMonitor({ probe }), probe }
}

function codex(
  state: { available?: boolean; loggedIn?: boolean; known?: boolean } = {},
): CodexRuntime & { accountStatus: ReturnType<typeof vi.fn> } {
  const account = {
    loggedIn: state.loggedIn ?? true,
    email: null,
    planType: null,
    accountType: null,
    pendingLogin: null,
    error: null,
  }
  return {
    availability: async () =>
      state.available === false
        ? { available: false, version: null, reason: 'codex is not installed on this Bridge server' }
        : { available: true, version: '0.160.1', reason: null },
    accountStatus: vi.fn(async () => account),
    knownAccount: () => (state.known === false ? null : account),
  } as never
}

describe('Claude sign-in monitor', () => {
  it('caches the CLI answer and announces only changes', async () => {
    let loggedIn = true
    let clock = 0
    const onChange = vi.fn()
    const probe = vi.fn(async () => ({ loggedIn }))
    const monitor = new ClaudeAuthMonitor({ probe, onChange, now: () => clock, ttlMs: 1000 })
    expect(monitor.loggedIn()).toBeNull()
    await monitor.status()
    await monitor.status()
    expect(probe).toHaveBeenCalledTimes(1)
    expect(onChange.mock.calls).toEqual([[true]])

    loggedIn = false
    clock = 2000
    await monitor.status()
    expect(monitor.loggedIn()).toBe(false)
    monitor.record({ loggedIn: false })
    expect(onChange.mock.calls).toEqual([[true], [false]])
  })

  it('treats a CLI that does not answer as unknown, not signed out', async () => {
    const { monitor } = claude('fails')
    await expect(monitor.status()).rejects.toThrow('timed out')
    expect(await monitor.loggedInWithin(50)).toBeNull()
    expect(monitor.loggedIn()).toBeNull()
  })

  it('answers a known state at once and refreshes it in the background', async () => {
    const { monitor, probe } = claude(true)
    monitor.record({ loggedIn: false })
    expect(await monitor.loggedInWithin(1)).toBe(false)
    expect(probe).toHaveBeenCalledTimes(0) // recorded just now: still fresh
  })

  it('re-reads the sign-in when the credentials file changes', async () => {
    const home = fs.mkdtempSync(path.join(os.tmpdir(), 'claude-auth-watch-'))
    fs.mkdirSync(path.join(home, '.claude'))
    const { monitor, probe } = claude(true)
    try {
      monitor.watch(home)
      fs.writeFileSync(path.join(home, '.claude', '.credentials.json'), '{}')
      await vi.waitFor(() => expect(probe).toHaveBeenCalled(), { timeout: 5000, interval: 50 })
      expect(monitor.loggedIn()).toBe(true)
    } finally {
      monitor.unwatch()
      fs.rmSync(home, { recursive: true, force: true })
    }
  })
})

describe('ChatGPT sign-in state', () => {
  it('reports a missing codex binary as signed out with the reason', async () => {
    expect(await codexEngineAuth(codex({ available: false }), true)).toEqual({
      loggedIn: false,
      reason: 'ChatGPT is unavailable: codex is not installed on this Bridge server',
    })
  })

  it('never starts an app-server for a passive read', async () => {
    const runtime = codex({ known: false })
    expect(await codexEngineAuth(runtime, false)).toEqual({ loggedIn: null, reason: null })
    expect(runtime.accountStatus).not.toHaveBeenCalled()
  })
})

// The four sign-in combinations, for both requested engines.
describe.each([
  { claudeIn: true, codexIn: true },
  { claudeIn: true, codexIn: false },
  { claudeIn: false, codexIn: true },
  { claudeIn: false, codexIn: false },
])('Claude signed in: $claudeIn, ChatGPT signed in: $codexIn', ({ claudeIn, codexIn }) => {
  const deps = () => {
    const { monitor } = claude(claudeIn)
    monitor.record({ loggedIn: claudeIn })
    return { claude: monitor, runtime: codex({ loggedIn: codexIn }) }
  }

  it('reports both engines', async () => {
    const state = await engineAuthState(deps(), true)
    expect(state.claude.loggedIn).toBe(claudeIn)
    expect(state.codex.loggedIn).toBe(codexIn)
    expect(state.claude.reason).toBe(claudeIn ? null : 'Claude is not signed in on this Bridge server.')
    expect(state.codex.reason).toBe(codexIn ? null : 'ChatGPT is not signed in on this Bridge server.')
  })

  it.each(['claude', 'codex'] as const)(
    'opens a new %s chat on a signed-in engine when there is one',
    async (requested) => {
      const choice = await chooseEngineForNewChat(requested, deps())
      const signedIn = { claude: claudeIn, codex: codexIn }
      const other = requested === 'claude' ? 'codex' : 'claude'
      if (signedIn[requested] || !signedIn[other]) {
        expect(choice).toEqual({ engine: requested, notice: null })
      } else {
        expect(choice.engine).toBe(other)
        expect(choice.notice).toMatch(
          /is not signed in on this Bridge server\. This chat runs (Claude|ChatGPT) instead/,
        )
      }
    },
  )

  it.each(['claude', 'codex'] as const)('refuses %s as the main agent only when it is signed out', async (engine) => {
    const refusal = await mainAgentRefusal(engine, deps())
    const signedIn = engine === 'claude' ? claudeIn : codexIn
    expect(refusal === null).toBe(signedIn)
  })
})

describe('ChatGPT unreachable (proxy 407, untrusted CA)', () => {
  const unreachable = () => {
    const runtime = codex({ loggedIn: false })
    const status = {
      loggedIn: false,
      email: null,
      planType: null,
      accountType: null,
      pendingLogin: null,
      error: { kind: 'proxy-auth', message: 'The corporate proxy requires authentication (HTTP 407).', detail: '407' },
      checkFailed: true,
    }
    runtime.accountStatus.mockResolvedValue(status)
    const { monitor } = claude(true)
    monitor.record({ loggedIn: true })
    return { claude: monitor, runtime }
  }

  it('reports the sign-in as unknown with the reason, not as signed out', async () => {
    expect(await codexEngineAuth(unreachable().runtime, true)).toEqual({
      loggedIn: null,
      reason: 'The corporate proxy requires authentication (HTTP 407).',
    })
  })

  it('keeps a new GPT chat on ChatGPT instead of moving it to Claude', async () => {
    expect(await chooseEngineForNewChat('codex', unreachable())).toEqual({ engine: 'codex', notice: null })
  })
})
