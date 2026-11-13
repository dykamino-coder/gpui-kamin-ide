import { signal } from '@preact/signals'

export type Engine = 'claude' | 'codex'
export type EngineFilterValue = 'all' | Engine

function storedFilter(): EngineFilterValue {
  try {
    const v = localStorage.getItem('dashboard-engine-filter')
    return v === 'claude' || v === 'codex' ? v : 'all'
  } catch {
    return 'all'
  }
}

export const engineFilter = signal<EngineFilterValue>(storedFilter())

export function setEngineFilter(value: EngineFilterValue): void {
  engineFilter.value = value
  try {
    localStorage.setItem('dashboard-engine-filter', value)
  } catch {
    /* per-viewer convenience only */
  }
}

export function engineParam(value: EngineFilterValue): string {
  return value === 'all' ? '' : `&engine=${value}`
}
