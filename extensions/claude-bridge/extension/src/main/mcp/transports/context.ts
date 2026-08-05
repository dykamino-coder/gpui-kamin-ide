// Shared context interface passed to each transport module.
//
// Transport files (stdio / http / sse-legacy / ws) are standalone modules
// that read/write manager state + fire UI notifications. To avoid a
// circular import on the McpServerManager class itself, they take a
// structural context that exposes the few manager internals they need.

import type { BrowserWindow } from '@kaminide/host-compat'
import type { ChildProcess } from 'child_process'
import type WebSocket from 'ws'
import type {
  ExternalMcpServerConfig,
  McpResourceInfo,
  McpResourceTemplateInfo,
  McpPromptInfo,
} from '../../../shared/types'
import type { McpTestLogEntry } from '../test-log'

/** The full per-server runtime state kept by the manager. Transports mutate
 *  this (tool lists, pending requests, transport handles) inline. */
export interface McpServerState {
  config: ExternalMcpServerConfig
  status: 'disconnected' | 'connecting' | 'connected' | 'error'
  tools: string[]
  toolSchemas: Map<string, { name: string; description: string; inputSchema: Record<string, unknown> }>
  resources: McpResourceInfo[]
  resourceTemplates: McpResourceTemplateInfo[]
  prompts: McpPromptInfo[]
  /** Server-declared capabilities, as returned by `initialize`. */
  capabilities: Record<string, unknown>
  error?: string
  // stdio
  process?: ChildProcess
  // websocket
  ws?: WebSocket
  /** Legacy SSE transport: the message endpoint URL (from `endpoint` event)
   *  we POST JSON-RPC requests to. */
  sseMessageEndpoint?: string
  /** AbortController for the long-lived GET /sse stream — closed on disconnect. */
  sseAbort?: AbortController
  pendingRequests: Map<number, {
    resolve: (v: unknown) => void
    reject: (e: Error) => void
    /** Tracked so disconnect can cancel pending timeouts without leaking writes
     *  into a cleared pendingRequests map. */
    timer?: ReturnType<typeof setTimeout>
  }>
  nextRequestId: number
  inputBuffer: string
  /** Ring buffer of recent connect/test events. */
  testLog: McpTestLogEntry[]
  /** Auto-recovery bookkeeping. `reconnectAttempts` resets to 0 on a
   *  successful connect; capped at 10 to stop infinite churn against a
   *  permanently broken server. `reconnectTimer` is the pending
   *  setTimeout handle so `disconnectServer` can cancel it. The
   *  `expectedDisconnect` flag is raised by the manager BEFORE it kills
   *  a transport on the user's behalf so the exit handler doesn't
   *  schedule a reconnect we don't want. */
  reconnectAttempts?: number
  /** When the current connection came up. A connect that dies seconds later is
   *  not a success and must not refill the retry budget — see scheduleReconnect. */
  connectedAt?: number
  reconnectTimer?: ReturnType<typeof setTimeout>
  expectedDisconnect?: boolean
  /** Диагноз перманентной поломки процесса (MODULE_NOT_FOUND: файл сервера
   *  удалён/переехал — плагин снесли, а запись в ~/.claude.json осталась).
   *  Реконнект бессмыслен — exit-хендлер его пропускает и ставит error.
   *  Сбрасывается на явном connect (юзер мог починить путь). */
  permanentFailure?: string
}

/** Surface of the manager that transports need to call back into. */
export interface TransportContext {
  window: BrowserWindow
  servers: Map<string, McpServerState>
  appendLog(id: string, level: McpTestLogEntry['level'], msg: string): void
  notifyChanged(): void
  onToolsChanged: (() => void) | null
  /** Central dispatcher for server-initiated messages (requests / notifications /
   *  responses to our requests). Transports call this for every frame they
   *  parse off the wire. */
  handleServerMessage(
    serverId: string,
    msg: any,
    sendResponse: (response: Record<string, unknown>) => void,
  ): void
  /** OAuth-aware header builder for HTTP/SSE transports. */
  buildHttpHeaders(id: string): Promise<Record<string, string>>
  /** Attempt a refresh token grant. Returns true if new tokens were saved. */
  tryRefreshTokens(id: string): Promise<boolean>
  /** Persist discovered OAuth metadata back into the config (on 401 challenge). */
  saveConfigs(): void
  /** Only used by HTTP 401 auto-discovery flow. */
  setDiscoveredOAuthMetadata(id: string, asUrl: string, scopes?: string[]): void
  /** Called by transports when the connection drops without us asking
   *  for it (child process crashed, socket aborted, etc.). The manager
   *  schedules a reconnect with exponential backoff if the server is
   *  still enabled. Skipped silently when `expectedDisconnect` was set
   *  by the manager beforehand. */
  onUnexpectedDisconnect?: (id: string, reason: string) => void
}
