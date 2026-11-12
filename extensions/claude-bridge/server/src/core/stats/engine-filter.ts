// `?engine=` of the dashboard stats endpoints: all engines, or one of them.
// The value is whitelisted, so the SQL fragment is a literal, never user text.

import type { AccountingEngine } from '../../shared/engine-accounting'

export type EngineFilter = AccountingEngine | null

export const ACCOUNTING_ENGINES: readonly AccountingEngine[] = ['claude', 'codex']

/** Unknown or missing value = all engines. */
export function parseEngineFilter(value: string | null | undefined): EngineFilter {
  return value === 'claude' || value === 'codex' ? value : null
}

/** `AND <column> = '<engine>'` for a filter, '' for all engines. The column
 *  must already be engine-normalized (the accounting view coalesces NULL). */
export function engineClause(column: string, filter: EngineFilter): string {
  return filter ? ` AND ${column} = '${filter}'` : ''
}

/** Same for a column that may hold NULL (= Claude), e.g. api_errors.engine. */
export function nullableEngineClause(column: string, filter: EngineFilter): string {
  return filter ? ` AND COALESCE(${column}, 'claude') = '${filter}'` : ''
}
