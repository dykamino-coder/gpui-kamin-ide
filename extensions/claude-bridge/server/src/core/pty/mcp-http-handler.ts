// ============================================================================
// MCP HTTP Handler — receives JSON-RPC from Claude CLI, routes to client host
// ============================================================================
//
// Claude CLI spawned with settings.json pointing MCP server to:
//   http://localhost:3456/mcp/<sessionId>
//
// This handler:
// 1. Receives JSON-RPC requests (tools/list, tools/call, initialize)
// 2. For tools/call: forwards to the VSIX bridge host via WS, waits for response
// 3. For tools/list: returns the list of MCP tools
// 4. For initialize: returns server info

import type { Context } from 'hono'
import crypto from 'crypto'
import { debugLog, warnLog, errorLog } from '../logging'
import { getSession, sendMcpCall } from './session-manager'
import { eventBus } from '../events/bus'
import type { PtySession, ToolDefinition } from './types'
import { getSessionWs } from './session-ws'
import { sendToClient } from './session-io'
import { MCP_TOOLS } from './mcp-tools-defs'
import { matchesResourceTemplate } from './resource-template'

export { MCP_TOOLS } from './mcp-tools-defs'

const MCP_LOG_MAX = 50 // keep last N entries per session

// Claude CLI's MAX_MCP_OUTPUT_TOKENS is set to 200k in session-manager.ts.
// We truncate ~10% below that (≈180k tokens ≈ 720 KB, 4 chars/token) so the
// CLI never triggers its spill-to-file behavior. The spill file is written
// inside the CLI's process (our container) using the container's cwd — the
// model would then Read() it, which runs on the *client* VSIX bridge host and
// can't see /home/bridge/... paths. Truncating on our side keeps the whole
// result inline and preserves client-server separation.
const MCP_RESULT_HARD_CAP_BYTES = 720_000
// Reserve space for the [Output truncated…] marker so the notice itself
// never pushes us past the cap. Budget is generous — marker lines are ~400
// chars, 2 KB leaves headroom for tool-name formatting and future notes.
const TRUNCATION_NOTICE_RESERVE = 2_048
const DEFAULT_TRUNCATION_BUDGET = MCP_RESULT_HARD_CAP_BYTES - TRUNCATION_NOTICE_RESERVE
// Per MCP 2.1.91 a tool can opt into a higher per-result cap by attaching
// `_meta["anthropic/maxResultSizeChars"]` (max 500K) to the result. We honor
// it up to MCP_PER_RESULT_CEILING (matching CLI's 500K limit) for tools like
// DB-schema dumps that want to pass full output through.
const MCP_PER_RESULT_CEILING = 500_000
export function truncateMcpResult(result: Record<string, unknown>, toolName: string): Record<string, unknown> {
  const content = result.content as Array<Record<string, unknown>> | undefined
  if (!Array.isArray(content)) return result

  // Honor `_meta["anthropic/maxResultSizeChars"]` opt-in (MCP 2.1.91+).
  // Falls back to default cap when absent or out of range.
  const meta = result._meta as Record<string, unknown> | undefined
  const metaCap = meta?.['anthropic/maxResultSizeChars']
  let budget = DEFAULT_TRUNCATION_BUDGET
  if (typeof metaCap === 'number' && Number.isFinite(metaCap) && metaCap > 0) {
    budget = Math.min(metaCap, MCP_PER_RESULT_CEILING) - TRUNCATION_NOTICE_RESERVE
  }
  let remaining = budget
  let truncated = false
  let droppedBytes = 0
  const newContent: Array<Record<string, unknown>> = []
  for (const block of content) {
    if (block.type !== 'text' || typeof block.text !== 'string') {
      newContent.push(block)
      continue
    }
    const text = block.text as string
    if (text.length <= remaining) {
      newContent.push(block)
      remaining -= text.length
      continue
    }
    if (remaining > 0) {
      newContent.push({ ...block, text: text.slice(0, remaining) })
      droppedBytes += text.length - remaining
      remaining = 0
    } else {
      droppedBytes += text.length
    }
    truncated = true
  }
  if (truncated) {
    newContent.push({
      type: 'text',
      text: `\n\n[Output truncated by bridge — kept ${Math.round(budget / 1000)} KB, dropped ~${Math.ceil(droppedBytes / 1000)} KB. Tool: ${toolName}. This cap protects against Claude CLI spilling to a file the client can't read. Narrow the query (path, pattern, -C, head_limit / offset) to see more.]`,
    })
    return { ...result, content: newContent }
  }
  return result
}

function logMcpRequest(session: PtySession, method: string, status: 'ok' | 'error', error?: string, durationMs?: number): void {
  session.mcpLog.push({ ts: Date.now(), method, status, error, durationMs })
  if (session.mcpLog.length > MCP_LOG_MAX) {
    session.mcpLog = session.mcpLog.slice(-MCP_LOG_MAX)
  }
  if (status === 'error' && error) {
    session.mcpLastError = error
  }
}

// ---------------------------------------------------------------------------
// MCP Tool definitions live in `mcp-tools-defs.ts` (extracted in Sprint 2 /
// Stage C, C5). MCP_TOOL_NAMES stays here as the dependency-free derivation
// other modules import — keeping it next to the JSON-RPC handler also makes
// it the natural pickup point for tools/list responses.
// ---------------------------------------------------------------------------

/** Names of every tool we advertise via tools/list. Derived from MCP_TOOLS so
 *  there's only one source of truth — when you add a tool to MCP_TOOLS it
 *  automatically appears here, and `session-core.ts:DISALLOWED_BUILTIN_TOOLS`
 *  picks it up to deny the matching CLI native (so the model uses our MCP
 *  version instead of the CLI's own implementation). */
export const MCP_TOOL_NAMES: readonly string[] = MCP_TOOLS.map(t => t.name)


// ---------------------------------------------------------------------------
// JSON-RPC helpers
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// JSON-RPC request types — discriminated by `method`. Sprint 2 / Stage B (B1).
// Replaces the `JsonRpcRequest { method: string }` shape that left every
// `body.params` typed as `Record<string, unknown>`. Each variant declares the
// concrete params shape the MCP spec defines for that method, so the switch
// in `handleMcpRequest` narrows automatically and tool-name / arg lookups
// stop needing `as any` casts.
//
// Spec reference:
//   https://modelcontextprotocol.io/specification/2024-11-05/server/
// ---------------------------------------------------------------------------

interface JsonRpcEnvelope {
  jsonrpc: '2.0'
  id?: string | number
}

export interface InitializeRequest extends JsonRpcEnvelope {
  method: 'initialize'
  params?: {
    protocolVersion?: string
    capabilities?: Record<string, unknown>
    clientInfo?: { name?: string; version?: string }
  }
}

export interface ToolsListRequest extends JsonRpcEnvelope {
  method: 'tools/list'
  params?: { cursor?: string }
}

export interface ToolsCallRequest extends JsonRpcEnvelope {
  method: 'tools/call'
  params?: {
    name: string
    arguments?: Record<string, unknown>
  }
}

export interface ResourcesListRequest extends JsonRpcEnvelope {
  method: 'resources/list'
  params?: { cursor?: string }
}

export interface ResourcesReadRequest extends JsonRpcEnvelope {
  method: 'resources/read'
  params?: { uri: string }
}

export interface PromptsListRequest extends JsonRpcEnvelope {
  method: 'prompts/list'
  params?: { cursor?: string }
}

export interface PromptsGetRequest extends JsonRpcEnvelope {
  method: 'prompts/get'
  params?: { name: string; arguments?: Record<string, unknown> }
}

export interface NotificationRequest extends JsonRpcEnvelope {
  method: `notifications/${string}`
  params?: Record<string, unknown>
}

/** Fallback shape — anything we don't have a concrete variant for yet. */
export interface UnknownRequest extends JsonRpcEnvelope {
  method: string
  params?: Record<string, unknown>
}

export type JsonRpcRequest =
  | InitializeRequest
  | ToolsListRequest
  | ToolsCallRequest
  | ResourcesListRequest
  | ResourcesReadRequest
  | PromptsListRequest
  | PromptsGetRequest
  | NotificationRequest
  | UnknownRequest

export function jsonRpcResponse(id: string | number | undefined, result: unknown) {
  return { jsonrpc: '2.0', id, result }
}

export function jsonRpcError(id: string | number | undefined, code: number, message: string) {
  return { jsonrpc: '2.0', id, error: { code, message } }
}

/** Constant-time string equality for the bearer-token check. A length mismatch
 *  short-circuits (the expected `Bearer <uuid>` length is fixed, so this leaks
 *  nothing useful); equal-length inputs go through crypto.timingSafeEqual. */
function timingSafeEqualStr(actual: string | undefined, expected: string): boolean {
  if (typeof actual !== 'string') return false
  const a = Buffer.from(actual)
  const b = Buffer.from(expected)
  if (a.length !== b.length) return false
  return crypto.timingSafeEqual(a, b)
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/**
 * Handle MCP HTTP request from Claude CLI.
 * Route: POST /mcp/:sessionId
 *
 * Implements MCP Streamable HTTP transport:
 * - Requests (with id): return JSON-RPC response with Mcp-Session-Id header
 * - Notifications (without id): return 202 Accepted
 */
export async function handleMcpRequest(c: Context): Promise<Response> {
  const sessionId = c.req.param('sessionId')
  if (!sessionId) {
    return c.json(jsonRpcError(undefined, -32000, 'sessionId is required'), 400)
  }

  // Validate session exists
  const session = getSession(sessionId)
  if (!session) {
    return c.json(
      jsonRpcError(undefined, -32000, `Session ${sessionId} not found`),
      404
    )
  }

  // Validate MCP auth token — prevents external access to tool endpoints.
  // Constant-time compare so a co-located attacker can't recover the per-session
  // token byte-by-byte via response-timing (uuid entropy makes this impractical
  // anyway, but timing-safe is the correct default for a bearer check).
  const authHeader = c.req.header('authorization')
  const expectedToken = `Bearer ${session.mcpToken}`
  if (!timingSafeEqualStr(authHeader, expectedToken)) {
    warnLog('MCP auth failed', { sessionId, hasAuth: !!authHeader })
    return c.json(jsonRpcError(undefined, -32000, 'Unauthorized'), 401)
  }

  let body: JsonRpcRequest
  try {
    body = await c.req.json()
  } catch {
    return c.json(jsonRpcError(undefined, -32700, 'Parse error'), 400)
  }

  debugLog('MCP request', { sessionId, method: body.method, id: body.id })
  const reqStart = Date.now()

  const sid: string = sessionId
  // Helper: return JSON response with Mcp-Session-Id header (required by Streamable HTTP spec)
  function jsonResponse(data: unknown, status = 200): Response {
    const res = c.json(data, status as 200)
    res.headers.set('Mcp-Session-Id', sid)
    return res
  }

  /**
   * Return an SSE response that flushes headers immediately and streams the
   * final JSON-RPC response when `work()` settles. Use this for any handler
   * branch that may exceed CLI's hardcoded 60-second per-POST fetch timeout
   * (`wrapFetchWithTimeout` in `services/mcp/client.ts`). Without this,
   * `Bash`/`Grep`/`Glob` runs that exceed 60s die with "The operation timed
   * out" on the client because `fetch()` aborts before our `c.json()` can
   * write headers.
   *
   * Per the MCP Streamable HTTP spec, servers MAY respond to a POST with
   * `Content-Type: text/event-stream`, deliver the JSON-RPC response as a
   * `message` SSE event, and close the stream. CLI's
   * `StreamableHTTPClientTransport` parses this exactly the same way as a
   * plain JSON response from the caller's perspective.
   */
  function sseStreamResponse(
    jsonRpcId: string | number | null,
    work: () => Promise<unknown>,
  ): Response {
    const encoder = new TextEncoder()
    // 25s < CLI's 60s fetch timeout, and well under any commonly seen corp
    // proxy idle-drop window. Spec-allowed `:`-comment lines are ignored by
    // SDK SSE parser but force a chunk on the wire.
    const HEARTBEAT_MS = 25_000

    // Hoisted so cancel() (client disconnect) can stop the keepalive too.
    let closed = false
    let heartbeat: ReturnType<typeof setInterval> | null = null
    const clearHeartbeat = (): void => { if (heartbeat) { clearInterval(heartbeat); heartbeat = null } }

    const stream = new ReadableStream<Uint8Array>({
      async start(controller) {
        const safeEnqueue = (bytes: Uint8Array): void => {
          if (closed) return
          // A throw here means the consumer went away between our writes —
          // mark closed AND kill the keepalive so it doesn't fire uselessly for
          // the rest of a long (up-to-30-min) tool run.
          try { controller.enqueue(bytes) }
          catch { closed = true; clearHeartbeat() }
        }
        const safeClose = (): void => {
          if (closed) return
          closed = true
          try { controller.close() } catch { /* ignore */ }
        }

        // Force headers + first byte onto the wire immediately so the
        // client's fetch() promise resolves and any per-fetch timeout
        // (CLI's 60s wrapper) is cleared.
        safeEnqueue(encoder.encode(': open\n\n'))

        heartbeat = setInterval(() => safeEnqueue(encoder.encode(': keepalive\n\n')), HEARTBEAT_MS)

        try {
          const payload = await work()
          const line = `event: message\ndata: ${JSON.stringify(payload)}\n\n`
          safeEnqueue(encoder.encode(line))
        } catch (err) {
          const errMsg = err instanceof Error ? err.message : String(err)
          warnLog('SSE stream handler error', { sessionId, error: errMsg })
          const errPayload = jsonRpcError(jsonRpcId ?? undefined, -32603, errMsg)
          const line = `event: message\ndata: ${JSON.stringify(errPayload)}\n\n`
          safeEnqueue(encoder.encode(line))
        } finally {
          clearHeartbeat()
          safeClose()
        }
      },
      cancel() {
        // The CLI aborted / disconnected the fetch. Stop the keepalive so it
        // doesn't tick into a dead stream for the rest of a long tool run. We
        // deliberately DON'T tear anything else down: work() runs to completion
        // on the client and its result is discarded (safeEnqueue no-ops once
        // closed). Destroying the pipe here is what historically resurrected the
        // <synthetic> resume bug.
        closed = true
        clearHeartbeat()
      },
    })

    return new Response(stream, {
      status: 200,
      headers: {
        'Content-Type': 'text/event-stream',
        'Cache-Control': 'no-cache, no-transform',
        'Connection': 'keep-alive',
        'X-Accel-Buffering': 'no',  // disable nginx/proxy buffering if present
        'Mcp-Session-Id': sid,
      },
    })
  }

  switch (body.method) {
    case 'initialize': {
      // Read client's requested protocol version
      const params = body.params as { protocolVersion?: string } | undefined
      const clientVersion = params?.protocolVersion || '2024-11-05'

      session.mcpInitialized = true
      logMcpRequest(session, 'initialize', 'ok', undefined, Date.now() - reqStart)

      // Notify dashboard that MCP is now initialized
      eventBus.emit('session:updated', {
        sessionId,
        mcpCallCount: session.mcpCallCount,
        mcpInitialized: true,
        mcpLastError: session.mcpLastError,
        lastActivityAt: session.lastActivityAt.toISOString(),
      })

      return jsonResponse(jsonRpcResponse(body.id, {
        protocolVersion: clientVersion,
        capabilities: {
          tools: {},
          resources: {},
          prompts: {},
          elicitation: {},
        },
        serverInfo: {
          name: 'open-claude-bridge-user-tools',
          version: '4.0.0',
        },
        instructions: `This MCP server provides tools that execute on the user's local machine. All file, shell, and search operations run locally, not on the server.

Key tool capabilities:
- **Read**: Reads any file. Supports images (PNG, JPG, GIF, WebP) — image contents are presented visually. Also reads PDFs (use pages parameter for large files) and Jupyter notebooks (.ipynb). ALWAYS use Read to view screenshots or image files the user mentions.
- **Write**: Creates or overwrites files. Use Read first for existing files. Prefer Edit for modifications.
- **Edit**: Exact string replacement in files. Must Read the file first. Use replace_all for bulk replacements.
- **Glob**: Fast file pattern matching (e.g. "**/*.ts"). Returns paths sorted by modification time.
- **Grep**: Powerful ripgrep-based search. ALWAYS use this instead of grep/rg via Bash.
- **Bash**: Execute shell commands. Default timeout 25 min, max 30 min. For long-running jobs pass run_in_background:true, then read output with **BashOutput** (poll by bash_id) and stop it with **KillShell** — NOT TaskOutput, which cannot see these processes.
- **PowerShell**: Execute PowerShell commands (Windows). Same background/BashOutput/KillShell semantics as Bash.
- **BashOutput**: Read new stdout/stderr from a background Bash/PowerShell shell started with run_in_background (by bash_id).
- **KillShell**: Terminate a background shell by bash_id.
- **WebFetch**: Fetch URL content, convert HTML to markdown, process with a prompt.
- **NotebookEdit**: Edit Jupyter notebook cells. Read the notebook first.
- **EnterWorktree**: Create isolated git worktree (only when user explicitly asks).
- **AskUserQuestion**: ALWAYS use this tool to ask the user questions — shows an interactive widget with clickable buttons. NEVER print questions as plain text.`,
      }))
    }

    case 'notifications/initialized':
    case 'notifications/cancelled': {
      logMcpRequest(session, body.method, 'ok')
      // Notifications have no id — return 202 Accepted per MCP Streamable HTTP spec
      return new Response(null, {
        status: 202,
        headers: { 'Mcp-Session-Id': sessionId },
      })
    }

    case 'tools/list': {
      // Merge built-in MCP_TOOLS with any external tools registered via WS
      const builtinNames = new Set(MCP_TOOLS.map(t => t.name))
      const externalTools = session.registeredTools
        .filter(t => !builtinNames.has(t.name))
        .map(t => ({
          name: t.name,
          description: t.description,
          inputSchema: t.inputSchema,
        }))
      const toolList = [...MCP_TOOLS, ...externalTools]

      logMcpRequest(session, 'tools/list', 'ok', undefined, Date.now() - reqStart)
      debugLog('tools/list response', { builtinCount: MCP_TOOLS.length, externalCount: externalTools.length, total: toolList.length })
      return jsonResponse(jsonRpcResponse(body.id, {
        tools: toolList,
      }))
    }

    case 'tools/call': {
      const params = body.params as { name?: string; arguments?: Record<string, unknown> } | undefined
      const toolName = params?.name
      const toolInput = params?.arguments ?? {}

      if (!toolName) {
        logMcpRequest(session, 'tools/call', 'error', 'Missing tool name')
        return jsonResponse(jsonRpcError(body.id, -32602, 'Missing tool name'), 400)
      }

      // Check tool exists in built-in tools OR registered external tools
      const isBuiltin = MCP_TOOLS.some(t => t.name === toolName)
      const isExternal = session.registeredTools.some(t => t.name === toolName)
      if (!isBuiltin && !isExternal) {
        logMcpRequest(session, `tools/call:${toolName}`, 'error', `Unknown tool: ${toolName}`)
        return jsonResponse(jsonRpcError(body.id, -32602, `Unknown tool: ${toolName}`), 400)
      }

      // Stream the response as SSE so the headers reach the CLI within
      // milliseconds. Without this, CLI's `wrapFetchWithTimeout` (60s hard
      // ceiling, hardcoded in services/mcp/client.ts) aborts the fetch for
      // any tool that takes longer than 60s — Bash/Grep/Glob runs that
      // legitimately take minutes show up as "The operation timed out" on
      // the client, even though our bridge happily finishes the work.
      return sseStreamResponse(body.id ?? null, async () => {
        try {
          const result = await sendMcpCall(sessionId, toolName, toolInput)
          const dur = Date.now() - reqStart
          logMcpRequest(session, `tools/call:${toolName}`, 'ok', undefined, dur)

          // If the client host returned a properly-formatted MCP result (with content array),
          // pass it through directly. This preserves image content blocks and other types.
          if (result && typeof result === 'object' && !Array.isArray(result) &&
              'content' in (result as Record<string, unknown>) &&
              Array.isArray((result as Record<string, unknown>).content)) {
            return jsonRpcResponse(body.id, truncateMcpResult(result as Record<string, unknown>, toolName))
          }

          // Otherwise wrap raw result in text
          return jsonRpcResponse(body.id, truncateMcpResult({
            content: [
              { type: 'text', text: typeof result === 'string' ? result : JSON.stringify(result) },
            ],
          }, toolName))
        } catch (err) {
          const errMsg = err instanceof Error ? err.message : String(err)
          errorLog('MCP tool call failed', { sessionId, toolName, error: errMsg })
          logMcpRequest(session, `tools/call:${toolName}`, 'error', errMsg, Date.now() - reqStart)

          // Return error as tool result (not JSON-RPC error) so Claude can see it
          return jsonRpcResponse(body.id, {
            content: [
              { type: 'text', text: `Error: ${errMsg}` },
            ],
            isError: true,
          })
        }
      })
    }

    // -----------------------------------------------------------------
    // Elicitation — interactive user prompts (AskUserQuestion, ExitPlanMode)
    // -----------------------------------------------------------------
    case 'elicitation/create': {
      const params = body.params as {
        message?: string
        requestedSchema?: Record<string, unknown>
      } | undefined

      const message = params?.message || ''
      const requestedSchema = params?.requestedSchema

      debugLog('Elicitation request', { sessionId, message: message.slice(0, 100) })

      const requestId = `elicit-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`

      const ws = getSessionWs(sessionId)
      if (!ws) {
        // No socket at all for this session — nothing can ever show the widget.
        logMcpRequest(session, 'elicitation/create', 'ok', undefined, Date.now() - reqStart)
        return jsonResponse(jsonRpcResponse(body.id, { action: 'dismiss' }))
      }

      // ── Deferred flow ─────────────────────────────────────────────────
      // Respond to the CLI immediately so its internal MCP client doesn't
      // sit on an open HTTP connection that any layer (undici, proxy, etc.)
      // might abort after a few minutes. The real user answer is delivered
      // later via PTY stdin as the next user turn — CLI picks it up
      // naturally and the model reacts to it as a regular follow-up.
      //
      // Registered with `deferred: true` so `handleElicitationResponse`
      // knows to route the answer to PTY rather than a dead promise. Register
      // BEFORE sending — and keep it registered even if delivery fails now — so
      // a re-send on reattach can still find it (a momentary not-OPEN socket
      // during a reconnect must NOT silently lose the question, the old bug).
      session.pendingElicitations.set(requestId, {
        resolve: () => { /* unused in deferred flow */ },
        reject: () => { /* unused in deferred flow */ },
        toolName: 'elicitation',
        message,
        requestedSchema,
        createdAt: Date.now(),
        jsonRpcId: body.id,
        deferred: true,
      })

      // Guarded send (bufferedAmount cap + try/catch) — a raw ws.send on a racing
      // socket could silently fail and drop the widget while the pending stayed.
      sendToClient(ws, {
        type: 'elicitation:request',
        requestId,
        toolName: 'elicitation',
        message,
        requestedSchema,
      })
      eventBus.emit('elicitation:pending', { sessionId, requestId, message })

      logMcpRequest(session, 'elicitation/create', 'ok', undefined, Date.now() - reqStart)
      return jsonResponse(jsonRpcResponse(body.id, {
        action: 'accept',
        content: {
          _deferred: true,
          note: 'User is still answering the interactive question. Do not act yet — their reply will arrive as the next user message.',
        },
      }))
    }

    case 'resources/list': {
      logMcpRequest(session, 'resources/list', 'ok')
      return jsonResponse(jsonRpcResponse(body.id, {
        resources: session.registeredResources.map(({ uri, name, description, mimeType }) => ({ uri, name, description, mimeType })),
      }))
    }

    case 'resources/templates/list': {
      logMcpRequest(session, 'resources/templates/list', 'ok')
      return jsonResponse(jsonRpcResponse(body.id, {
        resourceTemplates: session.registeredResourceTemplates.map(({ uriTemplate, name, description, mimeType }) => ({
          uriTemplate, name, description, mimeType,
        })),
      }))
    }

    case 'resources/read': {
      const params = body.params as { uri?: string } | undefined
      const resource = session.registeredResources.find(item => item.uri === params?.uri)
      const template = !resource && typeof params?.uri === 'string'
        ? session.registeredResourceTemplates.find(item => matchesResourceTemplate(item.uriTemplate, params.uri!))
        : undefined
      if (!resource && !template) {
        logMcpRequest(session, 'resources/read', 'error', 'resource not found')
        return jsonResponse(jsonRpcError(body.id, -32002, `Resource not found: ${params?.uri ?? '(no uri)'}`))
      }
      try {
        const result = await sendMcpCall(sessionId, '__bridge_mcp_resource_read', resource
          ? { uri: resource.uri }
          : { uri: params!.uri!, serverId: template!.serverId })
        if (!result || typeof result !== 'object' || !Array.isArray((result as { contents?: unknown[] }).contents)) {
          throw new Error('External MCP server returned an invalid resources/read result')
        }
        logMcpRequest(session, 'resources/read', 'ok')
        return jsonResponse(jsonRpcResponse(body.id, result))
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        logMcpRequest(session, 'resources/read', 'error', message)
        return jsonResponse(jsonRpcError(body.id, -32002, message))
      }
    }

    case 'prompts/list': {
      logMcpRequest(session, 'prompts/list', 'ok')
      return jsonResponse(jsonRpcResponse(body.id, {
        prompts: session.registeredPrompts.map(({ name, description, arguments: args }) => ({ name, description, arguments: args })),
      }))
    }

    case 'prompts/get': {
      const params = body.params as { name?: string; arguments?: Record<string, unknown> } | undefined
      const prompt = session.registeredPrompts.find(item => item.name === params?.name)
      if (!prompt) return jsonResponse(jsonRpcError(body.id, -32602, `Prompt not found: ${params?.name ?? '(no name)'}`))
      try {
        const result = await sendMcpCall(sessionId, '__bridge_mcp_prompt_get', {
          name: prompt.name,
          arguments: params?.arguments ?? {},
        })
        if (!result || typeof result !== 'object' || !Array.isArray((result as { messages?: unknown[] }).messages)) {
          throw new Error('External MCP server returned an invalid prompts/get result')
        }
        logMcpRequest(session, 'prompts/get', 'ok')
        return jsonResponse(jsonRpcResponse(body.id, result))
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        logMcpRequest(session, 'prompts/get', 'error', message)
        return jsonResponse(jsonRpcError(body.id, -32603, message))
      }
    }

    default: {
      // Unknown notifications (no id) → 202
      if (!body.id) {
        debugLog('Unknown MCP notification', { method: body.method })
        logMcpRequest(session, body.method, 'ok')
        return new Response(null, {
          status: 202,
          headers: { 'Mcp-Session-Id': sessionId },
        })
      }
      warnLog('Unknown MCP method', { method: body.method })
      logMcpRequest(session, body.method, 'error', `Method not found: ${body.method}`)
      return jsonResponse(
        jsonRpcError(body.id, -32601, `Method not found: ${body.method}`),
        400
      )
    }
  }
}

/**
 * Handle MCP GET request — SSE event stream (Streamable HTTP spec).
 * Claude Code may attempt to open an SSE stream for server-initiated messages.
 * We don't need server push, so return 405 with session header.
 */
export function handleMcpGet(c: Context): Response {
  const sessionId = c.req.param('sessionId') ?? ''
  return new Response(null, {
    status: 405,
    statusText: 'Method Not Allowed',
    headers: { 'Mcp-Session-Id': sessionId },
  })
}

/**
 * Handle MCP DELETE request — session termination (Streamable HTTP spec).
 * Return 200 to acknowledge.
 */
export function handleMcpDelete(c: Context): Response {
  return new Response(null, { status: 200 })
}
