// Barrel file preserved for backcompat. The 2095-line monolith was split
// into:
//   - ./manager.ts — class McpServerManager + registry + lifecycle + OAuth
//   - ./discovery.ts — plugin / ~/.claude.json / project .mcp.json discovery,
//                      plus parseMcpJson + env/home expansion
//   - ./test-log.ts — per-server test-log ring buffer helpers
//   - ./transports/stdio.ts — stdio transport
//   - ./transports/http.ts — streamable HTTP transport
//   - ./transports/sse-legacy.ts — legacy SSE (2024-11-05) transport
//   - ./transports/ws.ts — WebSocket transport
//   - ./transports/context.ts — TransportContext + McpServerState types

export { McpServerManager } from './manager'
export type { ExternalToolSchema, McpTestLogEntry } from './manager'
