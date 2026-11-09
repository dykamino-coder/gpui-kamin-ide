// Deterministic stdio peer, not an LSP implementation. Shared production code
// must discover, start and route every request to this separate process.
const fs = require('node:fs')
let buffer = Buffer.alloc(0)
const documents = new Map()
const trace = process.env.BRIDGEWEB_LSP_TRACE
function send(message) {
  const body = Buffer.from(JSON.stringify({ jsonrpc: '2.0', ...message }))
  process.stdout.write(Buffer.concat([Buffer.from(`Content-Length: ${body.length}\r\n\r\n`), body]))
}
function receive(message) {
  fs.appendFileSync(trace, JSON.stringify({ pid: process.pid, cwd: process.cwd(), ...message }) + '\n')
  const { method, params, id } = message
  if (method === 'exit') return process.exit(0)
  if (method === 'textDocument/didOpen' || method === 'textDocument/didChange') {
    const { uri } = params.textDocument
    documents.set(uri, method.endsWith('didOpen') ? params.textDocument.text : params.contentChanges[0].text)
    send({ method: 'textDocument/publishDiagnostics', params: { uri, diagnostics: [{
      range: { start: { line: 1, character: 2 }, end: { line: 1, character: 3 } },
      severity: 2, source: 'workspace-fixture', code: 'fixture-warning', message: documents.get(uri),
    }] } })
  }
  if (id === undefined) return
  let result = null
  if (method === 'initialize') result = { capabilities: { textDocumentSync: 1, hoverProvider: true, definitionProvider: true, referencesProvider: true } }
  if (method === 'textDocument/hover') result = { contents: { kind: 'markdown', value: `fixture hover: ${documents.get(params.textDocument.uri)}` } }
  const range = { start: { line: 1, character: 2 }, end: { line: 1, character: 3 } }
  if (method === 'textDocument/definition') result = [{ targetUri: params.textDocument.uri, targetRange: range, targetSelectionRange: range }]
  if (method === 'textDocument/references') result = [{ uri: params.textDocument.uri, range }]
  send({ id, result })
}
process.stdin.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk])
  while (true) {
    const end = buffer.indexOf('\r\n\r\n')
    if (end < 0) return
    const length = Number(/Content-Length: (\d+)/i.exec(buffer.subarray(0, end).toString())[1])
    if (buffer.length < end + 4 + length) return
    const message = JSON.parse(buffer.subarray(end + 4, end + 4 + length).toString())
    buffer = buffer.subarray(end + 4 + length)
    receive(message)
  }
})
