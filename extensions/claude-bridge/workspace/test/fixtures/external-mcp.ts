import assert from 'node:assert/strict'
import { createServer } from 'node:http'

/** Real loopback HTTP MCP transport. It deliberately never finishes tools/call. */
export async function externalMcp() {
  let entered!: () => void
  let closed!: () => void
  const started = new Promise<void>(resolve => { entered = resolve })
  const cancelled = new Promise<void>(resolve => { closed = resolve })
  const server = createServer(async (request, response) => {
    const chunks: Buffer[] = []
    for await (const chunk of request) chunks.push(chunk)
    const frame = JSON.parse(Buffer.concat(chunks).toString('utf8'))
    response.setHeader('Content-Type', 'application/json')
    if (frame.method === 'tools/call') {
      // Headers arrive, but the body stays open: cancellation must cover both.
      response.writeHead(200)
      response.write('{"jsonrpc":"2.0",')
      response.on('close', closed)
      entered()
      return
    }
    const result = frame.method === 'initialize'
      ? { protocolVersion: '2024-11-05', capabilities: {}, serverInfo: { name: 'fixture', version: '1' } }
      : frame.method === 'tools/list'
        ? { tools: [{ name: 'fixture_wait', description: 'wait for shutdown', inputSchema: { type: 'object' } }] }
        : {}
    response.end(JSON.stringify({ jsonrpc: '2.0', id: frame.id, result }))
  })
  await new Promise<void>(resolve => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as { port: number }
  return {
    url: `http://127.0.0.1:${port}/mcp`, started, cancelled,
    async close() {
      server.closeAllConnections()
      await new Promise<void>(resolve => server.close(() => resolve()))
    },
  }
}

export async function deadline<T>(promise: Promise<T>, message: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout>
  try {
    return await Promise.race([promise, new Promise<never>((_resolve, reject) => {
      timer = setTimeout(() => reject(new Error(message)), 5000)
    })])
  } finally { clearTimeout(timer!) }
}

export async function assertProcessStopped(pid: number, label: string) {
  for (let attempt = 0; attempt < 200; attempt++) {
    try { process.kill(pid, 0) } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error
      return
    }
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  assert.fail(`terminal disposal must stop the owned ${label} shell process`)
}
