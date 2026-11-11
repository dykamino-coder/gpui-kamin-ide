// Engine types re-exported so an embedding UI never imports private paths. The
// declaration bundle inlines them into dist/index.d.ts.
export type { TabInfo as Tab, ConnectionConfig, ConnectionState as SessionConnectionState } from '../../webview/shared/types'
export type { JsonlEntry, JsonlStatus } from '../../webview/shared/jsonl-types'

export type TabId = string

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
