import { describe, expect, it, vi } from 'vitest'
import { CodexRuntime, codexStatus, codexUnavailableReason, parseCodexVersion } from './codex-runtime'
import { codexStatusRow } from '../../ui/src/components/dashboard/codex-account-model'
import { classifyCodexError, type CodexErrorInfo } from './codex-errors'
import type { CodexNetworkReport } from './network-probe'

describe('codex availability', () => {
  it('parses the version line of codex --version', () => {
    expect(parseCodexVersion('codex-cli 0.160.1\n')).toBe('0.160.1')
    expect(parseCodexVersion('WARNING: alias\ncodex-cli 0.160.1-alpha.2')).toBe('0.160.1-alpha.2')
    expect(parseCodexVersion('codex dev build')).toBe('codex dev build')
    expect(parseCodexVersion('')).toBeNull()
  })

  it('explains a missing, non-executable or crashing binary in one line', () => {
    expect(codexUnavailableReason(Object.assign(new Error('spawn codex ENOENT'), { code: 'ENOENT' }))).toBe(
      'codex is not installed on this Bridge server',
    )
    expect(codexUnavailableReason(new Error('spawn /usr/bin/codex EACCES'))).toBe(
      'codex on this Bridge server is not executable',
    )
    expect(codexUnavailableReason(new Error('Command failed: codex --version\nGLIBC_2.38 not found'))).toBe(
      'codex does not start on this Bridge server: Command failed: codex --version',
    )
  })

  it('keeps a good probe and re-probes a failure after the retry window', async () => {
    vi.useFakeTimers()
    try {
      let installed = false
      const probeVersion = vi.fn(async () => {
        if (!installed) throw Object.assign(new Error('spawn codex ENOENT'), { code: 'ENOENT' })
        return 'codex-cli 0.160.1'
      })
      const runtime = new CodexRuntime({ home: '/tmp/bridge-codex-test', probeVersion, env: {} })
      expect(await runtime.availability()).toMatchObject({ available: false })
      installed = true
      expect(await runtime.availability()).toMatchObject({ available: false })
      vi.advanceTimersByTime(31_000)
      expect(await runtime.availability()).toEqual({ available: true, version: '0.160.1', reason: null })
      vi.advanceTimersByTime(10 * 60_000)
      await runtime.availability()
      expect(probeVersion).toHaveBeenCalledTimes(2)
    } finally {
      vi.useRealTimers()
    }
  })

  it('does not hold the status page on a slow first probe', async () => {
    const slow = { availability: () => new Promise<never>(() => undefined) } as unknown as CodexRuntime
    expect(await codexStatus(() => slow, {}, 10)).toEqual({
      available: false,
      version: null,
      reason: 'checking codex…',
      pinned: null,
    })
  })

  it('shows the version, a pin mismatch or the reason on the status row', () => {
    expect(codexStatusRow({ available: true, version: '0.160.1', reason: null, pinned: '0.160.1' })).toEqual({
      text: '0.160.1',
      ok: true,
    })
    expect(codexStatusRow({ available: true, version: '0.161.0', reason: null, pinned: '0.160.1' })).toEqual({
      text: '0.161.0 (image pins 0.160.1)',
      ok: false,
    })
    expect(
      codexStatusRow({
        available: false,
        version: null,
        reason: 'codex is not installed on this Bridge server',
        pinned: null,
      }),
    ).toEqual({ text: 'unavailable — codex is not installed on this Bridge server', ok: false })
    expect(codexStatusRow(undefined)).toEqual({ text: '—', ok: true })
  })
})

function report(problem: CodexErrorInfo | null): CodexNetworkReport {
  return { checkedAt: 0, viaProxy: true, customCa: false, hosts: [], problem }
}

describe('codex account read that fails on the network', () => {
  const blocked = () => Promise.reject(new Error('workspace routing discovery failed'))

  it('names the proxy or TLS cause found by the probe and marks the state unknown', async () => {
    const probeNetwork = vi.fn(async () =>
      report({
        kind: 'proxy-auth',
        message: 'The corporate proxy requires authentication (HTTP 407).',
        detail: 'Proxy response (407) !== 200 when HTTP Tunneling',
      }),
    )
    const runtime = new CodexRuntime({ home: '/tmp/bridge-codex-test', env: {}, startHost: blocked, probeNetwork })
    const status = await runtime.accountStatus()
    expect(status.loggedIn).toBe(false)
    expect(status.checkFailed).toBe(true)
    expect(status.error).toEqual({
      kind: 'proxy-auth',
      message: 'The corporate proxy requires authentication (HTTP 407).',
      detail: 'workspace routing discovery failed; Proxy response (407) !== 200 when HTTP Tunneling',
    })
    expect(probeNetwork).toHaveBeenCalledTimes(1)
  })

  it("keeps codex's own words when the probe finds nothing", async () => {
    const runtime = new CodexRuntime({
      home: '/tmp/bridge-codex-test',
      env: {},
      startHost: blocked,
      probeNetwork: async () => report(null),
    })
    const status = await runtime.accountStatus()
    expect(status.checkFailed).toBe(true)
    expect(status.error?.detail).toBe('workspace routing discovery failed')
  })

  it('names who refused a bare 403 at sign-in, and shares one probe across asks', async () => {
    const denied = {
      kind: 'proxy-denied' as const,
      message: 'The proxy refuses auth.openai.com.',
      detail: 'auth.openai.com: HTTP 403 without cf-ray',
    }
    const probeNetwork = vi.fn(async () => report(denied))
    const runtime = new CodexRuntime({ home: '/tmp/bridge-codex-test', env: {}, startHost: blocked, probeNetwork })
    const info = await runtime.explainFailure(
      classifyCodexError('device code request failed with status 403 Forbidden'),
    )
    expect(info).toEqual({
      kind: 'proxy-denied',
      message: denied.message,
      detail: `device code request failed with status 403 Forbidden; ${denied.detail}`,
    })
    await runtime.networkReport()
    expect(probeNetwork).toHaveBeenCalledTimes(1)
    // A cause codex already names is not re-probed.
    expect((await runtime.explainFailure(classifyCodexError('407 Proxy Authentication Required'))).kind).toBe(
      'proxy-auth',
    )
    expect(probeNetwork).toHaveBeenCalledTimes(1)
  })
})
