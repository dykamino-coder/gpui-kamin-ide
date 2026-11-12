import { beforeEach, describe, expect, it } from 'vitest'
import {
  codexCatalog,
  codexCatalogLoading,
  codexNotice,
  defaultEngine,
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
})
