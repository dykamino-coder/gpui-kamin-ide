// ============================================================================
// PTY Module — barrel export
// ============================================================================

export type {
  PtySession,
  SessionConfig,
  SessionState,
  McpToolRequest,
  McpToolResponse,
  McpToolDenied,
  PendingMcpCall,
  ClientToServerMsg,
  ElectronToServerMsg,
  ServerToClientMsg,
  ServerToElectronMsg,
  ToolDefinition,
  TreeNode,
  SavedSession,
  PendingElicitation,
  ElicitationResult,
} from './types'

export {
  createSession,
  destroySession,
  getSession,
  getAllSessions,
  countUserSessions,
  shutdownAll,
  writeInput,
  submitText,
  resizeTerminal,
  sendMcpCall,
  handleMcpResponse,
  handleMcpDenied,
  handleElicitationResponse,
  startSessionReaper,
  stopSessionReaper,
  startConsoleShrinkSweep,
  stopConsoleShrinkSweep,
  registerChildSession,
  getSessionTree,
  restartWithModel,
} from './session-manager'

export { attachSessionWebSocket, getSessionWs } from './session-ws'
export { handleMcpRequest } from './mcp-http-handler'
