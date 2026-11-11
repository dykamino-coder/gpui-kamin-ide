// Engine types re-exported so an embedding UI never imports private paths. The
// declaration bundle inlines them into dist/index.d.ts.
export type {
  TabInfo as Tab, ConnectionConfig, ConnectionState as SessionConnectionState, SessionUsage, SessionUsageWindow,
  PermissionDecision, PermissionRequest, ElicitationRequest, SavedSession,
  ExternalMcpServerConfig as McpServerConfig, ExternalMcpServerInfo as McpServer, McpToolInfo, McpResourceInfo,
  McpResourceTemplateInfo, McpPromptInfo, McpRegistryEntry, MarketplaceInfo as Marketplace,
} from '../../webview/shared/types'
export type { JsonlEntry, JsonlStatus } from '../../webview/shared/jsonl-types'

import type { ExternalMcpServerConfig, McpRegistryEntry } from '../../webview/shared/types'

export type TabId = string

/** Incremental text or thinking append of the live turn (`streaming-delta`). Sent only when the
 * Bridge server streams for this credential; otherwise the transcript arrives as whole entries. */
export interface StreamingDelta { msgId: string; blockIdx: number; appendText: string; kind: 'text' | 'thinking' }

/** Busy/idle of a session (`session:activity`): `isWorking` is true while a turn runs. */
export interface SessionActivity { rawTitle: string; isWorking: boolean; promptReady?: boolean; waiting?: boolean; hookDriven?: boolean }

/** A question from the agent's AskUserQuestion tool (`ask-user-question`). The tool returns at once;
 * the answer reaches the agent as the user's next message. */
export interface UserQuestion {
  requestId: string
  question: string
  options?: string[]
  questions?: { header?: string; question: string; options?: (string | { label: string; description?: string })[]; multiSelect?: boolean }[]
  plan?: string
  isPlanApproval?: boolean
}

/** Host-side approval policy for the workspace tools (Bash, Write, …) the agent calls. */
export type ToolPermissionMode = 'ask' | 'acceptEdits' | 'plan' | 'bypassPermissions'

export interface SubagentEntries { tab: TabId; agentName: string; agentId?: string; entries: unknown[] }

export interface MonitorInfo {
  /** `<tabId>/<plugin>@<marketplace>/<monitor>` */
  id: string
  tabId: TabId
  pluginId: string
  pluginName: string
  monitorName: string
  description: string
  command: string
  status: 'running' | 'exited' | 'error'
  startedAt: number
  exitCode?: number | null
}
export interface MonitorLine { ts: number; stream: 'stdout' | 'stderr' | 'meta'; text: string }
export type MonitorEvent =
  | { type: 'started'; monitor: Omit<MonitorInfo, 'status' | 'startedAt' | 'exitCode'> }
  | { type: 'output'; id: string; line: MonitorLine }
  | { type: 'status'; id: string; status: MonitorInfo['status']; exitCode?: number | null }
  | { type: 'stopped'; id: string }
  | { type: 'error'; tabId: TabId; pluginId: string; monitorName: string; error: string }

/** Matches PreviewDescriptor in the workspace host (workspace/src/preview.ts). */
export interface PreviewDescriptor {
  name: string
  /** Same-origin path under /preview/remotes/<name>/. */
  entry: string
  exposedModule: string
  props: Record<string, unknown>
  type: 'global' | 'module'
  /** Increments on every mount or remount. */
  revision: number
}

/** Payload of the `workspace:preview-state` event and of GET /preview/state. */
export interface PreviewState {
  descriptor: PreviewDescriptor | null
  /** Fingerprint of the remote entry; changes when the dev server rebuilt it. */
  build: string | null
  /** False while the dev server inside the workspace is not answering. */
  available: boolean
}

/** Options of a new session. The host pins the Bridge server and credential. */
export interface CreateTabOptions {
  /** Project directory inside the workspace; relative paths resolve against its root. */
  cwd?: string
}

/** A draft for `mcp.addServer`: the host assigns `id`. */
export type McpServerInput = Omit<ExternalMcpServerConfig, 'id'>
/** Result of `mcp.testServer` (`mcp:test-server`): a fresh connection attempt and its tool names. */
export interface McpTestResult { ok: boolean; tools: string[]; error?: string }
/** `mcp:read-resource` / `mcp:get-prompt`: the MCP server's own reply, or its failure text. */
export type McpCallResult = { ok: true; result: unknown } | { ok: false; error: string }
/** `mcp:registry-list`: the public connector registry fetched by the host. */
export interface McpRegistryResult { ok: boolean; servers: McpRegistryEntry[]; error?: string }
/** One tool call the host executes for a session (`mcp-activity`): `pending`, then `completed` or `denied`. */
export interface McpActivity {
  requestId: string
  toolName: string
  input: Record<string, unknown>
  timestamp: number
  status: 'pending' | 'completed' | 'denied'
  result?: unknown
  durationMs?: number
}

/** A skill visible to sessions (`skills:list`): project, user or `plugin:<name>`. */
export interface Skill {
  name: string
  fileName: string
  description: string
  /** Absolute path of the skill file in the workspace container. */
  path: string
  source: 'project' | 'user' | `plugin:${string}`
  displayName?: string
  argumentHint?: string
  shell?: string
  version?: string
  whenToUse?: string
  disableModelInvocation?: boolean
}
/** Result of `skills.create`: the stored name is a slug of the requested one. */
export interface CreatedSkill { name: string; fileName: string; path: string }

/** A subagent definition (`agents:list`). */
export interface Agent {
  name: string
  fileName: string
  description: string
  model: string | null
  tools: string[]
  path: string
  source: 'project' | 'user' | `plugin:${string}`
  color?: string
  effort?: string | number
  maxTurns?: number
  disallowedTools?: string[]
  memory?: string
  isolation?: string
  background?: boolean
  skills?: string[]
}

export interface PluginContentCounts {
  commands: number
  agents: number
  skills: number
  lspServers: number
  mcpServers: number
  hooks: number
  isBrokenMarketplace?: boolean
  subPluginTotal?: number
  subPluginInstalled?: number
  nestedMarketplaceName?: string
}
export interface PluginDependency { name: string; requested: string | null; installed: boolean; enabled: boolean }
/** An installed plugin (`plugins:list-installed`). Its id is `<name>@<marketplace>`. */
export interface InstalledPlugin {
  name: string
  marketplace: string
  enabled: boolean
  version: string
  scope: string
  /** Pass to `plugins.contents`. */
  installPath: string
  installedAt: string
  description: string
  isCached: boolean
  counts: PluginContentCounts
  dependencies: PluginDependency[]
}
export interface PluginContentItem { name: string; description?: string }
/** What a plugin directory provides (`plugins:get-contents`). */
export interface PluginContents {
  skills: PluginContentItem[]
  agents: PluginContentItem[]
  commands: PluginContentItem[]
  mcpServers: string[]
  hooks: { event: string; matcher?: string; type: string; summary: string }[]
}
/** A plugin offered by a marketplace (`plugins:browse-marketplace`). */
export interface MarketplacePlugin {
  name: string
  version: string
  description: string
  author: string
  marketplace: string
  keywords: string[]
  /** Pass to `plugins.install` and `plugins.contents`. */
  pluginPath: string
  isInstalled: boolean
  hasCommands: boolean
  isCached?: boolean
  counts?: PluginContentCounts
  /** The plugin's files live in another repository that the host clones on demand. */
  isRemoteSource?: boolean
  cloneFailed?: boolean
  cloneError?: string
  isLocalSourceMissing?: boolean
}
/** Result of a settings mutation: open sessions pick the change up after a restart. */
export type RestartResult = { ok: true; restartRequired?: boolean } | { ok: false; error: string }
/** `plugins:get-options-schema`: the plugin's `userConfig` declarations and stored values. */
export interface PluginOptions { schema: Record<string, Record<string, unknown>>; values: Record<string, unknown>; sensitiveKeys: string[] }
/** `plugins:update-marketplace`. */
export interface MarketplaceUpdate { ok: boolean; lastUpdated?: string; changed?: boolean; error?: string }
/** One marketplace of `plugins:refresh-all-marketplaces`, and each `marketplaces:updated` event. */
export interface MarketplaceRefresh { name: string; ok: boolean; changed?: boolean; error?: string }
/** `plugins:browse-progress`: plugins of a marketplace scanned so far. */
export interface BrowseProgress { marketplace: string; done: number; total: number; current: string | null }

/** One hook handler as stored in settings.json. `host` overrides where a `command` hook runs. */
export interface HookHandler {
  type: 'command' | 'prompt' | 'agent' | 'http' | 'mcp_tool' | (string & {})
  command?: string
  args?: string[]
  prompt?: string
  url?: string
  server?: string
  tool?: string
  input?: Record<string, unknown>
  host?: 'local' | 'server'
  if?: string
  timeout?: number
  async?: boolean
  asyncRewake?: boolean
}
export interface HookMatcher { matcher?: string; hooks: HookHandler[] }
/** User hooks by event name (`hooks:list`); positions in it address `hooks.save` and `hooks.remove`. */
export type HooksByEvent = Record<string, HookMatcher[]>
export interface HookSource { kind: 'user' | 'project' | 'plugin'; pluginId?: string; manifestPath?: string; projectPath?: string }
/** A hook from any source with its origin (`hooks:list-all`). */
export interface HookEntry { event: string; matcherIdx: number; matcher?: string; handler: HookHandler; source: HookSource }
/** A ready-made hook from the Bridge server's library (`hooks:get-library`). */
export interface HookTemplate { id: string; event: string; matcher?: string; handler: HookHandler; [key: string]: unknown }
/** `hooks.save`: without `index` the handler is appended, with it the addressed handler is replaced. */
export interface HookDraft { event: string; matcher?: string; handler: HookHandler; index?: { matcherIdx: number; handlerIdx: number } }
/** `hooks:test`: the hook's outcome, run in the workspace (`effectiveHost: 'local'`) or on the Bridge server. */
export interface HookTestResult { ok: boolean; effectiveHost?: string; result?: Record<string, unknown>; error?: string; [key: string]: unknown }
export interface PluginHook { event: string; matcher?: string; handler: HookHandler; hash: string }
/** Hooks of an enabled plugin that do not run until approved (`plugin-hooks:awaiting-approval`). */
export interface PendingPluginHookApproval { pluginId: string; hooks: PluginHook[]; approvedHashes: string[] }
