export type UsageSource = 'statusline' | 'tui-compatibility' | 'unavailable'
export interface UsageWindow {
  id: string
  label: string
  model?: string
  percent: number
  resetsAt: string | null
  resetText: string | null
  observedAt: string
  source: Exclude<UsageSource, 'unavailable'>
  freshness: 'fresh' | 'stale'
  reason: string | null
}
export interface PlanUsageData {
  windows: UsageWindow[]
  observedAt: string
  attemptedAt: string
  source: UsageSource
  claudeCodeVersion: string | null
  state: 'complete' | 'partial' | 'unavailable'
  reason: string | null
  diagnostics: { missing: string[]; finalScreenLines: number; outputLimited: boolean }
  extra: string | null
  /** Deprecated read-only compatibility projections; dashboard uses windows. */
  session: { percent: number; resets: string } | null
  weekAll: { percent: number; resets: string } | null
  weekSonnet: { percent: number; resets: string } | null
  timestamp: string
}
