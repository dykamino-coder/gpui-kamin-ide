// MCP LSP tools — KaminIDE VSIX adapter.
//
// KaminIDE hosts language servers through its extension host. These tools reach
// the live providers via the canonical `vscode.execute*Provider` commands
// (registered host-side in src/exthost/lsp-commands.ts) and `vscode.languages
// .getDiagnostics`. When no provider matches the file (no language extension
// active, or the LSP server hasn't loaded that file), the provider result is
// empty and we return the SAME graceful "no server" result the Electron app
// produced, so the model falls back to Grep/Read.
import path from 'path'
import * as vscode from 'vscode'
import type { McpResult } from '../tool-registry'

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

export async function lspDiagnostics(input: Record<string, unknown>): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  // getDiagnostics is real host-side — returns raw vscode.Diagnostic objects an
  // extension has already published for this file (empty is legitimate, not an
  // error: the LSP may not have loaded a file the user never opened).
  const diags = vscode.languages.getDiagnostics(vscode.Uri.file(file)) as Array<{
    range?: { start?: { line?: number; character?: number } }
    severity?: number; message?: string; source?: string; code?: unknown
  }>
  if (!diags || diags.length === 0) return textResult(`No diagnostics for ${file}.`)
  const sev = ['Error', 'Warning', 'Info', 'Hint']
  const lines = diags.map((d) => {
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

export async function lspHover(input: Record<string, unknown>): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const line = parseLine(input), character = parseChar(input)
  const uri = vscode.Uri.file(file)
  const pos = new vscode.Position(line, character)
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

export async function lspDefinition(input: Record<string, unknown>): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const locs = await locations('vscode.executeDefinitionProvider', file, parseLine(input), parseChar(input))
  if (locs.length === 0) return errorResult(NO_SERVER)
  return textResult(formatLocations(locs))
}

export async function lspReferences(input: Record<string, unknown>): Promise<McpResult> {
  const file = requireAbsolutePath(input)
  if (!file) return errorResult('file must be an absolute path')
  const locs = await locations('vscode.executeReferenceProvider', file, parseLine(input), parseChar(input))
  if (locs.length === 0) return errorResult(NO_SERVER)
  return textResult(formatLocations(locs))
}
