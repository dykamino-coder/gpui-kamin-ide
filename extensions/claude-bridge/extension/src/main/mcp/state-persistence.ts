// State persistence for `McpServerManager` — extracted from `manager.ts`
// (Sprint 2 / Stage C, C3). Owns:
//   1. Legacy electron-store cache (gradual deprecation, kept for migration).
//   2. `~/.claude.json` mutator (the actual source of truth, shared with CLI).
//   3. Serialise-and-write helpers used by the manager class.
//
// All functions are pure / class-free so the manager file shrinks and the
// FS-touching code can be unit-tested in isolation.

import path from 'path'
import fs from 'fs'
import os from 'os'
import type { ExternalMcpServerConfig } from '../../shared/types'

/** Tiny JSON-file store replacing electron-store (KaminIDE VSIX). This is the
 *  LEGACY persistence layer — current code clears it on every save and uses
 *  `~/.claude.json` as the source of truth, so it exists only for one-time
 *  migration of pre-`~/.claude.json` configs. */
class McpLegacyStore {
  private readonly file = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'mcp-servers.json')
  private data: { mcpServers: ExternalMcpServerConfig[] }

  constructor() {
    try {
      this.data = JSON.parse(fs.readFileSync(this.file, 'utf8'))
      if (!Array.isArray(this.data.mcpServers)) this.data.mcpServers = []
    } catch {
      this.data = { mcpServers: [] }
    }
  }
  get(key: 'mcpServers'): ExternalMcpServerConfig[] { return this.data[key] }
  set(key: 'mcpServers', value: ExternalMcpServerConfig[]): void {
    this.data[key] = value
    try {
      fs.mkdirSync(path.dirname(this.file), { recursive: true })
      const tmp = `${this.file}.tmp`
      fs.writeFileSync(tmp, JSON.stringify(this.data, null, 2), 'utf8')
      fs.renameSync(tmp, this.file)
    } catch { /* best-effort */ }
  }
}

let _store: McpLegacyStore | null = null

export function getMcpStore(): McpLegacyStore {
  if (!_store) _store = new McpLegacyStore()
  return _store
}

/** Serialise a server config into `~/.claude.json` `mcpServers` entry shape.
 *  Stripping unset fields keeps the file tidy (CLI re-emits the same shape
 *  on its own writes). */
export function configToClaudeJsonEntry(config: ExternalMcpServerConfig): Record<string, unknown> {
  const entry: Record<string, unknown> = { type: config.type }
  if (config.type === 'http' || config.type === 'sse' || config.type === 'ws') {
    if (config.url) entry.url = config.url
    if (config.headers && Object.keys(config.headers).length > 0) entry.headers = config.headers
  } else {
    if (config.command) entry.command = config.command
    if (config.args && config.args.length > 0) entry.args = config.args
    if (config.env && Object.keys(config.env).length > 0) entry.env = config.env
  }
  if (config.oauth) entry.oauth = config.oauth
  return entry
}

/** Read-modify-write of `~/.claude.json` so MCP config stays consistent.
 *  The file is shared with Claude Code CLI — the mutator only touches the
 *  `mcpServers` slice; everything else is preserved verbatim. */
export async function mutateClaudeJson(
  mutate: (mcpServers: Record<string, any>, root: any) => void,
): Promise<void> {
  const claudeJsonPath = path.join(os.homedir(), '.claude.json')
  let parsed: any = {}
  try {
    const raw = await fs.promises.readFile(claudeJsonPath, 'utf-8')
    parsed = JSON.parse(raw)
  } catch { /* fresh file */ }
  if (!parsed || typeof parsed !== 'object') parsed = {}
  if (!parsed.mcpServers || typeof parsed.mcpServers !== 'object') parsed.mcpServers = {}
  mutate(parsed.mcpServers, parsed)
  await fs.promises.writeFile(claudeJsonPath, JSON.stringify(parsed, null, 2), 'utf-8')
}

// ─── Кэш схем тулов для ленивых stdio-серверов (экономия RAM) ────────────────
// stdio-MCP = child-процесс; со второго запуска IDE тулы регистрируются в CLI
// ИЗ ЭТОГО КЭША, а процесс поднимается только при первом tools/call
// (см. LAZY_STDIO_MCP в manager.ts). Ключ — mcpConnectionFingerprint конфига:
// правка команды/аргов инвалидирует запись естественным несовпадением ключа.

export interface CachedToolSchema {
  name: string
  description: string
  inputSchema: Record<string, unknown>
}

const toolSchemaCacheFile = path.join(os.homedir(), '.claude', 'open-claude-bridge', 'mcp-tool-schemas.json')

export function loadToolSchemaCache(): Record<string, CachedToolSchema[]> {
  try {
    const parsed: unknown = JSON.parse(fs.readFileSync(toolSchemaCacheFile, 'utf8'))
    if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
      return parsed as Record<string, CachedToolSchema[]>
    }
  } catch { /* нет кэша / бит — первый коннект пересоздаст */ }
  return {}
}

export function saveToolSchemaCache(cache: Record<string, CachedToolSchema[]>): void {
  try {
    fs.mkdirSync(path.dirname(toolSchemaCacheFile), { recursive: true })
    const tmp = `${toolSchemaCacheFile}.tmp`
    fs.writeFileSync(tmp, JSON.stringify(cache, null, 2), 'utf8')
    fs.renameSync(tmp, toolSchemaCacheFile)
  } catch { /* best-effort: кэш — оптимизация, не источник истины */ }
}
