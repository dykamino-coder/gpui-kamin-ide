// Checkout-only offline stand-in for the two network peers of `codex app-server`:
// an OpenAI Responses API endpoint (SSE) and a Bridge MCP endpoint (streamable
// HTTP, JSON responses). It never contacts the network, never executes the
// tool arguments it receives and records every request so tests can assert on
// exactly what the model was offered and what came back.
import http from 'node:http'

const sse = (events) => events.map((ev) => `event: ${ev.type}\ndata: ${JSON.stringify(ev)}\n\n`).join('')

const DEFAULT_MCP_TOOLS = [
  {
    name: 'Read',
    description: 'Read a file on the user machine (synthetic)',
    inputSchema: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'] },
  },
  {
    name: 'Bash',
    description: 'Run a command on the user machine (synthetic)',
    inputSchema: { type: 'object', properties: { command: { type: 'string' } }, required: ['command'] },
  },
]

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    req.on('data', (c) => chunks.push(c))
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    req.on('error', reject)
  })
}

/** Finds the namespace under which codex exposes an MCP tool in this request. */
function mcpNamespaceFor(body, tool) {
  for (const t of body.tools ?? []) {
    if (t.type === 'namespace' && (t.tools ?? []).some((x) => x.name === tool)) return t.name
  }
  return undefined
}

function outputFor(step, n, index, body) {
  const id = `${step.type}_${n}_${index}`
  if (step.type === 'message') {
    const chunks = step.chunks ?? [step.text]
    return [
      { type: 'response.output_item.added', item: { type: 'message', role: 'assistant', id, content: [] } },
      ...chunks.map((delta) => ({ type: 'response.output_text.delta', item_id: id, delta })),
      {
        type: 'response.output_item.done',
        item: { type: 'message', role: 'assistant', id, content: [{ type: 'output_text', text: chunks.join('') }] },
      },
    ]
  }
  if (step.type === 'reasoning') {
    const summary = (step.summary ?? []).map((text) => ({ type: 'summary_text', text }))
    return [
      { type: 'response.output_item.added', item: { type: 'reasoning', id, summary: [] } },
      ...(step.summary ?? []).map((delta, i) => ({
        type: 'response.reasoning_summary_text.delta',
        item_id: id,
        delta,
        summary_index: i,
      })),
      { type: 'response.output_item.done', item: { type: 'reasoning', id, summary, encrypted_content: null } },
    ]
  }
  if (step.type === 'call' || step.type === 'mcp-call') {
    const namespace = step.type === 'mcp-call' ? mcpNamespaceFor(body, step.name) : step.namespace
    const item = {
      type: 'function_call',
      id: `fc_${n}_${index}`,
      call_id: step.callId ?? `call_${n}_${index}`,
      name: step.name,
      arguments: JSON.stringify(step.arguments ?? {}),
    }
    if (namespace) item.namespace = namespace
    return [{ type: 'response.output_item.done', item }]
  }
  if (step.type === 'custom') {
    // Freeform tool call, e.g. code mode's `exec` (JavaScript source as input).
    const item = {
      type: 'custom_tool_call',
      id: `ctc_${n}_${index}`,
      call_id: step.callId ?? `call_${n}_${index}`,
      name: step.name,
      input: step.input,
    }
    if (step.namespace) item.namespace = step.namespace
    return [{ type: 'response.output_item.done', item }]
  }
  throw new Error(`Unknown mock step ${step.type}`)
}

/**
 * @param {{
 *   script?: Array<Array<object> | { fail: { code: string, message: string } }>,
 *   scripts?: Record<string, Array<Array<object> | { fail: { code: string, message: string } }>>,
 *   mcpToken?: string,
 *   mcpTools?: Array<object>,
 *   usage?: { input: number, output: number },
 * }} options
 */
export async function startCodexMock(options = {}) {
  const script = [...(options.script ?? [])]
  // Per-conversation scripts for parallel threads: the key is a marker that
  // appears only in that conversation's user input.
  const scripts = Object.fromEntries(Object.entries(options.scripts ?? {}).map(([k, v]) => [k, [...v]]))
  const queueFor = (body) => {
    const text = JSON.stringify(body.input ?? [])
    const key = Object.keys(scripts).find((k) => text.includes(k))
    return key ? scripts[key] : script
  }
  const mcpTools = options.mcpTools ?? DEFAULT_MCP_TOOLS
  const usage = options.usage ?? { input: 10, output: 2 }
  /** @type {Array<{ path: string, body: any }>} */
  const responsesRequests = []
  /** @type {Array<{ path: string, authorized: boolean, authorization?: string, body: any }>} */
  const mcpRequests = []
  let counter = 0

  const server = http.createServer(async (req, res) => {
    const n = ++counter
    const url = new URL(req.url ?? '/', 'http://127.0.0.1')
    try {
      if (url.pathname.startsWith('/mcp/')) {
        const authorized = !options.mcpToken || req.headers.authorization === `Bearer ${options.mcpToken}`
        if (req.method === 'DELETE') {
          res.writeHead(200).end()
          return
        }
        if (req.method !== 'POST') {
          res.writeHead(405).end()
          return
        }
        const raw = await readBody(req)
        const msg = raw ? JSON.parse(raw) : {}
        // Tests use synthetic tokens only; the header is kept to prove per-thread isolation.
        mcpRequests.push({ path: url.pathname, authorized, authorization: req.headers.authorization, body: msg })
        if (!authorized) {
          res.writeHead(401).end()
          return
        }
        if (msg.id === undefined) {
          res.writeHead(202).end()
          return
        }
        let result = {}
        if (msg.method === 'initialize') {
          result = {
            protocolVersion: msg.params?.protocolVersion ?? '2025-03-26',
            capabilities: { tools: {} },
            serverInfo: { name: 'bridge-mock', version: '0' },
          }
        } else if (msg.method === 'tools/list') {
          result = { tools: mcpTools }
        } else if (msg.method === 'tools/call') {
          result = { content: [{ type: 'text', text: `MCP-OK ${msg.params?.name}` }] }
        }
        const data = JSON.stringify({ jsonrpc: '2.0', id: msg.id, result })
        res.writeHead(200, { 'content-type': 'application/json', 'mcp-session-id': 'mock-session' }).end(data)
        return
      }
      if (req.method === 'GET' && url.pathname.endsWith('/models')) {
        res.writeHead(200, { 'content-type': 'application/json' }).end('{"models":[]}')
        return
      }
      if (req.method === 'POST' && url.pathname.endsWith('/responses')) {
        const body = JSON.parse(await readBody(req))
        responsesRequests.push({ path: url.pathname, body })
        const rid = `resp_${n}`
        const next = queueFor(body).shift()
        const events = [{ type: 'response.created', response: { id: rid } }]
        if (next && !Array.isArray(next) && next.fail) {
          events.push({ type: 'response.failed', response: { id: rid, error: next.fail } })
        } else {
          const steps = next ?? [{ type: 'message', text: 'done' }]
          steps.forEach((step, i) => events.push(...outputFor(step, n, i, body)))
          events.push({
            type: 'response.completed',
            response: {
              id: rid,
              usage: {
                input_tokens: usage.input,
                input_tokens_details: null,
                output_tokens: usage.output,
                output_tokens_details: null,
                total_tokens: usage.input + usage.output,
              },
            },
          })
        }
        const data = sse(events)
        res.writeHead(200, { 'content-type': 'text/event-stream', 'content-length': Buffer.byteLength(data) }).end(data)
        return
      }
      res.writeHead(404).end()
    } catch (error) {
      res.writeHead(500).end(String(error))
    }
  })
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
  const port = /** @type {import('node:net').AddressInfo} */ (server.address()).port
  return {
    port,
    responsesBaseUrl: `http://127.0.0.1:${port}/v1`,
    mcpUrl: (id) => `http://127.0.0.1:${port}/mcp/${id}`,
    responsesRequests,
    mcpRequests,
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections?.()
        server.close(() => resolve(undefined))
      }),
  }
}
