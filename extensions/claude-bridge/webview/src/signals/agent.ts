// Agent engine state for the model selector: the user's main agent, the GPT
// list the server's codex reports and the shared ChatGPT account state.
import { signal } from '@preact/signals'
import type { AgentEngine, CodexAccountInfo, CodexModelsPayload } from '../../shared/types'

/** Main agent for new chats; the server stores it per token (agent:preferences). */
export const defaultEngine = signal<AgentEngine>('claude')

/** Last GPT list + account reply; null until the selector first asks. */
export const codexCatalog = signal<CodexModelsPayload | null>(null)

/** A GPT list request is in flight. */
export const codexCatalogLoading = signal(false)

export function receiveCodexModels(payload: CodexModelsPayload): void {
  codexCatalog.value = { ...payload, models: Array.isArray(payload.models) ? payload.models : [] }
  codexCatalogLoading.value = false
}

/** A codex chat was refused for a missing ChatGPT sign-in: show it in the selector. */
export function receiveCodexAuth(account: CodexAccountInfo): void {
  codexCatalog.value = { ...(codexCatalog.value ?? { models: [] }), account }
}

export function receiveAgentPreferences(data: { defaultEngine?: AgentEngine }): void {
  if (data.defaultEngine === 'claude' || data.defaultEngine === 'codex') defaultEngine.value = data.defaultEngine
}

/** The server has no working codex: GPT chats cannot be opened at all. */
export function codexUnavailable(catalog: CodexModelsPayload | null): boolean {
  return Boolean(catalog?.unavailable)
}

/** One line telling the user why GPT chats cannot answer, or null when they can. */
export function codexNotice(catalog: CodexModelsPayload | null): string | null {
  if (!catalog) return null
  if (catalog.unavailable) return `ChatGPT is unavailable: ${catalog.unavailable}`
  if (catalog.error) return catalog.error.message
  const account = catalog.account
  if (account && !account.loggedIn) {
    return account.error?.message ?? 'ChatGPT is not signed in. An administrator signs in from the Bridge dashboard.'
  }
  return null
}
