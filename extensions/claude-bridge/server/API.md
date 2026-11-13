# Bridge administration API

Open **API** in the dashboard sidebar, or visit `/api/docs`. Scalar and all its
assets ship in `dist/dashboard`; no CDN or remote fonts are required. The generated
OpenAPI 3.1 document is available at `/api/openapi.json` and can also be downloaded
or imported into another Scalar instance. Use the configured dashboard Basic
credentials or a dashboard administrator session bearer token. Scalar exposes
both security schemes in its authentication selector. Authentication remains
disabled when the dashboard's existing authentication setting is disabled.

`src/core/api/{dashboard-contracts,service-contracts}.ts` define the operations;
`schemas.ts` defines their Zod schemas. The same schemas validate live requests
and JSON responses and produce JSON Schema for OpenAPI. There is no separate
handwritten specification. `registeredContracts(createApp())` rejects any HTTP
route missing a contract; the integration test covers production and development
registrations. Middleware registrations (`ALL`) are not HTTP operations.

## Dashboard inventory

Paths in the table are relative to `/api/dashboard` unless stated otherwise.
The inventory covers `src/ui/SingleDashboard.tsx`, all components, the API client,
auth/statistics signals and direct fetch/WebSocket callers.

| Admin data/action (UI source)                                                                                                                 | HTTP API / transport                                                                                                                                                                  |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sign in, check session, sign out (`Login`, auth signals)                                                                                      | `POST /auth/login`, `GET /auth/session`, `POST /auth/logout`                                                                                                                          |
| Server version, mode, address, database and process/system health (`StatusCard`, header)                                                      | `GET /status`; root `/health`; `GET /api/bridge-version`                                                                                                                              |
| Connection/configuration values (`CompactConfig`, connection guide)                                                                           | `GET /config`, `GET /proxy-settings`                                                                                                                                                  |
| Edit log level, DB request limit, HTTP/HTTPS proxy, bypass list, CA path (`CompactConfig`)                                                    | `PUT /proxy-settings` with a partial JSON object; `maxRequests` now persists                                                                                                          |
| Upload CA file, clear CA override, reset defaults (`CompactConfig`)                                                                           | `POST /proxy-settings/cert` multipart `cert`; `PUT /proxy-settings` with `caCert:null`; `DELETE /proxy-settings`                                                                      |
| Read-only legacy settings returned alongside editable values                                                                                  | `GET /config`, `GET /proxy-settings`; only the six documented editable fields are written                                                                                             |
| Claude account email, plan, organization, expiry, authentication status (`StatusCard`)                                                        | `GET /account`, `GET /auth-status`; `force=true` refreshes                                                                                                                            |
| Explicit health refresh and displayed result (`SingleDashboard`)                                                                              | `POST /health/refresh`; existing legacy `prompt` query accepted; no paid prompt is sent                                                                                               |
| Claude usage/reset/freshness windows (`UsageBars`)                                                                                            | `GET /usage`, `force=1` refreshes                                                                                                                                                     |
| Codex account, device login state, limits and models (`CodexAccountCard`, `CodexLimitBars`)                                                   | `GET /codex/account`, `GET /codex/limits`, `GET /codex/models`; `force=true` refreshes account/limits                                                                                 |
| Start/cancel device login, sign out (`CodexAccountCard`, terminal toolbar)                                                                    | `POST /codex/login`, `POST /codex/login/cancel`, `POST /codex/logout`                                                                                                                 |
| Token list and setup guides (`ApiTokens`)                                                                                                     | `GET /tokens` (full for admin session Bearer, masked otherwise); `POST /tokens` creates/recovers a setup credential by name; `POST /tokens/resolve` resolves a caller-held credential |
| Rename/delete token (`ApiTokens`, API client)                                                                                                 | `PATCH /tokens/{id}` (new route), `DELETE /tokens/{id}`                                                                                                                               |
| Per-token streaming/capture preferences                                                                                                       | `GET`, `PUT /tokens/{tokenId}/streaming-settings`                                                                                                                                     |
| User list, Claude/Codex totals and cost (`UserCards`, sidebar)                                                                                | `GET /users?engine=all\|claude\|codex`                                                                                                                                                |
| Rollup totals, models, heatmap and time range (`StatsGrid`, `StatsCard`, engine/range selectors)                                              | `GET /stats/overview`; `range=all\|30d\|7d`, `engine`, optional `token`, `agg=hm`, `tz` offset                                                                                        |
| Batched user cards, local timezone, revision polling (`StatsCard`, users-stats signals)                                                       | `GET /stats/users-overview`; `range`, `engine`, `subject`, IANA `tz`, `tzOffset`, `rev`                                                                                               |
| Usage chart and per-user time series (`UsageChart`)                                                                                           | `GET /stats/timeseries?period=day&days=30&engine=all`                                                                                                                                 |
| Live counters, user/model token totals, request log (`StatsGrid`, request signals)                                                            | `GET /stats`; live updates on root `/ws/dashboard`                                                                                                                                    |
| Live sessions, engine/user groups, resource use, attachment state (`ActivePtySessions`)                                                       | `GET /sessions`, `GET /sessions/stats`, `GET /sessions/tree?tokenId=...`                                                                                                              |
| Kill live session, inspect MCP call log                                                                                                       | `POST /sessions/{id}/kill`, `GET /sessions/{id}/mcp-log`                                                                                                                              |
| Historical session cards: title, folder, counts, model, JSONL availability (`TokenSessionsList`)                                              | `GET /tokens/{tokenId}/sessions?engine=all&limit=50&before=...`                                                                                                                       |
| View/export transcript (`TokenSessionsList`)                                                                                                  | `GET /sessions/{sessionId}/jsonl`; download: `POST /sessions/{sessionId}/jsonl-download`, retain the HttpOnly cookie, then GET returned URL within 30 seconds (one use)               |
| Delete historical session/file (`TokenSessionsList`)                                                                                          | `DELETE /sessions/{sessionId}?rmFile=1` (does not kill a live process)                                                                                                                |
| MCP tools, servers, usage/wasted schema bytes, user/project breakdown (`McpUsageReport`)                                                      | `GET /mcp-report`                                                                                                                                                                     |
| Request list, pagination, user filtering (`RequestListModal`, `RequestCapture`)                                                               | `GET /requests?limit=50&offset=0&userName=...`; `GET /users/{userName}/requests`                                                                                                      |
| Request detail/body/response/tool list and browser-generated TXT/HTML/JSON export (`RequestDetail`, `ToolsModal`, `DownloadMenu`, `ErrorLog`) | `GET /requests/{id}`; `GET /user-chat/{userName}`; exports transform those same API values locally                                                                                    |
| Clear all history, delete user history or one entry                                                                                           | `DELETE /requests`, `DELETE /requests/filter?userName=...`, `DELETE /requests/{id}`                                                                                                   |
| Error totals, timeline and recent details (`ApiErrorsCard`)                                                                                   | `GET /errors/count`, `GET /errors/timeseries`, `GET /errors/recent`; optional `engine`                                                                                                |
| Terminal session count and noninteractive quick-action equivalents (`Terminal`)                                                               | New `GET /terminal` lists HTTP equivalents for Claude status/usage and Codex status/usage/login/logout                                                                                |
| Interactive terminal launch/resume, Claude login/logout, input/resize (`Terminal`)                                                            | Root `/ws/terminal` WebSocket, documented below and in OpenAPI `x-websockets`                                                                                                         |
| API sidebar link (`Sidebar`)                                                                                                                  | Root `GET /api/docs`, `GET /api/openapi.json`                                                                                                                                         |
| Installer availability/download (`Header`)                                                                                                    | Root `GET /api/download/check`, `GET /download/kaminide`                                                                                                                              |
| Theme, selected engine/range, sort, expanded groups, copied text                                                                              | Browser presentation preferences; corresponding server data/filters are exposed above; no server mutation exists                                                                      |

The OTel request history retains its current filter limitations: a nonempty
`status` filter and an endpoint other than `anthropic` return 400. Deletion by
filter requires a user name. These are documented existing limitations, not
silent approximations of a destructive filter.

Token list preserves its existing visibility: an administrator session Bearer
receives full values, and other callers receive masked values. Token rename
returns the same representation as the list. The UI masks the displayed value
and copies the list value directly, preserving its existing copy behavior.
Login, token creation/recovery and development fixtures retain their existing
credential responses.
Account APIs never serialize `auth.json` or stored account credentials. Device
login returns the short-lived user code shown by the dashboard.

## Remaining server routes

`service-contracts.ts` also covers health (`/`, `/health`), MCP Streamable HTTP
(`/mcp/{sessionId}` GET/POST/DELETE), OTLP JSON (`/otel/v1/{logs,metrics}`),
customization sync (`/api/sync/{tokenId}/{user,project,status,tree,file,reload-diagnostics}`),
hook dispatch/library/log/test/merged/active routes (`/api/hooks/...`), development
fixtures (`/api/test/...`), dashboard/static/legacy redirects (`/ui`, `/ui/*`,
`/dashboard`, `/dashboard/*`), installer distribution (`/download...`,
`/api/download/check`, `/api/bridge-version`) and legacy/native updates
(`/updates/win32/...`, `/updates/kaminide/...`). Development routes are absent
from the production specification. Existing token ownership rules are retained;
MCP and CLI hook dispatch use the per-session MCP credential, not an admin token.
User-defined OTLP attributes, hook payloads and MCP tool schemas intentionally
use open JSON extension objects. Static files, JSONL and SSE remain streaming
responses rather than buffered JSON arrays.

## WebSocket contracts

- `/ws/dashboard` immediately sends `{type:"init",data,timestamp}`, then live
  `{type,data,timestamp}` events. Its existing event transport has no authentication
  handshake. Command control messages `commands:subscribe`, `commands:output` and
  `commands:stop` carry an administrator session `token` when authentication is
  enabled; output/stop also carry `sessionId` and command `id`.
- `/ws/terminal?engine=claude|codex`: client messages are `{type:"input",data}`,
  `{type:"resize",cols,rows}` and `{type:"refresh"}`; server messages are
  `{type:"output",data}`, `{type:"exit",code}` and `{type:"error",data}`. Codex
  accepts `command=interactive|resume|status|usage`. Claude starts a server shell;
  send its toolbar CLI command through input. Claude login/logout remain
  interactive. The transport retains its existing access behavior.
- `/ws/session` is the IDE session JSON protocol with a user API token handshake;
  shared protocol types describe input/output, tool calls and hook forwarding.

## Verification

Run `npm run typecheck`, `npm run lint`, `npm test -- --maxWorkers=4`, and
`npm --prefix src/ui run build`. `admin-api.test.ts` uses an in-memory DuckDB with
synthetic users/sessions/telemetry, mock account/CLI boundaries and real handlers.
It checks registered-route coverage, request rejection, response validation,
administrator authentication, masking, token rename and the raw-body sync path.
`settings.test.ts` checks persistence of the UI's request-history limit.

For the opt-in Chromium acceptance, set `BRIDGE_API_BROWSER_EXECUTABLE` to a local
Chromium executable and `BRIDGE_API_BROWSER_EVIDENCE` to the agent's private
diagnostics directory, then run `npx vitest run src/core/server/admin-api.test.ts`.
It uses a temporary browser profile and local synthetic server, records screenshot
and request/error evidence, and asserts no external HTTP request. No real account
CLI, owner's browser profile or paid prompt is used.
