// Runs the shipping handlers in a separate process. No replacement filesystem
// or shell implementation: the harness supplies only an isolated cwd/profile.
import readline from 'node:readline'
import { performance } from 'node:perf_hooks'
import { readFile, writeFile, editFile, globSearch, grepSearch } from '../src/main/mcp/tools/file-system'
import { bashExecute, powerShellExecute } from '../src/main/mcp/tools/shell'
import { notebookEdit } from '../src/main/mcp/tools/notebook'
import { initBridgeState } from '../src/main/bridge-state'
import { MCP_TOOLS } from '../../server/src/core/pty/mcp-tools-defs'

const handlers = {
  Read: readFile, Write: writeFile, Edit: editFile, Glob: globSearch,
  Grep: grepSearch, Bash: bashExecute, PowerShell: powerShellExecute, NotebookEdit: notebookEdit,
}
initBridgeState(process.env.BENCH_PROFILE!)
const lines = readline.createInterface({ input: process.stdin })
async function main(): Promise<void> {
for await (const line of lines) {
  const request = JSON.parse(line)
  if (request.method === 'tools/list') {
    process.stdout.write(JSON.stringify({ id: request.id, tools: MCP_TOOLS.filter(t => t.name in handlers) }) + '\n')
    continue
  }
  const start = performance.now()
  try {
    const handler = handlers[request.name as keyof typeof handlers]
    if (!handler) throw new Error(`Unmeasured handler: ${request.name}`)
    const result = await handler(request.input, { cwd: request.cwd })
    process.stdout.write(JSON.stringify({ id: request.id, result, handlerMs: performance.now() - start }) + '\n')
  } catch (error) {
    process.stdout.write(JSON.stringify({ id: request.id, error: String(error) }) + '\n')
  }
}
}
void main().catch(error => { console.error(error); process.exitCode = 1 })
