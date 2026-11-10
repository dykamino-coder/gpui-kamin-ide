import assert from 'node:assert/strict'
import { appendFile, readFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import path from 'node:path'

// Owned loopback MCP peer. Its disk journal is independent of host replies and
// lets the browser test detect lost, duplicated or misrouted calls.
export async function browserMcp(t, root, label) {
  const journal = path.join(root, `mcp-${label}.jsonl`)
  const resource = path.join(root, 'mcp-resource.txt')
  const requests = []
  const errors = []
  const tools = ['fixture_record', 'fixture_second'].map(name => ({ name,
    description: `Synthetic ${label} tool`, inputSchema: { type: 'object', properties: { nonce: { type: 'string' } }, required: ['nonce'] } }))
  const resources = [1, 2].map(i => ({ uri: `fixture://${label}/${i}`, name: `Resource ${i}`, mimeType: 'text/plain' }))
  const templates = [1, 2].map(i => ({ uriTemplate: `fixture://${label}/template-${i}/{key}`, name: `Template ${i}` }))
  const prompts = [1, 2].map(i => ({ name: `prompt_${i}`, arguments: [{ name: 'topic', required: true }] }))
  const lists = { 'tools/list': ['tools', tools], 'resources/list': ['resources', resources],
    'resources/templates/list': ['resourceTemplates', templates], 'prompts/list': ['prompts', prompts] }
  const server = createServer((req, res) => {
    void (async () => {
      assert.equal(req.method, 'POST')
      let raw = ''
      for await (const chunk of req) raw += chunk
      const message = JSON.parse(raw)
      requests.push(message)
      if (message.id === undefined) { res.writeHead(202); res.end(); return }
      let result
      if (message.method === 'initialize') result = { protocolVersion: '2024-11-05',
        capabilities: { tools: {}, resources: {}, prompts: {} }, serverInfo: { name: `fixture-${label}`, version: '1' } }
      else if (lists[message.method]) {
        const [key, items] = lists[message.method]
        assert.ok(message.params?.cursor === undefined || message.params.cursor === 'second')
        const second = message.params?.cursor === 'second'
        result = { [key]: [items[second ? 1 : 0]], ...(!second ? { nextCursor: 'second' } : {}) }
      } else if (message.method === 'resources/read') {
        assert.ok(message.params.uri.startsWith(`fixture://${label}/`))
        result = { contents: [{ uri: message.params.uri, mimeType: 'text/plain', text: await readFile(resource, 'utf8') }] }
      } else if (message.method === 'prompts/get') {
        assert.ok(prompts.some(prompt => prompt.name === message.params.name))
        result = { messages: [{ role: 'user', content: { type: 'text', text: `${label}:${message.params.arguments.topic}` } }] }
      } else if (message.method === 'tools/call') {
        assert.ok(tools.some(tool => tool.name === message.params.name))
        assert.equal(typeof message.params.arguments.nonce, 'string')
        await appendFile(journal, JSON.stringify(message.params) + '\n')
        result = { content: [{ type: 'text', text: `${label}:${message.params.arguments.nonce}` }] }
        // Progress travels through the real shared HTTP transport's SSE parser.
        res.writeHead(200, { 'Content-Type': 'text/event-stream' })
        res.end(`data: ${JSON.stringify({ jsonrpc: '2.0', method: 'notifications/progress', params: {
          progressToken: message.params.arguments.nonce, progress: 1, total: 1, message: 'Synthetic tool complete',
        } })}\n\ndata: ${JSON.stringify({ jsonrpc: '2.0', id: message.id, result })}\n\n`)
        return
      } else throw new Error('Unexpected synthetic MCP method')
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ jsonrpc: '2.0', id: message.id, result }))
    })().catch(error => {
      errors.push(error)
      res.writeHead(500); res.end('Synthetic MCP fixture assertion failed')
    })
  })
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
  t.after(async () => {
    await new Promise(resolve => server.close(resolve))
    assert.deepEqual(errors, [], 'Every received MCP request must satisfy the independent fixture assertions')
  })
  return { url: `http://127.0.0.1:${server.address().port}/mcp`, journal, requests, resources, prompts, templates }
}
