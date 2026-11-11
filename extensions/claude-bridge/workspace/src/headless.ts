// The one composition of the workspace service: shared BridgeHost, files, shell,
// terminals, MCP, hooks, skills, plugins, user tools and the preview proxy. The
// CLI (cli.ts) and embedding applications both start the service through
// startHeadlessWorkspace(); only the sign-in policy and the optional UI differ.
// Published as @kaminide/bridge-workspace-host (see HEADLESS.md).
import fs from 'node:fs'
import path from 'node:path'
import type { IncomingMessage, ServerResponse } from 'node:http'
import type { AddressInfo } from 'node:net'
import { WorkspaceAuth, serverTokenResolver, type WorkspaceAuthPolicy } from './auth.js'
import { createBrowserEnvironment } from './browser-environment.js'
import { withBrowserServer } from './browser-server.js'
import { loadCredentials, resetCredentials, SESSION_FILE } from './credentials.js'
import { WorkspaceFiles } from './files.js'
import { WorkspacePreview } from './preview.js'
import { createSharedBackend } from './runtime.js'
import { startWorkspaceServer } from './server.js'
import { withWorkspaceShell } from './shell-state.js'
import { TerminalWorkspaceBackend } from './terminals.js'
declare const BRIDGE_VERSION: string

/** BridgeHost (client engine) version this build reports to the Bridge server. */
export const version: string = BRIDGE_VERSION

export {
  SESSION_COOKIE, readSessionCookie, createSessionCookie, sessionCookieHeader,
  type WorkspaceAuthPolicy,
} from './auth.js'
export { resetCredentials }
export type { PreviewDescriptor, PreviewSnapshot } from './preview.js'
export { WorkspacePreview }

/** The workspace's own Bridge credential, for policies built on it. */
export interface BridgeCredentialContext {
  token: string
  /** Per-workspace secret used to key persisted browser sessions. */
  browserSecret: string
  storage: string
  serverUrl: string
}

export interface HeadlessWorkspaceOptions {
  /** Project directory the agent works in. Must exist. */
  root: string
  /** Private state directory (credentials, sessions, drafts). Keep it on a persistent volume. */
  storage: string
  /** Bridge server origin or /ws/session URL (ws, wss, http or https). */
  serverUrl: string
  /** Exact origin browsers use; Host and Origin of every request must match it. */
  origin: string
  /** Listen address; defaults to 127.0.0.1. */
  bind?: string
  /** Listen port; defaults to 3460, 0 picks a free port. */
  port?: number
  /** Sign-in policy of the embedding application, or a factory given the
   * workspace's own Bridge credential (see bridgeTokenAuth). */
  auth: WorkspaceAuthPolicy | ((bridge: BridgeCredentialContext) => WorkspaceAuthPolicy)
  /** Loopback-only automatic sign-in through `auth.loginLocal`. Default false. */
  autoLogin?: boolean
  /** Optional trusted static assets. Without it non-API GET requests answer 503. */
  serveApp?: (request: IncomingMessage, response: ServerResponse) => void | Promise<void>
  /** Source of the one-time bootstrap secret (BRIDGE_TOKEN / BRIDGE_TOKEN_FILE).
   * Defaults to process.env. The two variables are removed after start. */
  env?: NodeJS.ProcessEnv
  onWatchError?: () => void
}

export interface HeadlessWorkspace {
  /** Effective browser origin. */
  readonly origin: string
  readonly address: AddressInfo
  /** Module Federation preview state owned by this process. */
  readonly preview: WorkspacePreview
  /** Idempotent: closes sockets, terminals, file watches and the shared runtime. */
  close(): Promise<void>
}

/** The built-in policy: sign in with the workspace's Bridge token, sessions
 * revalidated against the Bridge server and persisted in `storage`. */
export function bridgeTokenAuth(bridge: BridgeCredentialContext, ttlMs = 30 * 24 * 60 * 60 * 1000): WorkspaceAuthPolicy {
  return new WorkspaceAuth(bridge.token, serverTokenResolver(bridge.serverUrl), ttlMs, Date.now,
    { secret: bridge.browserSecret, file: path.join(bridge.storage, SESSION_FILE) })
}

/** Start the workspace service. One instance per process: the shared handler
 * registries are process-wide (see createSharedBackend). */
export async function startHeadlessWorkspace(options: HeadlessWorkspaceOptions): Promise<HeadlessWorkspace> {
  const root = fs.realpathSync(options.root)
  const storage = path.resolve(options.storage)
  const publicUrl = new URL(options.origin)
  if (publicUrl.origin !== options.origin || !['http:', 'https:'].includes(publicUrl.protocol)) {
    throw new Error('Invalid browser origin')
  }
  const { serverUrl } = options
  const env = options.env ?? process.env
  const credentials = await loadCredentials(storage, serverUrl, env)
  const { token } = credentials
  // The bootstrap secret is consumed once; it must not leak into agent shells.
  delete env.BRIDGE_TOKEN
  delete env.BRIDGE_TOKEN_FILE
  const auth = typeof options.auth === 'function'
    ? options.auth({ token, browserSecret: credentials.browserSecret, storage, serverUrl })
    : options.auth

  const preview = new WorkspacePreview()
  const browser = createBrowserEnvironment({ root, preview })
  const backend = createSharedBackend({ root, storage, serverUrl, token, version: BRIDGE_VERSION, environment: browser.environment })
  const dispose = () => {
    try { terminals?.dispose() } finally { try { files?.dispose() } finally { backend.dispose() } }
  }
  let files: WorkspaceFiles | undefined
  let terminals: TerminalWorkspaceBackend | undefined
  let server: Awaited<ReturnType<typeof startWorkspaceServer>>
  try {
    const shell = withWorkspaceShell(backend, path.join(storage, 'workspace-shell.json'))
    files = new WorkspaceFiles(browser.wrap(withBrowserServer(shell, { serverUrl, token })), {
      root,
      onWatchError: options.onWatchError ?? (() => { process.stderr.write('Bridge workspace file watch failed.\n') }),
    })
    terminals = new TerminalWorkspaceBackend(files, root)
    server = await startWorkspaceServer({
      bind: options.bind, port: options.port, origin: options.origin, autoLogin: options.autoLogin ?? false,
      auth, backend: terminals, serveApp: options.serveApp, preview,
    })
  } catch (error) { dispose(); throw error }

  let closed: Promise<void> | undefined
  return {
    origin: server.origin,
    address: server.address,
    preview,
    close() {
      closed ??= (async () => { dispose(); await server.close() })()
      return closed
    },
  }
}
