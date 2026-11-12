// Agent engine state for the model selector: the user's main agent, the GPT
// list the server's codex reports and the shared ChatGPT account state.
import { signal } from '@preact/signals'
import type { AgentEngine, AgentPreferencesPayload, CodexAccountInfo, CodexModelsPayload, EngineAuthState } from '../../shared/types'

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

/** Which engines the server is signed in to; null until the server says. */
export const engineAuth = signal<EngineAuthState | null>(null)

export function receiveAgentPreferences(data: AgentPreferencesPayload): void {
  if (data.defaultEngine === 'claude' || data.defaultEngine === 'codex') defaultEngine.value = data.defaultEngine
  if (data.auth?.claude && data.auth?.codex) engineAuth.value = data.auth
}

const ENGINE_NAME: Record<AgentEngine, string> = { claude: 'Claude', codex: 'ChatGPT' }

/** Why chats on `engine` cannot run on this server, or null when they can
 *  (or the state is not known yet — nothing is disabled on a guess). */
export function engineBlock(engine: AgentEngine, auth: EngineAuthState | null, catalog: CodexModelsPayload | null): string | null {
  if (engine === 'codex' && catalog?.unavailable) return `ChatGPT is unavailable: ${catalog.unavailable}`
  // The live push (agent:preferences) is newer than the last GPT list reply.
  const state = auth?.[engine]
  if (state?.loggedIn === false) return state.reason ?? `${ENGINE_NAME[engine]} is not signed in on this Bridge server.`
  if (state?.loggedIn === true) return null
  if (engine === 'codex' && catalog?.account && !catalog.account.loggedIn) return codexNotice(catalog)
  return null
}

/** Neither engine can run chats: one line pointing at the dashboard. */
export const NO_ENGINE_NOTICE = 'Neither Claude nor ChatGPT is signed in on this Bridge server. Sign in from the Bridge dashboard.'

/** The Bridge dashboard of the server behind `serverUrl` (ws(s)://host/ws/session). */
export function dashboardUrl(serverUrl: string): string {
  const base = serverUrl.trim().replace(/^ws(s?):\/\//i, 'http$1://').replace(/\/ws\/session\/?$/, '').replace(/\/+$/, '')
  return `${base}/ui`
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
