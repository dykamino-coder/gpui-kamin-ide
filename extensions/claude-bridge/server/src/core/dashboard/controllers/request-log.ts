// Контракт истории запросов панели (INC-2026-0016).
//
// Маршруты читают события OTel, но раньше молча игнорировали смещение и
// фильтры, объявляли успехом событие с ошибкой, а удаляли из ДРУГОГО набора
// данных, чем читали. Худшее следствие: удаление с фильтром «пользователь +
// ошибки» стирало ВСЮ историю и метрики пользователя, а не совпавшее.
//
// Решение: маршруты поддерживаются и работают ТОЛЬКО с набором OTel. То, что
// этот набор честно исполнить не может, отвергается с внятной причиной, а не
// исполняется наполовину. Молчаливо проигнорированный фильтр в запросе на
// УДАЛЕНИЕ — это и есть потеря данных.

/** Статус запроса восстанавливается по самому событию, а не назначается. */
export type RequestStatus = 'success' | 'error'

export interface TelemetryEventLike {
  id?: number
  sessionId?: string
  userName?: string
  eventName?: string
  data: Record<string, unknown>
  timestamp?: string
}

export interface ParsedListQuery {
  limit: number
  offset: number
  userName?: string
}

export interface ParsedDeleteFilter {
  userName: string
}

export type Parsed<T> = { ok: true; value: T } | { ok: false; error: string }

const DEFAULT_LIMIT = 50
const MAX_LIMIT = 500

function parseBounded(raw: string | undefined, fallback: number, max: number): number | null {
  if (raw === undefined || raw === '') return fallback
  const n = Number(raw)
  if (!Number.isInteger(n) || n < 0 || n > max) return null
  return n
}

/** Набор OTel не хранит ни конечную точку, ни статус отдельными полями:
 *  статус выводится из содержимого события, а конечная точка у всех одна.
 *  Поэтому предикат по ним в выборку не превращается — такой фильтр
 *  отвергается, а не исполняется частично. */
function rejectUnsupported(q: Record<string, string | undefined>): string | null {
  if (q.status !== undefined && q.status !== '') {
    return 'status filtering is not supported by the OTel request history'
  }
  if (q.endpoint !== undefined && q.endpoint !== '' && q.endpoint !== 'anthropic') {
    return `unknown endpoint "${q.endpoint}"; the OTel request history only records "anthropic"`
  }
  return null
}

export function parseListQuery(q: Record<string, string | undefined>): Parsed<ParsedListQuery> {
  const unsupported = rejectUnsupported(q)
  if (unsupported) return { ok: false, error: unsupported }
  const limit = parseBounded(q.limit, DEFAULT_LIMIT, MAX_LIMIT)
  if (limit === null) return { ok: false, error: `limit must be an integer between 0 and ${String(MAX_LIMIT)}` }
  const offset = parseBounded(q.offset, 0, Number.MAX_SAFE_INTEGER)
  if (offset === null) return { ok: false, error: 'offset must be a non-negative integer' }
  const userName = q.userName === undefined || q.userName === '' ? undefined : q.userName
  return { ok: true, value: userName === undefined ? { limit, offset } : { limit, offset, userName } }
}

/** Удаление по фильтру. Без имени пользователя удалять нечего: фильтр,
 *  который ничего не сужает, раньше приводил к стиранию всего набора. */
export function parseDeleteFilter(q: Record<string, string | undefined>): Parsed<ParsedDeleteFilter> {
  const unsupported = rejectUnsupported(q)
  if (unsupported) return { ok: false, error: unsupported }
  if (q.userName === undefined || q.userName === '') {
    return { ok: false, error: 'userName is required; use the clear-all endpoint to remove everything' }
  }
  return { ok: true, value: { userName: q.userName } }
}

export function statusOf(ev: TelemetryEventLike): { status: RequestStatus; statusCode: number } {
  const error = ev.data.error
  if (typeof error === 'string' && error.length > 0) {
    const code = ev.data.statusCode
    return { status: 'error', statusCode: typeof code === 'number' && code > 0 ? code : 500 }
  }
  const code = ev.data.statusCode
  return { status: 'success', statusCode: typeof code === 'number' && code > 0 ? code : 200 }
}

/** Единое отображение события OTel в запись журнала запросов. Список и
 *  подробность обязаны отвечать одинаково — раньше они расходились. */
export function toRequestLogEntry(ev: TelemetryEventLike): Record<string, unknown> {
  const { status, statusCode } = statusOf(ev)
  return {
    id: String(ev.id ?? ''),
    timestamp: ev.timestamp,
    endpoint: 'anthropic',
    method: 'POST',
    model: (ev.data.model as string) || 'unknown',
    userName: ev.userName || undefined,
    durationMs: (ev.data.durationMs as number) || 0,
    inputTokens: (ev.data.inputTokens as number) || 0,
    outputTokens: (ev.data.outputTokens as number) || 0,
    cacheReadTokens: (ev.data.cacheReadTokens as number) || 0,
    cacheWriteTokens: (ev.data.cacheWriteTokens as number) || 0,
    toolsUsed: [] as string[],
    status,
    statusCode,
    error: (ev.data.error as string) || undefined,
    isUserRequest: false,
    sessionKey: ev.sessionId || undefined,
    eventName: ev.eventName,
    eventData: ev.data,
  }
}
