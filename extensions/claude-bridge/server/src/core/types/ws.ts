// ============================================================================
// WebSocket protocol messages (multiplexed on /ws/session)
// ============================================================================

import type { JsonlEntry } from '../../shared/jsonl-types'
import type { ToolDefinition, TreeNode } from './pty'

// -- Electron → Server --

export interface WsMsgSessionCreate {
  type: 'session:create'
  token: string
  /** Optional working directory for Claude CLI */
  cwd?: string
  /** Initial terminal size */
  cols?: number
  rows?: number
  /** External MCP tool schemas to register before PTY starts */
  externalTools?: ToolDefinition[]
  /** Client streaming-protocol version. >=1 → server sends incremental
   *  `streaming:delta` frames between boundary snapshots instead of a full
   *  snapshot every tick. Absent/0 → legacy full-snapshot streaming. */
  protocolVersion?: number
  /** The user's standing instructions for this token. APPENDED to the technical
   *  system prompt (buildSystemPrompt) — never replaces it. Rides the session
   *  message so nothing has to be stored or synced server-side. */
  basePrompt?: string
  /** Absolute dir where THIS client mirrors the full transcript (one
   *  `<conversationId>.jsonl` per conversation). Embedded in the CLI system
   *  prompt so Claude can read its own history via the user-tools file MCP,
   *  which runs on the same (client) machine. */
  transcriptMirrorDir?: string
}

export interface WsMsgSessionInput {
  type: 'session:input'
  data: string
}

/** User pressed Stop: abort the in-flight upstream stream AND SIGINT the CLI. */
export interface WsMsgSessionInterrupt {
  type: 'session:interrupt'
}

/**
 * Submit a full user message: wraps in bracketed paste, waits for the CLI
 * to echo it back in its input buffer, then sends exactly one CR. Replaces
 * the fragile "paste + timed retries of \r" dance on the renderer side.
 */
export interface WsMsgSessionSubmitText {
  type: 'session:submitText'
  data: string
}

export interface WsMsgSessionResize {
  type: 'session:resize'
  cols: number
  rows: number
}

export interface WsMsgMcpResponse {
  type: 'mcp:response'
  requestId: string
  result: unknown
}

export interface WsMsgMcpDenied {
  type: 'mcp:denied'
  requestId: string
  reason: string
}

/** Electron responds to an elicitation request (AskUserQuestion, ExitPlanMode, etc.) */
export interface WsMsgElicitationResponse {
  type: 'elicitation:response'
  requestId: string
  action: 'accept' | 'deny' | 'dismiss'
  content?: Record<string, unknown>
}

/** Client explicitly ends the session (Disconnect button / tab close).
 *  Unlike a bare WS drop — which only DETACHES the session for a grace
 *  window awaiting reattach — this destroys it immediately. */
export interface WsMsgSessionEnd {
  type: 'session:end'
}

/** Electron requests session resume */
export interface WsMsgSessionResume {
  type: 'session:resume'
  token: string
  conversationId: string
  cwd?: string
  cols?: number
  rows?: number
  /** External MCP tool schemas to register before PTY starts */
  externalTools?: ToolDefinition[]
  /** Client streaming-protocol version — see WsMsgSessionCreate. */
  protocolVersion?: number
  /** See WsMsgSessionCreate. A resume respawns the CLI, so it must carry the
   *  prompt too — otherwise the instructions would only ever reach brand-new
   *  sessions and silently vanish on every reconnect. */
  basePrompt?: string
  /** See WsMsgSessionCreate. */
  transcriptMirrorDir?: string
}

/** Electron requests effort level change (requires PTY restart) */
export interface WsMsgSessionChangeEffort {
  type: 'session:change-effort'
  effort: string
}

/** Electron requests model change (requires PTY restart) */
export interface WsMsgSessionChangeModel {
  type: 'session:change-model'
  model: string
}

/** Electron requests deletion of session data (settingsDir + JSONL) */
export interface WsMsgSessionDeleteData {
  type: 'session:delete-data'
  conversationId: string
}

/** Electron registers external MCP tools with the server session */
export interface WsMsgRegisterExternalTools {
  type: 'mcp:register-external-tools'
  tools: ToolDefinition[]
}

/** Electron requests raw JSONL file download */
export interface WsMsgJsonlDownloadRequest {
  type: 'jsonl:download-request'
}

/** Incremental catch-up: the client mirrors the transcript locally and asks for
 *  everything it does not already hold. The server answers with either a
 *  resumed stream or an order to start over — never ambiguously, because
 *  extending a mirror that is no longer a prefix would fabricate history. */
export interface WsMsgJsonlSyncRequest {
  type: 'jsonl:sync-request'
  /** File fingerprint the client's mirror was built from. */
  sinceHead?: string
  /** How many bytes of the transcript the client already holds. */
  sincePos?: number
  /** Start offset of the LAST record the client holds, with its uuid. The server
   *  reads that record and compares: same offset, same record, or no resume.
   *  Without this a repair that shifts offsets by exactly one record length
   *  lands on another record's boundary and passes unnoticed. */
  lastPos?: number
  lastUuid?: string
}

/** Electron asks the server to load ONE archived compact segment whole, by
 *  TIMESTAMP range — the "view an old conversation" path. Served from the
 *  authoritative transcript file (the client's local mirror is a bounded tail
 *  and may not hold this range). Empty `fromTs` = from the start; empty `toTs` =
 *  to the newest. */
export interface WsMsgJsonlSegmentRequest {
  type: 'jsonl:segment-request'
  fromTs: string
  toTs: string
}

/** Electron's reply to a server-initiated `hook:execute` — local-host shell
 *  finished running the hook command, returns stdout/stderr/exit. */
export interface WsMsgHookResponse {
  type: 'hook:response'
  requestId: string
  result: {
    stdout: string
    stderr: string
    exitCode: number
    outcome: 'success' | 'error' | 'timeout' | 'cancelled'
    jsonOutput?: Record<string, unknown>
    durationMs: number
  }
}

export type ElectronToServerMsg =
  | WsMsgSessionCreate
  | WsMsgSessionEnd
  | WsMsgSessionInput
  | WsMsgSessionInterrupt
  | WsMsgSessionSubmitText
  | WsMsgSessionResize
  | WsMsgMcpResponse
  | WsMsgMcpDenied
  | WsMsgElicitationResponse
  | WsMsgSessionResume
  | WsMsgSessionChangeEffort
  | WsMsgSessionChangeModel
  | WsMsgSessionDeleteData
  | WsMsgRegisterExternalTools
  | WsMsgJsonlDownloadRequest
  | WsMsgJsonlSyncRequest
  | WsMsgJsonlSegmentRequest
  | WsMsgHookResponse

// -- Server → Electron --

export interface WsMsgSessionCreated {
  type: 'session:created'
  sessionId: string
  /** Resume target conversation was not found — a fresh session started instead. */
  resumeNotFound?: boolean
}

export interface WsMsgSessionOutput {
  type: 'session:output'
  data: string
}

export interface WsMsgSessionExit {
  type: 'session:exit'
  code: number
  sessionId: string
}

export interface WsMsgSessionError {
  type: 'session:error'
  error: string
}

export interface WsMsgMcpCall {
  type: 'mcp:call'
  requestId: string
  toolName: string
  input: Record<string, unknown>
}

// JSONL streaming messages (Server → Electron)
export interface WsMsgJsonlEntries {
  type: 'jsonl:entries'
  entries: JsonlEntry[]
}

export interface WsMsgJsonlStatus {
  type: 'jsonl:status'
  status: 'searching' | 'watching' | 'error'
  filePath?: string
  error?: string
  compacted?: boolean
  replayComplete?: boolean
  fileHead?: string
  fileSize?: number
  replayProgress?: { sent: number; total: number }
  /** Authoritative compact-segment index, sent with replayComplete. */
  segmentIndex?: { boundaries: { ts: string }[]; counts: number[] }
}

/** Server's reply to `jsonl:segment-request` — one archived segment's records,
 *  read from the authoritative transcript by timestamp range. */
export interface WsMsgJsonlSegmentResponse {
  type: 'jsonl:segment-response'
  fromTs: string
  toTs: string
  records: JsonlEntry[]
}

/** Server sends elicitation request to Electron (show widget) */
export interface WsMsgElicitationRequest {
  type: 'elicitation:request'
  requestId: string
  toolName: string
  message: string
  requestedSchema?: Record<string, unknown>
}

/** Server notifies Electron about agent spawn */
export interface WsMsgAgentSpawned {
  type: 'session:agent-spawned'
  parentSessionId: string
  childSessionId: string
  agentName: string
  description: string
}

/** Server sends full session tree update */
export interface WsMsgTreeUpdate {
  type: 'session:tree-update'
  tree: TreeNode[]
}

/** Server notifies Electron that session was restarted (e.g., effort or model change) */
export interface WsMsgSessionRestarted {
  type: 'session:restarted'
  sessionId: string
  effort?: string
  model?: string
}

/** Server sends CLI activity status — either from OSC escape sequences
 *  (spinner/idle, the flappy fallback) OR from a deterministic source:
 *  bridge lifecycle hooks (UserPromptSubmit/Stop/Notification/…) and the
 *  Ctrl+C interrupt push. The `hookDriven` flag tells the client this beats
 *  the OSC heuristic; the extra flags carry the richer hook state. */
export interface WsMsgSessionActivity {
  type: 'session:activity'
  sessionId: string
  /** Raw OSC text including spinner icon, e.g. "⠒ Thinking..." or "✳ Claude Code" */
  rawTitle: string
  /** Whether CLI is actively working (spinner detected) vs idle (✳ or prompt) */
  isWorking: boolean
  /** Deterministic (hook/interrupt) source — overrides the OSC fallback client-side. */
  hookDriven?: boolean
  /** CLI is idle at the prompt, ready for input (Stop/SessionStart hooks). */
  promptReady?: boolean
  /** CLI is blocked waiting for the user (Notification hook: permission/answer). */
  waiting?: boolean
  /** Stop hook: the last assistant message text. */
  lastMessage?: string
  /** Notification hook: the message the CLI is waiting on. */
  notificationMessage?: string
}

/** Server sends raw JSONL file content for download */
export interface WsMsgJsonlDownloadResponse {
  type: 'jsonl:download-response'
  content: string | null
  fileName: string | null
  error?: string
}

export type ServerToElectronMsg =
  | WsMsgSessionCreated
  | WsMsgSessionOutput
  | WsMsgSessionExit
  | WsMsgSessionError
  | WsMsgMcpCall
  | WsMsgJsonlEntries
  | WsMsgJsonlStatus
  | WsMsgElicitationRequest
  | WsMsgAgentSpawned
  | WsMsgTreeUpdate
  | WsMsgSessionRestarted
  | WsMsgSessionActivity
  | WsMsgJsonlDownloadResponse
  | WsMsgJsonlSegmentResponse
