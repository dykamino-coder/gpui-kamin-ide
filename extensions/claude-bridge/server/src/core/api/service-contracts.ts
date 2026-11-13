import { z } from 'zod'
import { operation as op, type Contract } from './contract'
import * as s from './schemas'

const health = z.object({
  status: s.str,
  service: s.str,
  version: s.str,
  mode: s.str,
  sessions: s.sessionCounts,
  statsWrite: z.object({ depth: s.num, oldestWaitMs: s.num, headRunningMs: s.num }),
})
const publicRoute = { security: [] }
const sync = { tag: 'File sync', security: ['apiBearer'] }
const hooks = { tag: 'Hooks', security: ['apiBearer'] }
const test = {
  tag: 'Development test API',
  security: [],
  description: 'Registered only when NODE_ENV is not production. Intended for synthetic client-host integration tests.',
}
const binary = { ...publicRoute, tag: 'Distribution', responseMedia: 'application/octet-stream' }
// These protocols carry user-defined JSON (hook input, OTLP attributes, MCP tool
// schemas). JSON-valued extension fields are intentional, not missing contracts.
const fileMap = z.record(s.str, s.str)
const syncBody = z.looseObject({
  skills: fileMap.optional(),
  agents: fileMap.optional(),
  commands: fileMap.optional(),
  settings: s.str.optional(),
  claudeMd: s.str.optional(),
  agentsMd: s.str.optional(),
  pluginModules: z.array(s.object).optional(),
})

export const serviceContracts: Contract[] = [
  op('GET', '/', 'Bridge health', health, { ...publicRoute, tag: 'Health' }),
  op('GET', '/health', 'Bridge health', health, { ...publicRoute, tag: 'Health' }),
  ...['GET', 'POST', 'DELETE'].map((method) =>
    op(method, '/mcp/:sessionId', 'MCP Streamable HTTP transport', method === 'GET' ? undefined : s.json, {
      tag: 'MCP transport',
      security: ['mcpBearer'],
      responseMedia: method === 'GET' ? 'text/event-stream' : 'application/json',
      ...(method === 'POST' ? { body: z.union([s.object, z.array(s.object)]) } : {}),
      description:
        'MCP JSON-RPC 2.0 session transport. POST carries request/notification or batch; GET opens SSE; DELETE closes transport. Session ownership and protocol validation remain in the MCP handler.',
    }),
  ),
  op('POST', '/otel/v1/metrics', 'Receive OTLP JSON metrics', z.object({}), {
    tag: 'Telemetry',
    security: [],
    body: z.object({ resourceMetrics: z.array(s.object) }),
  }),
  op('POST', '/otel/v1/logs', 'Receive OTLP JSON logs', z.object({}), {
    tag: 'Telemetry',
    security: [],
    body: z.object({ resourceLogs: z.array(s.object) }),
  }),
  op(
    'GET',
    '/api/sync/:tokenId/reload-diagnostics',
    'Recent skill reload acknowledgements',
    z.object({ events: z.array(s.object) }),
    sync,
  ),
  op('POST', '/api/sync/:tokenId/user', 'Upload user customization files', s.ok, {
    ...sync,
    body: syncBody,
    description: 'File maps and optional customization metadata. Uses the existing sync token/hash ownership contract.',
  }),
  op('POST', '/api/sync/:tokenId/project', 'Upload project customization files', s.ok, {
    ...sync,
    body: syncBody.extend({ projectPath: s.str.min(1), rules: fileMap.optional() }),
  }),
  op(
    'GET',
    '/api/sync/:tokenId/status',
    'Customization sync status',
    z.object({ user: s.object.nullable(), projects: z.array(s.object) }),
    sync,
  ),
  op('GET', '/api/sync/:tokenId/tree', 'Synced file tree', z.looseObject({ projects: z.array(s.object) }), sync),
  op(
    'GET',
    '/api/sync/:tokenId/file',
    'Read a synced file preview',
    z.object({ path: s.str, size: s.num, truncated: s.bool, content: s.str }),
    { ...sync, query: z.object({ path: s.str }) },
  ),
  op('POST', '/api/hooks/:sessionId/:event/:hookId', 'Dispatch a registered CLI hook', s.object, {
    ...hooks,
    security: ['mcpBearer'],
    body: s.object,
    description:
      'Hook-specific input/output fields follow the registered handler. Empty responses are permitted by the CLI hook protocol.',
  }),
  op('GET', '/api/hooks/library', 'Hook template library', z.object({ templates: z.array(s.object) }), hooks),
  op('GET', '/api/hooks/log', 'Recent hook executions', z.object({ entries: z.array(s.object) }), {
    ...hooks,
    query: z.object({ limit: s.integerQuery.optional() }),
  }),
  op('POST', '/api/hooks/emit/:event', 'Emit a Bridge hook event', s.ok, {
    ...hooks,
    body: s.object,
    bodyOptional: true,
  }),
  op('POST', '/api/hooks/log/clear', 'Clear hook execution history', s.ok, hooks),
  op('POST', '/api/hooks/test', 'Test a draft hook', z.object({ ok: s.bool, result: s.json, effectiveHost: s.str }), {
    ...hooks,
    body: z.object({ event: s.str, matcher: s.str.optional(), handler: s.object, mockPayload: s.object.optional() }),
  }),
  op('GET', '/api/hooks/merged', 'Merged hook sources', z.object({ hooks: z.array(s.object) }), {
    ...hooks,
    query: z.object({ cwd: s.str.optional() }),
  }),
  op(
    'GET',
    '/api/hooks/active/:sessionId',
    'Active hooks for a session',
    z.object({ hooks: z.array(s.object) }),
    hooks,
  ),
  op('GET', '/api/test/ping', 'Development API ping', s.ok.extend({ timestamp: s.str, sessionCount: s.num }), test),
  op(
    'POST',
    '/api/test/ensure-token',
    'Issue a synthetic integration token',
    z.object({ token: s.str, id: s.str, created: s.bool }),
    { ...test, body: z.object({ name: s.str.optional() }), bodyOptional: true },
  ),
  op('GET', '/api/test/tokens', 'Synthetic token identities', z.object({ tokens: z.array(s.token) }), test),
  op(
    'GET',
    '/api/test/sessions',
    'Synthetic session list',
    z.object({ sessions: z.array(s.object), count: s.num }),
    test,
  ),
  op('GET', '/api/test/sessions/:id', 'Synthetic session detail', z.looseObject({ id: s.str }), test),
  op('GET', '/api/test/tree/:tokenId', 'Synthetic session tree', z.object({ tree: z.array(s.object) }), test),
  op('POST', '/api/test/mcp-call/:sessionId', 'Simulate a client-host MCP call', z.object({ result: s.json }), {
    ...test,
    body: z.object({ toolName: s.str.min(1), input: s.object.optional() }),
  }),
  op(
    'POST',
    '/api/test/elicitation/:sessionId',
    'Simulate a client-host elicitation',
    z.object({ requestId: s.str, result: s.object }),
    { ...test, body: z.object({ message: s.str.min(1), requestedSchema: s.object.optional() }) },
  ),
  op('POST', '/api/test/ws-send/:sessionId', 'Send a synthetic client-host message', z.object({ sent: s.bool }), {
    ...test,
    body: s.json,
  }),
  op('GET', '/ui', 'Dashboard entry', undefined, { ...publicRoute, tag: 'Web UI', responseMedia: 'text/html' }),
  op('GET', '/ui/*', 'Dashboard assets and SPA routes', undefined, {
    ...publicRoute,
    tag: 'Web UI',
    responseMedia: 'application/octet-stream',
  }),
  op('GET', '/dashboard', 'Legacy dashboard redirect', undefined, { ...publicRoute, tag: 'Web UI', statuses: [302] }),
  op('GET', '/dashboard/*', 'Legacy dashboard path redirect', undefined, {
    ...publicRoute,
    tag: 'Web UI',
    statuses: [302],
  }),
  op('GET', '/download', 'Installer download page', undefined, {
    ...publicRoute,
    tag: 'Distribution',
    responseMedia: 'text/html',
  }),
  op('GET', '/download/kaminide', 'Download KaminIDE installer', undefined, binary),
  op('GET', '/download/electron', 'Legacy installer download alias', undefined, binary),
  op(
    'GET',
    '/api/download/check',
    'Installer availability',
    z.object({ available: s.bool, size: s.num, name: s.nullableString }),
    { ...publicRoute, tag: 'Distribution' },
  ),
  op(
    'GET',
    '/api/bridge-version',
    'Bridge and installer versions',
    z.object({ version: s.str, installerAvailable: s.bool, feedUrl: s.str, installerName: s.nullableString }),
    { ...publicRoute, tag: 'Distribution' },
  ),
  op('GET', '/updates/win32/RELEASES', 'Legacy update manifest', undefined, {
    ...publicRoute,
    tag: 'Distribution',
    responseMedia: 'text/plain',
  }),
  op('GET', '/updates/win32/:file{.+\\.nupkg}', 'Legacy update package', undefined, binary),
  op(
    'GET',
    '/updates/kaminide/:target/:arch/:current',
    'Native update manifest',
    z.object({ version: s.str, pub_date: s.str, url: s.str, notes: s.str }),
    { ...publicRoute, tag: 'Distribution', statuses: [200, 204] },
  ),
  op('GET', '/updates/kaminide/artifact/:name', 'Native update artifact', undefined, binary),
  op('GET', '/updates/kaminide/artifact', 'Native update artifact alias', undefined, binary),
  op(
    'GET',
    '/api/openapi.json',
    'Download the generated OpenAPI 3.1 specification',
    z.looseObject({ openapi: z.literal('3.1.0'), info: s.object, paths: s.object }),
    { tag: 'API documentation', security: ['basicAuth', 'adminBearer'] },
  ),
  op('GET', '/api/docs', 'Interactive Scalar API reference', undefined, {
    tag: 'API documentation',
    security: ['basicAuth', 'adminBearer'],
    responseMedia: 'text/html',
  }),
]
