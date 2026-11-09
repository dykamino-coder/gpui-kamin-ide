// Disposable stdio MCP server. A model tool call leaves independently verifiable bytes.
import readline from 'node:readline'
import { appendFileSync } from 'node:fs'
const send = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`)
readline.createInterface({ input: process.stdin }).on('line', line => {
  const request = JSON.parse(line)
  if (request.id === undefined) return
  if (request.method === 'initialize') return send(request.id, {
    protocolVersion: request.params.protocolVersion, capabilities: { tools: {} }, serverInfo: { name: 'acceptance', version: '1' },
  })
  if (request.method === 'tools/list') return send(request.id, { tools: [{ name: 'acceptance_record',
    description: 'Record the acceptance nonce in the workspace.',
    inputSchema: { type: 'object', properties: { nonce: { type: 'string' } }, required: ['nonce'] } }] })
  if (request.method === 'tools/call' && request.params.name === 'acceptance_record') {
    appendFileSync('/workspace/bridgeweb-acc-mcp-result', `${request.params.arguments.nonce}\n`)
    return send(request.id, { content: [{ type: 'text', text: 'recorded' }] })
  }
  if (request.method === 'ping') return send(request.id, {})
  process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id: request.id, error: { code: -32601, message: 'Unsupported fixture method' } })}\n`)
})
