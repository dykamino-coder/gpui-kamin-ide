import { beforeEach, describe, expect, it } from 'vitest'
import {
  codexCatalog,
  codexCatalogLoading,
  codexNotice,
  codexUnavailable,
  dashboardUrl,
  defaultEngine,
  engineAuth,
  engineBlock,
  receiveAgentPreferences,
  receiveCodexAuth,
  receiveCodexModels,
} from './agent'

const gpt = { id: 'gpt-5.5', model: 'gpt-5.5', displayName: 'GPT-5.5', description: '', isDefault: true, defaultReasoningEffort: null, reasoningEfforts: [] }

describe('agent engine signals', () => {
  beforeEach(() => {
    codexCatalog.value = null
    codexCatalogLoading.value = false
    defaultEngine.value = 'claude'
  })

  it('stores the GPT list and ends the loading state', () => {
    codexCatalogLoading.value = true
    receiveCodexModels({ models: [gpt], account: { loggedIn: true, email: 'synthetic@example.test' } })
    expect(codexCatalog.value?.models).toEqual([gpt])
    expect(codexCatalogLoading.value).toBe(false)
    expect(codexNotice(codexCatalog.value)).toBeNull()
  })

  it('keeps the list when a chat reports the sign-in gate', () => {
    receiveCodexModels({ models: [gpt], account: { loggedIn: true } })
    receiveCodexAuth({ loggedIn: false })
    expect(codexCatalog.value?.models).toEqual([gpt])
    expect(codexNotice(codexCatalog.value)).toMatch(/not signed in.*dashboard/)
  })

  it('shows the plain-language transport error from the server', () => {
    receiveCodexModels({ models: [], error: { kind: 'proxy-auth', message: 'The proxy rejected the credentials (HTTP 407).' } })
    expect(codexNotice(codexCatalog.value)).toBe('The proxy rejected the credentials (HTTP 407).')
  })

  it('takes only a known main agent', () => {
    receiveAgentPreferences({ defaultEngine: 'codex' })
    expect(defaultEngine.value).toBe('codex')
    receiveAgentPreferences({ defaultEngine: 'gemini' as never })
    receiveAgentPreferences({})
    expect(defaultEngine.value).toBe('codex')
  })

  it('marks the server without codex as unavailable with its reason', () => {
    receiveCodexModels({ models: [], unavailable: 'codex is not installed on this Bridge server' })
    expect(codexUnavailable(codexCatalog.value)).toBe(true)
    expect(codexNotice(codexCatalog.value)).toBe('ChatGPT is unavailable: codex is not installed on this Bridge server')
    receiveCodexModels({ models: [gpt] })
    expect(codexUnavailable(codexCatalog.value)).toBe(false)
  })
})
describe('engine sign-in state', () => {
  const out = { loggedIn: false, reason: 'X is signed out.' }
  const on = { loggedIn: true, reason: null }
  const unknown = { loggedIn: null, reason: null }

  it('keeps the pushed state and blocks only a signed-out engine', () => {
    engineAuth.value = null
    receiveAgentPreferences({ auth: { claude: out, codex: on } })
    expect(engineAuth.value).toEqual({ claude: out, codex: on })
    expect(engineBlock('claude', engineAuth.value, null)).toBe('X is signed out.')
    expect(engineBlock('codex', engineAuth.value, null)).toBeNull()
  })

  it('never blocks on an unknown state, and the live push beats an older GPT list reply', () => {
    const stale = { models: [gpt], account: { loggedIn: false } }
    expect(engineBlock('claude', { claude: unknown, codex: unknown }, null)).toBeNull()
    expect(engineBlock('codex', { claude: unknown, codex: unknown }, stale)).toMatch(/not signed in/)
    expect(engineBlock('codex', { claude: on, codex: on }, stale)).toBeNull()
    expect(engineBlock('codex', { claude: on, codex: on }, { models: [], unavailable: 'no codex' })).toBe('ChatGPT is unavailable: no codex')
  })

  it('points at the dashboard of the connected server', () => {
    expect(dashboardUrl('ws://localhost:3456/ws/session')).toBe('http://localhost:3456/ui')
    expect(dashboardUrl('wss://bridge.example.test')).toBe('https://bridge.example.test/ui')
  })
})
