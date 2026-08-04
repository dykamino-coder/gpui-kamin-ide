// ============================================================================
// Core Public API
// ============================================================================

// Server
export { createApp } from "./server"
export type { App } from "./server"

// Config
export { config, VERSION, SERVICE_NAME } from "./config"
export { getConfigPath, readBridgeSettings } from "./config/opencode"
export type { ConfigCheckResult, OpenCodeConfig, OpenCodeProviderOptions } from "./config/opencode"

// CLI args
export { args } from "./utils/args"
export type { ParsedArgs } from "./utils/args"

// Logging
export { LOG_PATH } from "./logging"

// Stats
export { stats } from "./stats/tracker"

// Events
export { eventBus } from "./events/bus"
export type { BridgeEvent, BridgeEventType } from "./events/bus"

// WebSocket
export { attachWebSocket, installUpgradeHandler } from "./server/ws"
export { attachTerminalWebSocket, cleanupTerminal } from "./server/terminal"
// PTY Session Management
export {
  createSession,
  destroySession,
  getSession,
  getAllSessions,
  shutdownAll,
  startSessionReaper,
  stopSessionReaper,
  startConsoleShrinkSweep,
  stopConsoleShrinkSweep,
  getSessionTree,
} from "./pty/session-manager"
export { attachSessionWebSocket } from "./pty/session-ws"
export type { ToolDefinition, TreeNode, SavedSession } from "./pty/types"

// Types
export type {
  Config,
  AccountInfo,
  HealthCheck,
} from "./types"
