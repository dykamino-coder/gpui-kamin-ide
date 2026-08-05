// ============================================================================
// PTY Session Types
// ============================================================================

import type { WebSocket as WS } from 'ws'
import type { PendingElicitation } from './elicitation'

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

export type SessionState = 'starting' | 'running' | 'exiting' | 'exited'

export interface McpLogEntry {
  ts: number
  method: string
  status: 'ok' | 'error'
  error?: string
  durationMs?: number
}

export interface PtySession {
  id: string
  pty: ReturnType<typeof import('node-pty').spawn>
  ws: WS
  /** Token-based user identity */
  userName: string
  tokenId: string
  /** Temp directory for this session's settings.json, .claude/, etc. */
  settingsDir: string
  /** User's working directory on their machine (from the client host) */
  cwd: string
  state: SessionState
  createdAt: Date
  lastActivityAt: Date
  /** Running count of MCP tool calls executed */
  mcpCallCount: number
  /** Running count of user inputs (Enter presses) */
  inputCount: number
  /** Dashboard live counters, recomputed from the JSONL on every replay and
   *  incremented on tail (replay-safe — reset at replay start). */
  userMessages: number
  assistantMessages: number
  /** Real context window at the last assistant turn (input + cache tokens). */
  contextTokens: number
  /** Cumulative API throughput: input + cache_read + cache_write + output. */
  totalTokens: number
  /** MCP HTTP request log for debugging (last N entries) */
  mcpLog: McpLogEntry[]
  /** Whether MCP initialize succeeded */
  mcpInitialized: boolean
  /** Last MCP error */
  mcpLastError: string | null
  /** Secret token for MCP HTTP auth (per-session) */
  mcpToken: string
  /** Accumulated PTY output lines (used by output debounce) */
  outputBuffer: string[]
  /** Running byte total of outputBuffer, maintained incrementally so the hot
   *  output path never re-sums the whole (≤5000-line) buffer on every chunk. */
  outputBufferBytes: number
  /** ms timestamp of the last client `session:resize`. A visible KaminIDE
   *  console heartbeats a resize every ~5s; silence ⇒ nobody's watching this
   *  console. Used by the opt-in idle-console PTY shrink. */
  lastResizeAt: number
  /** True while the PTY is held at the tiny idle-console size (opt-in shrink);
   *  cleared the moment a real client resize restores the viewport. */
  consoleShrunk: boolean
  /** JSONL file watcher for streaming session logs */
  jsonlWatcher?: {
    stop(): void
    getFilePath(): string | null
    replayAll?(): void
    scanSubagents?(): Promise<void>
    /** Send everything after a byte offset — the incremental catch-up for a
     *  client that already mirrors the transcript. Callers must verify the
     *  file's fingerprint first. */
    streamFrom?(offset: number, live: () => boolean): Promise<number>
  }
  /** Tool definitions registered for this session (dynamic list) */
  registeredTools: ToolDefinition[]
  /** External MCP resources/prompts flattened into the bridge MCP server. */
  registeredResources: ExternalResourceDefinition[]
  registeredResourceTemplates: ExternalResourceTemplateDefinition[]
  registeredPrompts: ExternalPromptDefinition[]
  /** Claude CLI's internal conversation ID (for --resume) */
  cliConversationId: string | null
  /** Прежние conversationId этой сессии (авто-compact меняет файл/id):
   *  reattach-finder матчит и по ним — клиент со старым id из metadata
   *  обязан находить живую сессию, а не резюмить скомпакченный файл. */
  conversationAliases?: string[]
  /** Resume asked for a conversation this server doesn't have — a FRESH
   *  session was started instead. The WS layer tells the client immediately
   *  (jsonl:status replayComplete + warning) so the chat doesn't sit in
   *  "Loading conversation…" until the first JSONL write. */
  resumeNotFound?: boolean
  /** Whether this session is a sub-agent (shorter idle timeout) */
  isSubAgent: boolean
  /** Parent session ID (for agent tree) */
  parentSessionId: string | null
  /** Child session IDs (agents/teammates) */
  childSessions: string[]
  /** Pending elicitation requests awaiting user response */
  pendingElicitations: Map<string, PendingElicitation>
  /** Session title extracted from CLI terminal title (OSC escape sequence) */
  sessionTitle: string | null
  /** Latest parsed CLI status-line text ("Shenaniganing… (58s · ↓ 219 tokens)") — optional */
  lastStatusText?: string
  /** Whether this session is being restarted (effort change) — suppresses exit handling */
  isRestarting: boolean
  /** Current effort level (preserved across restarts) */
  effort: string | null
  /** Current model (preserved across restarts) */
  model: string | null
  /** MITM streaming proxy (per-session port + CA). Optional — only present
   *  when streaming is enabled for this user's token. Closed in onExit. */
  streamingProxy?: { port: number; caCertPath: string; stop: () => Promise<void>; interrupt: () => void }
  /** Hash of the bearer token (per-token sync-dir key). Kept so restarts
   *  (model/effort change) re-generate settings.json with the same synced
   *  hooks/skills inputs as the original create. */
  bearerHash?: string
  /** Set when the client WS dropped without an explicit session:end. The CLI
   *  keeps running for a grace window awaiting reattach — killing it mid-turn
   *  is what used to corrupt the conversation JSONL (partial writes, dangling
   *  tool_use, double-writer on instant reconnect). */
  detachedAt?: Date | null
  /** Destroys the session when the detach grace window expires. */
  detachGraceTimer?: NodeJS.Timeout | null
  /** Negotiated streaming protocol (>=1 → emit `streaming:delta` between
   *  boundary snapshots; 0/undefined → legacy full-snapshot every tick).
   *  Refreshed on reattach so a resumed client gets the mode it just asked for. */
  streamProtocol?: number
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

export interface ToolDefinition {
  name: string
  description: string
  inputSchema: Record<string, unknown>
}

export interface ExternalResourceDefinition {
  uri: string
  name: string
  description?: string
  mimeType?: string
  serverId: string
  rawUri: string
}

export interface ExternalResourceTemplateDefinition {
  uriTemplate: string
  name: string
  description?: string
  mimeType?: string
  serverId: string
  rawUriTemplate: string
}

export interface ExternalPromptDefinition {
  name: string
  description?: string
  arguments?: unknown[]
  serverId: string
  rawName: string
}

// ---------------------------------------------------------------------------
// Session config for creation
// ---------------------------------------------------------------------------

export interface SessionConfig {
  /** Working directory for Claude CLI */
  cwd?: string
  /** Initial terminal dimensions */
  cols?: number
  rows?: number
  /** Additional Claude CLI flags */
  extraArgs?: string[]
  /** The user's standing instructions, APPENDED to the technical system prompt
   *  (never replacing it — see buildSystemPrompt). Client-supplied per session. */
  basePrompt?: string
  /** Resume a previous conversation */
  resumeConversationId?: string
  /** Mark as sub-agent session */
  isSubAgent?: boolean
  /** Parent session ID for agent tree */
  parentSessionId?: string
  /** Effort level for Claude CLI (low/medium/high) */
  effort?: string
  /** Model for Claude CLI (opus/sonnet/haiku) */
  model?: string
  /** Reuse existing settingsDir (for restart with --resume — keeps same slug so CLI finds JSONL) */
  reuseSettingsDir?: string
  /** Hash of bearer token (first 16 chars of sha256) — for per-token sync data lookup */
  bearerHash?: string
  /** Client streaming-protocol version negotiated at connect (>=1 → deltas). */
  protocolVersion?: number
  /** Absolute dir on the CLIENT machine where the full transcript is mirrored.
   *  Surfaced in the system prompt so Claude can read its own history (the
   *  user-tools file MCP runs on that same machine). */
  transcriptMirrorDir?: string
}

// ---------------------------------------------------------------------------
// Session tree (for sidebar)
// ---------------------------------------------------------------------------

export interface TreeNode {
  id: string
  type: 'session' | 'agent' | 'team' | 'teammate'
  label: string
  status: 'active' | 'idle' | 'busy' | 'done' | 'error' | 'exited'
  model?: string
  children: TreeNode[]
  parentId?: string
  folderName?: string
  cwd?: string
  /** Auto-generated session topic from CLI (e.g. "Fix auth bug") */
  sessionTitle?: string
}

/** Saved session for resume (stored in electron-store) */
export interface SavedSession {
  conversationId: string
  cwd: string
  label: string
  folderName: string
  model: string
  lastActivity: string
  messageCount: number
}

// ---------------------------------------------------------------------------
// Per-token sync data (VSIX bridge host → Bridge)
// ---------------------------------------------------------------------------

export interface SyncPluginData {
  id: string
  name: string
  marketplace: string
  sourceRoot: string
  manifest: Record<string, unknown>
  skills: Record<string, string>
  agents: Record<string, string>
  commands: Record<string, string>
  workflows: Record<string, string>
  outputStyles: Record<string, string>
  themes: Record<string, string>
  hooks: Record<string, unknown>
  settings?: string
}

export interface SyncUserData {
  skills: Record<string, string>   // relativePath → fileContent
  agents: Record<string, string>
  commands: Record<string, string> // ~/.claude/commands/*.md — custom slash commands
  plugins: Record<string, SyncPluginData>
  settings?: string
  claudeMd?: string
}

export interface SyncProjectData {
  skills: Record<string, string>
  rules: Record<string, string>
  agents: Record<string, string>
  commands: Record<string, string> // .claude/commands/*.md
  settings?: string       // .claude/settings.json (hooks and project config)
  claudeMd?: string        // root CLAUDE.md
  dotClaudeMd?: string     // .claude/CLAUDE.md
  claudeJson?: string      // .claude.json
  projectPath: string      // for identification
}
