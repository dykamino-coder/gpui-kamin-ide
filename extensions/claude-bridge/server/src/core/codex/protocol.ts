// ============================================================================
// Codex app-server protocol — the subset Bridge speaks.
//
// Hand-copied from `codex app-server generate-ts --experimental` of
// codex-cli 0.167.2 (the version the Bridge image pins). Only the shapes Bridge
// reads or sends are mirrored; unknown fields are tolerated everywhere because
// the API is experimental and grows between releases. When the pinned version
// changes, regenerate the types and re-check these definitions and the
// recorded fixtures under ./fixtures.
// ============================================================================

export const CODEX_PROTOCOL_VERSION = '0.167.2'

export type RequestId = string | number

export interface ClientInfo {
  name: string
  title: string | null
  version: string
}

export interface InitializeCapabilities {
  experimentalApi: boolean
  requestAttestation: boolean
  optOutNotificationMethods?: string[] | null
}

export interface InitializeParams {
  clientInfo: ClientInfo
  capabilities: InitializeCapabilities | null
}

export interface InitializeResponse {
  userAgent?: string
  codexHome?: string
  platformFamily?: string
  platformOs?: string
}

export type TextElement = Record<string, unknown>
export type UserInput = { type: 'text'; text: string; text_elements: TextElement[] }

export type AskForApproval = 'untrusted' | 'on-request' | 'never'
export type SandboxMode = 'read-only' | 'workspace-write' | 'danger-full-access'
/** Open string: the catalog decides which efforts a model accepts. */
export type ReasoningEffort = string

export interface DynamicToolSpec {
  type: 'function'
  name: string
  description: string
  inputSchema: unknown
}

export interface ThreadStartParams {
  model?: string | null
  cwd?: string | null
  approvalPolicy?: AskForApproval | null
  sandbox?: SandboxMode | null
  baseInstructions?: string | null
  developerInstructions?: string | null
  dynamicTools?: DynamicToolSpec[] | null
  config?: Record<string, unknown> | null
}

export interface ThreadResumeParams {
  threadId: string
  model?: string | null
  cwd?: string | null
  approvalPolicy?: AskForApproval | null
  sandbox?: SandboxMode | null
  excludeTurns?: boolean
}

export interface Thread {
  id: string
  model: string | null
  cwd: string
  path: string | null
  [key: string]: unknown
}

export interface ThreadStartResponse {
  thread: Thread
  model: string
  modelProvider: string
  reasoningEffort: ReasoningEffort | null
  [key: string]: unknown
}

export type ThreadResumeResponse = ThreadStartResponse

export interface TurnStartParams {
  threadId: string
  input: UserInput[]
  clientUserMessageId?: string | null
  model?: string | null
  effort?: ReasoningEffort | null
}

export interface TurnSteerParams {
  threadId: string
  input: UserInput[]
  expectedTurnId: string
  clientUserMessageId?: string | null
}

export interface TurnInterruptParams {
  threadId: string
  turnId: string
}

export type TurnStatus = 'completed' | 'interrupted' | 'failed' | 'inProgress'

export interface TurnError {
  message: string
  codexErrorInfo: unknown
  additionalDetails: string | null
}

export interface Turn {
  id: string
  status: TurnStatus
  error: TurnError | null
  durationMs: number | null
  [key: string]: unknown
}

export interface TurnStartResponse {
  turn: Turn
}

export interface TurnSteerResponse {
  turnId: string
}

export interface TokenUsageBreakdown {
  totalTokens: number
  inputTokens: number
  cachedInputTokens: number
  cacheWriteInputTokens: number
  outputTokens: number
  reasoningOutputTokens: number
}

export interface ThreadTokenUsage {
  total: TokenUsageBreakdown
  last: TokenUsageBreakdown
  modelContextWindow: number | null
}

export type McpToolCallStatus = 'inProgress' | 'completed' | 'failed'
export type DynamicToolCallStatus = 'inProgress' | 'completed' | 'failed'

/** Items Bridge renders; any other `type` is surfaced as unsupported. */
export type ThreadItem =
  | { type: 'userMessage'; id: string; clientId: string | null; content: Array<{ type: string; text?: string }> }
  | { type: 'agentMessage'; id: string; text: string; phase: string | null }
  | { type: 'reasoning'; id: string; summary: string[]; content: string[] }
  | {
      type: 'mcpToolCall'
      id: string
      server: string
      tool: string
      status: McpToolCallStatus
      arguments: unknown
      result: { content: unknown[]; structuredContent: unknown } | null
      error: { message: string } | null
      durationMs: number | null
    }
  | {
      type: 'dynamicToolCall'
      id: string
      namespace: string | null
      tool: string
      arguments: unknown
      status: DynamicToolCallStatus
      contentItems: Array<{ type: string; text?: string; imageUrl?: string }> | null
      success: boolean | null
      durationMs: number | null
    }
  | {
      type: 'commandExecution'
      id: string
      command: string
      status: string
      aggregatedOutput: string | null
      exitCode: number | null
      durationMs: number | null
    }
  | { type: 'fileChange'; id: string; changes: unknown[]; status: string }
  | { type: 'contextCompaction'; id: string }
  | { type: string; id?: string; [key: string]: unknown }

export interface ItemNotification {
  item: ThreadItem
  threadId: string
  turnId: string
  startedAtMs?: number
  completedAtMs?: number
}

/** Server→client notifications Bridge consumes, keyed by method. */
export interface CodexNotificationMap {
  'thread/started': { thread: Thread }
  'turn/started': { threadId: string; turn: Turn }
  'turn/completed': { threadId: string; turn: Turn }
  'item/started': ItemNotification
  'item/completed': ItemNotification
  'item/agentMessage/delta': { threadId: string; turnId: string; itemId: string; delta: string }
  'item/reasoning/summaryTextDelta': {
    threadId: string
    turnId: string
    itemId: string
    delta: string
    summaryIndex: number
  }
  'item/reasoning/textDelta': { threadId: string; turnId: string; itemId: string; delta: string; contentIndex: number }
  'thread/tokenUsage/updated': { threadId: string; turnId: string; tokenUsage: ThreadTokenUsage }
  'thread/compacted': { threadId: string; turnId: string }
  'model/rerouted': { threadId: string; turnId: string; fromModel: string; toModel: string; reason: string }
  error: { error: TurnError; willRetry: boolean; threadId: string; turnId: string }
  warning: { threadId: string | null; message: string }
}

export type CodexNotificationMethod = keyof CodexNotificationMap

/** Server→client requests that must be answered (approval, dynamic tools…). */
export type CodexServerRequestMethod =
  | 'item/commandExecution/requestApproval'
  | 'item/fileChange/requestApproval'
  | 'item/permissions/requestApproval'
  | 'item/tool/requestUserInput'
  | 'item/tool/call'
  | 'mcpServer/elicitation/request'
  | 'account/chatgptAuthTokens/refresh'
  | 'attestation/generate'
  | 'currentTime/read'
  | 'applyPatchApproval'
  | 'execCommandApproval'
