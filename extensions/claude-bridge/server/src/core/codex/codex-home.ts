// ============================================================================
// The Bridge CODEX_HOME and the environment of the codex child.
//
// Like the Claude account (/home/bridge/.claude), the ChatGPT account is one
// per Bridge server: an administrator signs in once (device code) and every
// session uses it. One home, served by one app-server process (codex-host.ts),
// keeps a single writer of auth.json, so token refreshes cannot race. In the
// container image it is the standard /home/bridge/.codex on its own volume
// (BRIDGE_CODEX_HOME and BRIDGE_CONTAINER=1 come from the Dockerfile). On a
// host install it defaults to a Bridge-owned ~/.kamin-bridge/codex, never the
// operator's personal ~/.codex. Older servers kept it in ~/.claude/codex-home;
// migrateLegacyCodexHome moves it out once. Bridge owns config.toml and
// catalog.json there and never reads or writes auth.json.
// ============================================================================

import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { mergeLoopbackNoProxy } from '../config/no-proxy'
import { codexUserConfig, renderToml } from './config-toml'
import type { ModelCatalog } from './model-catalog'

export const CODEX_HOME_ENV = 'BRIDGE_CODEX_HOME'

/** Set by the container image: there ~/.codex belongs to the Bridge user, not a person. */
export const CONTAINER_ENV = 'BRIDGE_CONTAINER'

export function defaultCodexHome(env: NodeJS.ProcessEnv = process.env): string {
  return env[CODEX_HOME_ENV] || path.join(os.homedir(), '.kamin-bridge', 'codex')
}

/** Where servers before this change kept the home (inside the Claude config dir). */
export function legacyCodexHome(operatorHome: string = os.homedir()): string {
  return path.join(operatorHome, '.claude', 'codex-home')
}

/**
 * Moves an older home out of ~/.claude once: only when the new home has no
 * auth.json yet and the legacy one exists. Copy, then remove, so it works
 * across volumes; a crash mid-copy leaves the legacy home intact and the next
 * start copies again.
 */
export function migrateLegacyCodexHome(home: string, legacy: string = legacyCodexHome()): boolean {
  if (path.resolve(home) === path.resolve(legacy)) return false
  if (!fs.existsSync(legacy)) return false
  if (fs.existsSync(path.join(home, 'auth.json'))) return false
  fs.mkdirSync(home, { recursive: true, mode: 0o700 })
  fs.cpSync(legacy, home, { recursive: true, force: true })
  fs.rmSync(legacy, { recursive: true, force: true })
  return true
}

/**
 * Refuses the operator's personal codex home and anything inside it. Inside
 * the container image (BRIDGE_CONTAINER=1) the Bridge user's own ~/.codex is
 * allowed when BRIDGE_CODEX_HOME names it explicitly.
 */
export function assertBridgeOwnedHome(
  home: string,
  operatorHome: string = os.homedir(),
  env: NodeJS.ProcessEnv = process.env,
): void {
  const resolved = path.resolve(home)
  const explicit = env[CODEX_HOME_ENV]
  if (env[CONTAINER_ENV] === '1' && explicit && path.resolve(explicit) === resolved) return
  const personal = path.resolve(operatorHome, '.codex')
  const rel = path.relative(personal, resolved)
  if (rel === '' || (!rel.startsWith('..') && !path.isAbsolute(rel))) {
    throw new Error('Refusing to use the operator ~/.codex as a Bridge CODEX_HOME')
  }
}

function writeAtomic(file: string, content: string): void {
  const tmp = `${file}.${process.pid}.${crypto.randomUUID()}.tmp`
  fs.writeFileSync(tmp, content, { mode: 0o600 })
  try {
    fs.renameSync(tmp, file)
  } catch (error) {
    fs.rmSync(tmp, { force: true })
    throw error
  }
}

export interface PreparedCodexHome {
  home: string
  configPath: string
  catalogPath: string
}

/**
 * Creates the home (0700) and (re)writes Bridge's files atomically. Every
 * preparation writes identical bytes for one binary version, so a concurrent
 * second writer is harmless; per-session settings travel in thread requests.
 */
export function prepareCodexHome(home: string, catalog: ModelCatalog): PreparedCodexHome {
  assertBridgeOwnedHome(home)
  fs.mkdirSync(home, { recursive: true, mode: 0o700 })
  if (process.platform !== 'win32') fs.chmodSync(home, 0o700)
  const catalogPath = path.join(home, 'catalog.json')
  const configPath = path.join(home, 'config.toml')
  writeAtomic(catalogPath, JSON.stringify(catalog))
  writeAtomic(configPath, renderToml(codexUserConfig({ catalogPath })))
  return { home, configPath, catalogPath }
}

/** Variables a codex child may inherit; everything else (provider keys, Bridge secrets) is dropped. */
const INHERITED_ENV = [
  'PATH',
  'Path',
  'PATHEXT',
  'SystemRoot',
  'SYSTEMROOT',
  'windir',
  'ComSpec',
  'COMSPEC',
  'TEMP',
  'TMP',
  'TMPDIR',
  'APPDATA',
  'LOCALAPPDATA',
  'LANG',
  'LC_ALL',
  'LC_CTYPE',
  'TZ',
  'HTTPS_PROXY',
  'https_proxy',
  'HTTP_PROXY',
  'http_proxy',
  'NO_PROXY',
  'no_proxy',
  'SSL_CERT_FILE',
  'SSL_CERT_DIR',
  'NODE_EXTRA_CA_CERTS',
  'CODEX_CA_CERTIFICATE',
]

const PROXY_VARS = ['HTTPS_PROXY', 'https_proxy', 'HTTP_PROXY', 'http_proxy', 'ALL_PROXY', 'all_proxy']

/** Codex (rustls) reads extra trust roots from this variable, added to the native roots. */
export const CODEX_CA_ENV = 'CODEX_CA_CERTIFICATE'

export interface CodexEnvOptions {
  home: string
  base?: NodeJS.ProcessEnv
  /**
   * Bridge's corporate proxy/CA settings — the same `injectProxyEnv` the
   * Claude CLI env goes through (session-env.ts), injected so unit tests do
   * not load the settings store.
   */
  injectProxy?: (env: Record<string, string>) => void
  /** Test-only additions (e.g. a mock provider key). */
  extra?: Record<string, string>
}

export function buildCodexEnv(options: CodexEnvOptions): NodeJS.ProcessEnv {
  const base = options.base ?? process.env
  const env: Record<string, string> = {}
  for (const key of [...INHERITED_ENV, 'ALL_PROXY', 'all_proxy']) {
    const value = base[key]
    if (value !== undefined) env[key] = value
  }
  options.injectProxy?.(env)
  // Inherited proxies get the same loopback bypass as dashboard-configured
  // ones: MCP calls to Bridge on 127.0.0.1 must never enter the proxy.
  if (PROXY_VARS.some((key) => env[key])) mergeLoopbackNoProxy(env)
  // The dashboard CA reaches Node children as NODE_EXTRA_CA_CERTS; codex's
  // HTTP stack ignores that and reads CODEX_CA_CERTIFICATE (codex-rs
  // http-client/src/custom_ca.rs, fallback SSL_CERT_FILE).
  if (env.NODE_EXTRA_CA_CERTS && !env[CODEX_CA_ENV]) env[CODEX_CA_ENV] = env.NODE_EXTRA_CA_CERTS
  env.CODEX_HOME = options.home
  // Codex also looks for user-level skills/agents under HOME; pointing HOME at
  // the per-user codex home keeps the server account's dotfiles out of reach.
  env.HOME = options.home
  env.USERPROFILE = options.home
  env.RUST_LOG = base.BRIDGE_CODEX_RUST_LOG || 'warn'
  return { ...env, ...options.extra }
}
