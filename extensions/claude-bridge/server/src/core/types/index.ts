// ============================================================================
// Barrel re-export for all core types
// ============================================================================

// Core (Claude API / config / stats) — from legacy src/core/types.ts
export type {
  Config,
  AccountInfo,
  Stats,
  HealthCheck,
  AnthropicMessage,
  ContentBlock,
  ThinkingConfig,
  AnthropicRequest,
  TokenUsage,
  AnthropicResponse,
  AnthropicStreamEvent,
  ClaudeModel,
} from './core'

// PTY / session
export type {
  SessionState,
  McpLogEntry,
  PtySession,
  ToolDefinition,
  SessionConfig,
  TreeNode,
  SavedSession,
  SyncUserData,
  SyncProjectData,
} from './pty'

// MCP call flow
export type {
  McpToolRequest,
  McpToolResponse,
  McpToolDenied,
  PendingMcpCall,
} from './mcp'

// Elicitation
export type {
  PendingElicitation,
  ElicitationResult,
} from './elicitation'

// WebSocket protocol
export type {
  WsMsgSessionCreate,
  WsMsgSessionInput,
  WsMsgSessionSubmitText,
  WsMsgSessionResize,
  WsMsgMcpResponse,
  WsMsgMcpDenied,
  WsMsgElicitationResponse,
  WsMsgSessionResume,
  WsMsgSessionChangeEffort,
  WsMsgSessionChangeModel,
  WsMsgSessionDeleteData,
  WsMsgRegisterExternalTools,
  WsMsgJsonlDownloadRequest,
  WsMsgJsonlSyncRequest,
  ElectronToServerMsg,
  WsMsgSessionCreated,
  WsMsgSessionOutput,
  WsMsgSessionExit,
  WsMsgSessionError,
  WsMsgMcpCall,
  WsMsgJsonlEntries,
  WsMsgJsonlStatus,
  WsMsgElicitationRequest,
  WsMsgAgentSpawned,
  WsMsgTreeUpdate,
  WsMsgSessionRestarted,
  WsMsgSessionActivity,
  WsMsgJsonlDownloadResponse,
  ServerToElectronMsg,
} from './ws'
