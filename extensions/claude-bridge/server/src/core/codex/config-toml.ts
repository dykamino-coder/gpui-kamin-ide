// ============================================================================
// Codex configuration owned by Bridge.
//
// Two scopes, one structured model:
//   - process (per user): the lockdown. Rendered both as `-c key=value`
//     overrides of the user's app-server (CLI overrides have the highest
//     precedence, so neither the user layer nor a project `.codex/config.toml`
//     can re-enable a built-in tool) and as `$CODEX_HOME/config.toml`, so any
//     codex subcommand run against that home (login, debug) is equally locked.
//   - thread (per Bridge session): the Bridge MCP endpoint, passed as the
//     `config` of thread/start|resume. One app-server serves all threads of a
//     user, and each thread connects to its own `/mcp/<endpoint>` with its own
//     bearer header. Verified with 0.160.1: threads reach their own endpoints,
//     and the header is not written to CODEX_HOME (rollouts, sqlite, config).
//
// The lockdown is exactly the configuration verified offline in the codexbr
// research (codex-cli 0.160.1): built-in shell/exec/patch/image/agent
// tools are off and the model sees only Bridge MCP tools, provider-side web
// search, and codex's MCP-resource and request_user_input helpers. apply_patch has no feature flag;
// it is removed per model through the patched catalog (model-catalog.ts).
// ============================================================================

import { INTERACTIVE_TOOLS } from '../pty/interactive-tools'

export type TomlScalar = string | number | boolean
export type TomlValue = TomlScalar | TomlValue[] | TomlTable
export interface TomlTable {
  [key: string]: TomlValue | undefined
}

/** Features that would give the model a tool besides Bridge MCP, or a side channel. */
export const CODEX_DISABLED_FEATURES = [
  'shell_tool',
  'unified_exec',
  'view_image',
  'goals',
  'apps',
  'plugins',
  'browser_use',
  'computer_use',
  'image_generation',
  'multi_agent',
  'memories',
  'hooks',
  'tool_suggest',
  'skill_search',
] as const

/** MCP server names; codex exposes their tools as `mcp__<server>` namespaces. */
export const BRIDGE_MCP_SERVER = 'bridge'
export const BRIDGE_INTERACTIVE_MCP_SERVER = 'bridge_interactive'
/**
 * Codex's MCP deadline (default 300 s) is per server, not per tool. Bridge
 * enforces its own per-tool deadlines (2 min, heavy tools 30 min), so the
 * codex one is only a backstop: one hour for ordinary tools.
 */
export const BRIDGE_MCP_TOOL_TIMEOUT_SEC = 3_600
/**
 * Tools that wait for the user have no Bridge deadline. Codex has no
 * "unbounded", so they live on a second server pointed at the same endpoint
 * with a 30-day timeout instead of stretching every tool to that.
 */
export const BRIDGE_INTERACTIVE_TOOL_TIMEOUT_SEC = 30 * 24 * 3_600

export interface CodexLockdownOptions {
  /** Absolute path of the patched model catalog. */
  catalogPath: string
  /**
   * Provider-side web search, on by default like Claude's WebSearch (owner
   * decision). It runs at OpenAI and reaches nothing on the server or client.
   */
  webSearch?: 'live' | 'cached' | 'disabled'
}

export function codexLockdownConfig(options: CodexLockdownOptions): TomlTable {
  return {
    web_search: options.webSearch ?? 'live',
    model_catalog_json: options.catalogPath,
    features: Object.fromEntries(CODEX_DISABLED_FEATURES.map((name) => [name, false])),
  }
}

/** Per-user file: lockdown plus file-based credential storage for the container. */
export function codexUserConfig(options: CodexLockdownOptions): TomlTable {
  return { cli_auth_credentials_store: 'file', ...codexLockdownConfig(options) }
}

export interface BridgeMcpOptions {
  /** `http://127.0.0.1:<port>/mcp/<endpointId>` — the existing Bridge MCP route. */
  url: string
  /** Per-session MCP bearer token (lives only in the thread request, never on disk). */
  token: string
  toolTimeoutSec?: number
  interactiveTools?: Iterable<string>
}

/** `config` for thread/start|resume: the session's Bridge MCP endpoint. */
export function bridgeMcpThreadConfig(options: BridgeMcpOptions): TomlTable {
  const interactive = [...(options.interactiveTools ?? INTERACTIVE_TOOLS)]
  const server = (timeout: number): TomlTable => ({
    url: options.url,
    http_headers: { Authorization: `Bearer ${options.token}` },
    // Bridge tools must be directly visible, not hidden behind tool_search.
    omit_tools_from: ['deferred'],
    // Bridge already asks the user on the client host; a codex approval would
    // hit approval_policy=never and fail every call.
    default_tools_approval_mode: 'approve',
    tool_timeout_sec: timeout,
    required: true,
  })
  return {
    mcp_servers: {
      [BRIDGE_MCP_SERVER]: {
        ...server(options.toolTimeoutSec ?? BRIDGE_MCP_TOOL_TIMEOUT_SEC),
        disabled_tools: interactive,
      },
      [BRIDGE_INTERACTIVE_MCP_SERVER]: {
        ...server(BRIDGE_INTERACTIVE_TOOL_TIMEOUT_SEC),
        enabled_tools: interactive,
        // The endpoint may not offer any interactive tool (e.g. a host without UI).
        required: false,
      },
    },
  }
}
/** Deep merge; later tables win on scalar conflicts. */
export function mergeTables(...tables: TomlTable[]): TomlTable {
  const out: TomlTable = {}
  for (const table of tables) {
    for (const [key, value] of Object.entries(table)) {
      if (value === undefined) continue
      const prev = out[key]
      out[key] = isTable(prev) && isTable(value) ? mergeTables(prev, value) : value
    }
  }
  return out
}

function isTable(value: TomlValue | undefined): value is TomlTable {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

const BARE_KEY = /^[A-Za-z0-9_-]+$/

function renderKey(key: string): string {
  return BARE_KEY.test(key) ? key : renderString(key)
}

function renderString(value: string): string {
  let out = '"'
  for (const ch of value) {
    const code = ch.codePointAt(0)!
    if (ch === '"') out += '\\"'
    else if (ch === '\\') out += '\\\\'
    else if (ch === '\n') out += '\\n'
    else if (ch === '\r') out += '\\r'
    else if (ch === '\t') out += '\\t'
    else if (code < 0x20 || code === 0x7f) out += `\\u${code.toString(16).padStart(4, '0')}`
    else out += ch
  }
  return out + '"'
}

export function renderValue(value: TomlValue): string {
  if (typeof value === 'string') return renderString(value)
  if (typeof value === 'boolean') return value ? 'true' : 'false'
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('TOML numbers must be finite')
    return String(value)
  }
  if (Array.isArray(value)) return `[${value.map(renderValue).join(', ')}]`
  const entries = Object.entries(value).filter(([, v]) => v !== undefined) as Array<[string, TomlValue]>
  return `{ ${entries.map(([k, v]) => `${renderKey(k)} = ${renderValue(v)}`).join(', ')} }`
}

/** Standard TOML document: scalars first, then `[a.b]` tables depth-first. */
export function renderToml(table: TomlTable): string {
  const lines: string[] = []
  const walk = (node: TomlTable, path: string[]) => {
    const entries = Object.entries(node).filter(([, v]) => v !== undefined) as Array<[string, TomlValue]>
    const scalars = entries.filter(([, v]) => !isTable(v))
    const tables = entries.filter(([, v]) => isTable(v)) as Array<[string, TomlTable]>
    if (path.length && (scalars.length || !tables.length)) {
      if (lines.length) lines.push('')
      lines.push(`[${path.map(renderKey).join('.')}]`)
    }
    for (const [key, value] of scalars) lines.push(`${renderKey(key)} = ${renderValue(value)}`)
    for (const [key, value] of tables) walk(value, [...path, key])
  }
  walk(table, [])
  return lines.join('\n') + '\n'
}

/**
 * `-c` arguments, one per leaf, in the order the model lists them. Codex
 * splits the override path on `.` without TOML key quoting, so only bare keys
 * are representable; anything else is a programming error, not user input.
 */
export function toConfigOverrides(table: TomlTable): string[] {
  const args: string[] = []
  const walk = (node: TomlTable, path: string[]) => {
    for (const [key, value] of Object.entries(node)) {
      if (value === undefined) continue
      if (!BARE_KEY.test(key)) throw new Error(`Config key ${JSON.stringify(key)} cannot be passed as -c`)
      if (isTable(value)) walk(value, [...path, key])
      else args.push('-c', `${[...path, key].join('.')}=${renderValue(value)}`)
    }
  }
  walk(table, [])
  return args
}
