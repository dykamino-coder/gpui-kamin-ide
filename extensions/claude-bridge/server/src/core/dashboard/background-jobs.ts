// ============================================================================
// Background jobs — periodic refresh of health, account, usage
// Pushes updates via eventBus → WS to all connected dashboard clients
// ============================================================================

import { config } from '../config'
import { eventBus } from '../events/bus'
import { debugLog, warnLog } from '../logging'

const HEALTH_INTERVAL = 5 * 60 * 1000  // 5 minutes
const ACCOUNT_INTERVAL = 5 * 60 * 1000 // 5 minutes
const USAGE_INTERVAL = 2 * 60 * 1000   // 2 minutes

let healthTimer: ReturnType<typeof setInterval> | null = null
let accountTimer: ReturnType<typeof setInterval> | null = null
let usageTimer: ReturnType<typeof setInterval> | null = null

async function refreshHealth(): Promise<void> {
  const base = `http://${config.host}:${config.port}`
  try {
    const res = await fetch(`${base}/api/dashboard/health/refresh`, { method: 'POST' })
    if (res.ok) {
      debugLog('Background: health refreshed')
    }
  } catch (err) {
    warnLog('Background: health refresh failed', { error: err instanceof Error ? err.message : 'Unknown' })
  }
}

async function refreshAccount(): Promise<void> {
  const base = `http://${config.host}:${config.port}`
  try {
    const res = await fetch(`${base}/api/dashboard/account`)
    if (res.ok) {
      debugLog('Background: account refreshed')
    }
  } catch (err) {
    warnLog('Background: account refresh failed', { error: err instanceof Error ? err.message : 'Unknown' })
  }
}

async function refreshUsage(): Promise<void> {
  const base = `http://${config.host}:${config.port}`
  try {
    const res = await fetch(`${base}/api/dashboard/usage`)
    if (res.ok) {
      const data = await res.json()
      eventBus.emit('usage:updated', data)
      debugLog('Background: usage refreshed')
    }
  } catch (err) {
    warnLog('Background: usage refresh failed', { error: err instanceof Error ? err.message : 'Unknown' })
  }
}

export function startBackgroundJobs(): void {
  // Stagger starts to avoid thundering herd
  healthTimer = setInterval(refreshHealth, HEALTH_INTERVAL)
  accountTimer = setInterval(refreshAccount, ACCOUNT_INTERVAL)
  usageTimer = setInterval(refreshUsage, USAGE_INTERVAL)
  debugLog('Background jobs started (health: 5m, account: 5m, usage: 2m)')
}

export function stopBackgroundJobs(): void {
  if (healthTimer) { clearInterval(healthTimer); healthTimer = null }
  if (accountTimer) { clearInterval(accountTimer); accountTimer = null }
  if (usageTimer) { clearInterval(usageTimer); usageTimer = null }
}
