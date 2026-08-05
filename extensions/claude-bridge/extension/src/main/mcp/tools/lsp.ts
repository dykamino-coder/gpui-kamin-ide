// MCP LSP tools — KaminIDE VSIX adapter.
//
// KaminIDE hosts language servers through its extension host. These tools reach
// the live providers via the canonical `vscode.execute*Provider` commands
// (registered host-side in src/exthost/lsp-commands.ts) and `vscode.languages
// .getDiagnostics`. When no provider matches the file (no language extension
// active, or the LSP server hasn't loaded that file), the provider result is
// empty and we return the same graceful "no server" result as the legacy client
// produced, so the model falls back to Grep/Read.
import path from 'path'
import * as vscode from 'vscode'
import type { McpResult, ToolContext } from '../tool-registry'
import { pluginLspManager } from '../../plugin-lsp'

interface LocationDto { uri: string; range: { startLine: number; startChar: number; endLine: number; endChar: number } }
interface HoverDto { contents: string[]; range?: { startLine: number; startChar: number; endLine: number; endChar: number } }

function errorResult(message: string): McpResult {
  return { content: [{ type: 'text', text: `Error: ${message}` }] }
}
function textResult(text: string): McpResult {
  return { content: [{ type: 'text', text }] }
}

function requireAbsolutePath(input: Record<string, unknown>, key = 'file'): string | null {
  const raw = input[key]
  if (typeof raw !== 'string' || !raw) return null
  return path.isAbsolute(raw) ? raw : null
}

const parseLine = (input: Record<string, unknown>): number => Math.max(0, Number(input.line ?? 0) || 0)
const parseChar = (input: Record<string, unknown>): number => Math.max(0, Number(input.character ?? 0) || 0)

const NO_SERVER = 'no LSP server is registered for this file extension'

export async function lspDiagnostics(input: Record<string, unknown>, context?: ToolContext): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  // getDiagnostics is real host-side — returns raw vscode.Diagnostic objects an
  // extension has already published for this file (empty is legitimate, not an
  // error: the LSP may not have loaded a file the user never opened).
  const diags = vscode.languages.getDiagnostics(vscode.Uri.file(file)) as Array<{
    range?: { start?: { line?: number; character?: number } }
    severity?: number; message?: string; source?: string; code?: unknown
  }>
  let pluginDiags: any[] = []
  try { pluginDiags = await pluginLspManager.getDiagnostics(file, context?.tabId) } catch { /* fall back to host providers */ }
  const combined = [...(diags ?? []), ...pluginDiags]
  if (combined.length === 0) return textResult(`No diagnostics for ${file}.`)
  const sev = ['Error', 'Warning', 'Info', 'Hint']
  const lines = combined.map((d) => {
    const line = (d.range?.start?.line ?? 0) + 1
    const col = (d.range?.start?.character ?? 0) + 1
    const label = sev[typeof d.severity === 'number' ? d.severity : 0] ?? 'Error'
    const src = d.source ? ` (${d.source})` : ''
    const code = d.code != null ? ` [${typeof d.code === 'object' ? JSON.stringify(d.code) : String(d.code)}]` : ''
    return `[${label}] ${file}:${line}:${col} ${d.message ?? ''}${src}${code}`
  })
  return textResult(lines.join('\n'))
}

// A hung/slow language server must not stall the MCP turn forever — race every
// provider request against a timeout and degrade to the graceful NO_SERVER path.
const LSP_TIMEOUT_MS = 15_000
async function execCmd<T>(command: string, ...args: unknown[]): Promise<T | undefined> {
  return Promise.race([
    vscode.commands.executeCommand(command, ...args) as Promise<T>,
    new Promise<undefined>((resolve) => setTimeout(() => resolve(undefined), LSP_TIMEOUT_MS)),
  ])
}

function hoverText(contents: any): string {
  const values = Array.isArray(contents) ? contents : [contents]
  return values.map((value) => {
    if (typeof value === 'string') return value
    if (value && typeof value.value === 'string') return value.value
    return ''
  }).filter(Boolean).join('\n')
}

export async function lspHover(input: Record<string, unknown>, context?: ToolContext): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const line = parseLine(input), character = parseChar(input)
  const uri = vscode.Uri.file(file)
  const pos = new vscode.Position(line, character)
  try {
    const pluginResults = await pluginLspManager.request(file, 'textDocument/hover', { position: { line, character } }, context?.tabId)
    const pluginBody = pluginResults.map(result => hoverText(result?.contents)).filter(Boolean).join('\n\n')
    if (pluginBody) return textResult(`Hover at ${file}:${line + 1}:${character + 1}\n\n${pluginBody}`)
  } catch { /* fall back to other VSIX providers */ }
  const hovers = await execCmd<HoverDto[]>('vscode.executeHoverProvider', uri, pos)
  if (!hovers || hovers.length === 0) return errorResult(NO_SERVER)
  const body = hovers.map((h) => (h.contents ?? []).join('\n')).filter(Boolean).join('\n\n')
  if (!body.trim()) return errorResult(NO_SERVER)
  return textResult(`Hover at ${file}:${line + 1}:${character + 1}\n\n${body}`)
}

async function locations(command: string, file: string, line: number, character: number): Promise<LocationDto[]> {
  const uri = vscode.Uri.file(file)
  const pos = new vscode.Position(line, character)
  return (await execCmd<LocationDto[]>(command, uri, pos)) ?? []
}

function formatLocations(locs: LocationDto[]): string {
  return locs.map((l) => `${l.uri}:${(l.range?.startLine ?? 0) + 1}:${(l.range?.startChar ?? 0) + 1}`).join('\n')
}

function normalizePluginLocations(results: any[]): LocationDto[] {
  return results.flatMap((result) => {
    const items = Array.isArray(result) ? result : result ? [result] : []
    return items.map((item: any) => {
      if (typeof item?.uri === 'string' && item.range) {
        return {
          uri: item.uri,
          range: {
            startLine: item.range.start?.line ?? 0,
            startChar: item.range.start?.character ?? 0,
            endLine: item.range.end?.line ?? 0,
            endChar: item.range.end?.character ?? 0,
          },
        }
      }
      if (typeof item?.targetUri === 'string' && item.targetSelectionRange) {
        return {
          uri: item.targetUri,
          range: {
            startLine: item.targetSelectionRange.start?.line ?? 0,
            startChar: item.targetSelectionRange.start?.character ?? 0,
            endLine: item.targetSelectionRange.end?.line ?? 0,
            endChar: item.targetSelectionRange.end?.character ?? 0,
          },
        }
      }
      return null
    }).filter((value: LocationDto | null): value is LocationDto => value !== null)
  })
}

export async function lspDefinition(input: Record<string, unknown>, context?: ToolContext): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const line = parseLine(input), character = parseChar(input)
  let locs: LocationDto[] = []
  try { locs = normalizePluginLocations(await pluginLspManager.request(file, 'textDocument/definition', { position: { line, character } }, context?.tabId)) } catch {}
  if (locs.length === 0) locs = await locations('vscode.executeDefinitionProvider', file, line, character)
  if (locs.length === 0) return errorResult(NO_SERVER)
  return textResult(formatLocations(locs))
}

export async function lspReferences(input: Record<string, unknown>, context?: ToolContext): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const line = parseLine(input), character = parseChar(input)
  let locs: LocationDto[] = []
  try {
    locs = normalizePluginLocations(await pluginLspManager.request(file, 'textDocument/references', {
      position: { line, character }, context: { includeDeclaration: true },
    }, context?.tabId))
  } catch {}
  if (locs.length === 0) locs = await locations('vscode.executeReferenceProvider', file, line, character)
  if (locs.length === 0) return errorResult(NO_SERVER)
  return textResult(formatLocations(locs))
}
