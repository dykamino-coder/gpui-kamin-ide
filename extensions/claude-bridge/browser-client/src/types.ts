// Engine types re-exported so an embedding UI never imports private paths. The
// declaration bundle inlines them into dist/index.d.ts.
export type {
  TabInfo as Tab, ConnectionConfig, ConnectionState as SessionConnectionState, SessionUsage, SessionUsageWindow,
  PermissionDecision, PermissionRequest, ElicitationRequest, SavedSession,
} from '../../webview/shared/types'
export type { JsonlEntry, JsonlStatus } from '../../webview/shared/jsonl-types'

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
