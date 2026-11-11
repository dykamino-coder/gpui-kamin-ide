// What the client host declared about itself at session create/resume (INC-2026-0097).
// The server keeps tool schemas and wording; which built-in tools exist, the
// host platform and the name of the place where they run come from the host.
// A session without a declaration (older clients) keeps the full list and the
// legacy texts.
import type { HostDeclaration } from '../types/pty'
import { MCP_TOOLS } from './mcp-tools-defs'

export type { HostDeclaration }

type McpTool = (typeof MCP_TOOLS)[number]

const PLATFORM_LABELS: Record<string, string> = { win32: 'Windows', darwin: 'macOS', linux: 'Linux' }
const SHELL_TOOLS = ['Bash', 'PowerShell']
const DEFAULT_PLACE = 'the client host'

function phrase(value: unknown, max: number): string {
  if (typeof value !== 'string') return ''
  return value
    .replace(/[\u0000-\u001f\u007f`]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .slice(0, max)
    .trim()
}

/** Wire value → declaration; anything that is not a declaration is "no declaration". */
export function normalizeHostDeclaration(raw: unknown): HostDeclaration | undefined {
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return undefined
  const input = raw as Record<string, unknown>
  if (!Array.isArray(input.tools)) return undefined
  const declared = new Set(input.tools.filter((name): name is string => typeof name === 'string'))
  const platform = phrase(input.platform, 32)
  return {
    tools: MCP_TOOLS.map((tool) => tool.name).filter((name) => declared.has(name)),
    platform: /^[A-Za-z0-9_.-]+$/.test(platform) ? platform : 'unknown',
    place: phrase(input.place, 80) || DEFAULT_PLACE,
  }
}

export function sameHostDeclaration(a?: HostDeclaration, b?: HostDeclaration): boolean {
  if (!a || !b) return a === b
  return a.platform === b.platform && a.place === b.place && a.tools.join('\n') === b.tools.join('\n')
}

export function hostPlatformLabel(host: HostDeclaration): string {
  return PLATFORM_LABELS[host.platform] ?? ''
}

/** "the workspace host (Linux)"; the bare place when the platform is unknown. */
export function hostPlace(host: HostDeclaration): string {
  const label = hostPlatformLabel(host)
  return label ? `${host.place} (${label})` : host.place
}

/** Shell tools of the session: both for an undeclared host, the declared ones otherwise. */
export function hostShells(host?: HostDeclaration): string[] {
  return host ? SHELL_TOOLS.filter((name) => host.tools.includes(name)) : SHELL_TOOLS
}

export function hostHasTool(host: HostDeclaration | undefined, name: string): boolean {
  return !host || host.tools.includes(name)
}

/** A built-in tool the declared host cannot execute; never true without a declaration. */
export function isUndeclaredBuiltin(host: HostDeclaration | undefined, name: string): boolean {
  return !!host && !host.tools.includes(name) && MCP_TOOLS.some((tool) => tool.name === name)
}

export function undeclaredToolMessage(host: HostDeclaration, name: string): string {
  return `Tool ${name} is not available in this session: ${hostPlace(host)} did not declare it. Use one of the tools listed for this session.`
}

/** Host wording of one description string, tool-level or inside the input schema. */
function reword(text: string, host: HostDeclaration, shells: string[]): string {
  return text
    .replaceAll("on the user's Windows host", `on ${hostPlace(host)}`)
    .replaceAll("from the user's filesystem", `from the filesystem of ${host.place}`)
    .replaceAll('to the local filesystem', `to the filesystem of ${host.place}`)
    .replaceAll("on the user's machine", `on ${host.place}`)
    .replaceAll('Bash/PowerShell', shells.join('/') || 'a shell tool')
    .replaceAll('for Bash or PowerShell', shells.length ? `for ${shells.join(' or ')}` : 'for a shell tool')
}

function rewordSchema<T>(value: T, host: HostDeclaration, shells: string[]): T {
  if (typeof value === 'string') return reword(value, host, shells) as T
  if (Array.isArray(value)) return value.map((item) => rewordSchema(item, host, shells)) as T
  if (!value || typeof value !== 'object') return value
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [
      key,
      key === 'description' ? rewordSchema(item, host, shells) : walk(item, host, shells),
    ]),
  ) as T
}

/** Descends without touching enum values, defaults or property names. */
function walk<T>(value: T, host: HostDeclaration, shells: string[]): T {
  return value && typeof value === 'object' && !Array.isArray(value) ? rewordSchema(value, host, shells) : value
}

function describe(tool: McpTool, host: HostDeclaration, shells: string[]): string {
  let text: string = tool.description
  if (tool.name === 'PowerShell' && host.platform !== 'win32') {
    // The Windows-only first paragraph does not apply to pwsh on another OS.
    const notes = text.indexOf('\n\nNotes:')
    text =
      `Executes a PowerShell 7+ command on ${hostPlace(host)} and returns its output. This host is not Windows: registry, COM and Win32 operations are unavailable; use it when a task needs PowerShell itself.` +
      (notes < 0 ? '' : text.slice(notes))
  }
  return reword(text, host, shells)
}

/** Built-in tools served to the session: exactly the declared ones, worded for the declared place. */
export function mcpToolsFor(host?: HostDeclaration): readonly McpTool[] {
  if (!host) return MCP_TOOLS
  const shells = hostShells(host)
  return MCP_TOOLS.filter((tool) => host.tools.includes(tool.name)).map(
    (tool) =>
      ({
        ...tool,
        description: describe(tool, host, shells),
        inputSchema: walk(tool.inputSchema, host, shells),
      }) as McpTool,
  )
}
