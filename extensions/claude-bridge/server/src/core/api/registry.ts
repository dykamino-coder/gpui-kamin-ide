import type { Hono, MiddlewareHandler } from 'hono'
import { z } from 'zod'
import { VERSION } from '../config'
import { dashboardContracts } from './dashboard-contracts'
import { serviceContracts } from './service-contracts'
import type { Contract } from './contract'
import { error } from './schemas'

export const contracts = [...dashboardContracts, ...serviceContracts]
const key = (method: string, path: string) => `${method.toUpperCase()} ${path}`
const byRoute = new Map(contracts.map((c) => [key(c.method, c.path), c]))

export function registeredContracts(app: Hono): Contract[] {
  const routes = new Map(app.routes.filter((r) => r.method !== 'ALL').map((r) => [key(r.method, r.path), r]))
  const missing = [...routes.keys()].filter((k) => !byRoute.has(k))
  if (missing.length) throw new Error(`Missing API contracts: ${missing.join(', ')}`)
  return [...routes.keys()].map((k) => byRoute.get(k)!)
}

export function jsonSchema(schema: z.ZodType) {
  // Input mode preserves the same unknown-key semantics as safeParse. Output
  // mode would incorrectly advertise additionalProperties:false for z.object.
  return z.toJSONSchema(schema, { target: 'draft-2020-12', io: 'input' })
}

export function openApiPath(path: string): string {
  return path.replace(/:([\w]+)(?:\{[^}]+\})?/g, '{$1}').replace('*', '{path}')
}

export function createSpecification(app: Hono) {
  const paths: Record<string, Record<string, unknown>> = {}
  for (const contract of registeredContracts(app)) {
    const path = openApiPath(contract.path)
    const parameters: Record<string, unknown>[] = [...path.matchAll(/\{(\w+)\}/g)].map((m) => ({
      name: m[1],
      in: 'path',
      required: true,
      schema: { type: 'string' },
      ...(m[1] === 'path' ? { description: 'Wildcard suffix; may contain slashes.', 'x-hono-wildcard': true } : {}),
    }))
    for (const [name, schema] of Object.entries(contract.query?.shape ?? {})) {
      parameters.push({ name, in: 'query', required: !schema.isOptional(), schema: jsonSchema(schema) })
    }
    const media = contract.responseMedia ?? 'application/json'
    const responses: Record<string, unknown> = {}
    for (const status of contract.statuses ?? [200]) {
      responses[status] = {
        description: status === 204 ? 'No content' : 'Handler response',
        ...(status === 204 || status === 302
          ? {}
          : {
              content: { [media]: { schema: contract.response ? jsonSchema(contract.response) : { type: 'string' } } },
            }),
      }
    }
    responses.default = {
      description: 'Handler or authentication error. Binary/static endpoints may return plain text.',
      content: { 'application/json': { schema: jsonSchema(error) }, 'text/plain': { schema: { type: 'string' } } },
    }
    const operation = {
      operationId: `${contract.method.toLowerCase()}_${path.replace(/[^\w]+/g, '_')}`,
      summary: contract.summary,
      tags: [contract.tag],
      description: contract.description,
      security: (contract.security ?? ['basicAuth', 'adminBearer', 'apiBearer']).map((entry) =>
        typeof entry === 'string' ? { [entry]: [] } : entry,
      ),
      parameters,
      responses,
      ...(contract.body
        ? {
            requestBody: {
              required: !contract.bodyOptional,
              content: { [contract.media ?? 'application/json']: { schema: jsonSchema(contract.body) } },
            },
          }
        : {}),
    }
    ;(paths[path] ??= {})[contract.method.toLowerCase()] = operation
  }
  return {
    openapi: '3.1.0',
    info: {
      title: 'Bridge API',
      version: VERSION,
      description:
        'Dashboard and Bridge service API. Basic uses the configured dashboard username/password; adminBearer uses the session issued by /api/dashboard/auth/login. apiBearer is a scoped user token. Interactive PTYs use the WebSocket protocols described in x-websockets. Existing token visibility is preserved: dashboard token list/rename return full values for administrator session Bearer and masked values otherwise.',
    },
    servers: [{ url: '/' }],
    paths,
    components: {
      securitySchemes: {
        basicAuth: {
          type: 'http',
          scheme: 'basic',
          description: 'Dashboard administrator credentials (when dashboard authentication is enabled).',
        },
        adminBearer: {
          type: 'http',
          scheme: 'bearer',
          description: 'Dashboard session token from POST /api/dashboard/auth/login.',
        },
        mcpBearer: {
          type: 'http',
          scheme: 'bearer',
          description:
            'Per-session MCP credential supplied to the CLI, distinct from administrator and user API tokens.',
        },
        apiBearer: {
          type: 'http',
          scheme: 'bearer',
          description:
            'Existing per-user API token. Dashboard operations retain their ownership/admin restrictions. Sync routes also accept their existing token-hash contract.',
        },
      },
    },
    'x-websockets': {
      '/ws/dashboard': {
        description:
          'Live dashboard events: receive {type:"init",data,timestamp} then {type,data,timestamp} events. The transport has no authentication handshake. Command control uses {type:"commands:subscribe",token}, {type:"commands:output",sessionId,id,token}, {type:"commands:stop",sessionId,id,token}; token must be an administrator session when dashboard authentication is enabled. Responses include commands:snapshots and commands:output.',
      },
      '/ws/terminal': {
        description:
          'Interactive PTY. Query: engine=claude|codex, command=interactive|resume|status|usage (Codex). Claude opens the server shell; send the toolbar command via input. Client JSON: {type:"input",data}, {type:"resize",cols,rows}, {type:"refresh"}. Server JSON: {type:"output",data}, {type:"exit",code}, {type:"error",data}. Claude /login and /logout are interactive CLI flows. Transport retains its existing access behavior.',
      },
      '/ws/session': {
        description:
          'IDE session JSON protocol, authenticated with the user API token in the session handshake. Carries input/output, MCP tool calls, hook forwarding and elicitation; see shared protocol types.',
      },
    },
  }
}

/** Schemas used here also produce the published JSON Schema. Streams bypass JSON validation. */
export const contractMiddleware: MiddlewareHandler = async (c, next) => {
  const route = c.req.matchedRoutes.find((r) => r.method === c.req.method)
  const contract = route && byRoute.get(key(route.method, route.path))
  if (!contract) return next()
  const invalid = () => c.json({ error: 'Request does not match the API schema' }, 400)
  if (contract.query && !contract.query.safeParse(c.req.query()).success) return invalid()
  if (contract.body) {
    try {
      const empty = !c.req.header('content-type') && !c.req.header('content-length')
      if (!(empty && contract.bodyOptional)) {
        // Validate a clone: sync's bounded reader intentionally consumes raw.body.
        const request = c.req.raw.clone()
        const body =
          contract.media === 'multipart/form-data' ? Object.fromEntries(await request.formData()) : await request.json()
        if (!contract.body.safeParse(body).success) return invalid()
      }
    } catch {
      return invalid()
    }
  }
  await next()
  if (!c.res.headers.get('content-type')?.includes('application/json') || c.res.status === 204) return
  const schema = c.res.status >= 400 && !contract.statuses?.includes(c.res.status) ? error : contract.response
  if (!schema) return
  const result = schema.safeParse(await c.res.clone().json())
  if (!result.success) {
    // Log paths only: validation diagnostics must not echo credentials/payloads.
    console.warn(
      'API response contract mismatch',
      contract.method,
      contract.path,
      result.error.issues.map((i) => i.path.join('.')),
    )
    c.res = c.json({ error: 'Response does not match the API schema' }, 500)
  }
}
