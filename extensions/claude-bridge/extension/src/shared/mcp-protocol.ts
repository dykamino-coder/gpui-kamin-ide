// VSIX bridge host → Server
export type ClientMessage =
  | { type: 'session:create'; token: string; cwd?: string; cols?: number; rows?: number; protocolVersion?: number }
  | { type: 'session:input'; data: string }
  | { type: 'session:interrupt' }
  | { type: 'session:submitText'; data: string }
  | { type: 'session:resize'; cols: number; rows: number }
  | { type: 'mcp:response'; requestId: string; result: unknown }
  | { type: 'mcp:denied'; requestId: string; reason: string }
  | { type: 'elicitation:response'; requestId: string; action: 'accept' | 'deny' | 'dismiss'; content?: Record<string, unknown> }
  | { type: 'session:resume'; token: string; conversationId: string; cwd?: string; cols?: number; rows?: number; protocolVersion?: number }
  | { type: 'session:change-effort'; effort: string }
  | { type: 'session:change-model'; model: string }
  | { type: 'session:delete-data'; conversationId: string }
  | { type: 'hook:response'; requestId: string; result: { stdout: string; stderr: string; exitCode: number; outcome: 'success' | 'error' | 'timeout' | 'cancelled'; jsonOutput?: Record<string, unknown>; durationMs: number } }
  | { type: 'mcp:register-external-content'; resources: Array<{ uri: string; name: string; description?: string; mimeType?: string; serverId: string; rawUri: string }>; resourceTemplates: Array<{ uriTemplate: string; name: string; description?: string; mimeType?: string; serverId: string; rawUriTemplate: string }>; prompts: Array<{ name: string; description?: string; arguments?: unknown[]; serverId: string; rawName: string }> }

// Server → VSIX bridge host
export type ServerMessage =
  | { type: 'session:created'; sessionId: string; effort?: string; model?: string; settingsDir?: string }
  | { type: 'session:output'; data: string }
  | { type: 'session:exit'; code: number; sessionId: string }
  | { type: 'session:error'; error: string; fatal?: boolean }
  | { type: 'mcp:call'; requestId: string; toolName: string; input: Record<string, unknown> }
  | { type: 'jsonl:entries'; entries: unknown[] }
  | { type: 'jsonl:status'; status: string; filePath?: string; error?: string; compacted?: boolean; replayComplete?: boolean; fileHead?: string; fileSize?: number; replayProgress?: { sent: number; total: number }; segmentIndex?: { boundaries: { ts: string }[]; counts: number[] } }
  | { type: 'jsonl:segment-response'; fromTs: string; toTs: string; records: unknown[] }
  | { type: 'elicitation:request'; requestId: string; toolName: string; message: string; requestedSchema?: Record<string, unknown> }
  | { type: 'session:agent-spawned'; parentSessionId: string; childSessionId: string; agentName: string; description: string }
  | { type: 'session:tree-update'; tree: unknown[] }
  | { type: 'session:restarted'; sessionId: string; effort?: string; model?: string }
  // Горячая смена модели (/model в живой PTY) — sessionId прежний, без реплея.
  | { type: 'session:model-changed'; sessionId: string; effort?: string; model?: string }
  | { type: 'jsonl:subagent-entries'; agentName: string; entries: unknown[]; agentId?: string }
  | { type: 'session:title'; title: string }
  | { type: 'session:conversation-id'; sessionId: string; conversationId: string }
  | { type: 'session:activity'; sessionId: string; rawTitle: string; isWorking: boolean }
  | { type: 'jsonl:download-response'; content: string | null; fileName: string | null; error?: string }
  | { type: 'jsonl:download-begin'; fileName: string | null; total: number }
  | { type: 'jsonl:download-chunk'; chunk: string; sent: number; total: number }
  | { type: 'jsonl:download-end'; fileName: string | null; total: number }
  | { type: 'hook:execute'; requestId: string; sessionId: string; hookId: string; event: string; command: string; args?: string[]; pluginId?: string; shell?: 'bash' | 'powershell' | 'sh' | 'zsh'; cwd?: string; env?: Record<string, string>; payload: Record<string, unknown>; timeoutMs?: number }
  | { type: 'streaming:entry'; sessionId: string; entry: Record<string, unknown> }
  | { type: 'streaming:delta'; sessionId: string; msgId: string; blockIdx: number; appendText: string; kind: 'text' | 'thinking' }
  | { type: 'streaming:status'; sessionId: string; code: number; retryAfterMs?: number }
