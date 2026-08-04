// ============================================================================
// Persistent settings — stored in DuckDB, editable via dashboard UI
// Priority: DB value > env var > default
// ============================================================================
//
// Reads are sync via the in-memory cache (`getSetting*`). The cache is
// hydrated lazily on first call by kicking off an async DB read in the
// background, so the very first sync read after boot will fall back to
// env/default until the cache resolves. For deterministic behaviour at
// startup, call `await primeSettingsCache()` once before serving traffic.
// Writes (`setSetting`, `resetSettings`, `saveCaCert`) are async since
// DuckDB is async-only.

import path from 'path'
import fs from 'fs'
import { getDb } from '../stats/database'

// All setting keys
export type SettingKey =
  | 'httpProxy' | 'httpsProxy' | 'noProxy' | 'caCert'
  | 'maxConcurrent' | 'logLevel'
  | 'maxConcurrentOpus' | 'maxConcurrentSonnet' | 'maxConcurrentHaiku'
  | 'loopDetection' | 'loopMinRepeats'
  | 'keepToolResults' | 'toolResultMax'
  | 'compression' | 'maxRequests'
  | 'sessionCache'

// Env var fallback map
const ENV_MAP: Record<SettingKey, string | null> = {
  httpProxy: 'CLAUDE_PROXY_HTTP_PROXY',
  httpsProxy: 'CLAUDE_PROXY_HTTPS_PROXY',
  noProxy: 'CLAUDE_PROXY_NO_PROXY',
  caCert: 'CLAUDE_PROXY_CA_CERT',
  maxConcurrent: 'CLAUDE_PROXY_MAX_CONCURRENT',
  maxConcurrentOpus: 'CLAUDE_PROXY_MAX_CONCURRENT_OPUS',
  maxConcurrentSonnet: 'CLAUDE_PROXY_MAX_CONCURRENT_SONNET',
  maxConcurrentHaiku: 'CLAUDE_PROXY_MAX_CONCURRENT_HAIKU',
  logLevel: 'CLAUDE_PROXY_LOG_LEVEL',
  loopDetection: 'CLAUDE_PROXY_DISABLE_LOOP_DETECTION',  // inverted: env "1" = off
  loopMinRepeats: 'CLAUDE_PROXY_LOOP_MIN_REPEATS',
  keepToolResults: 'CLAUDE_PROXY_KEEP_TOOL_RESULTS',
  toolResultMax: 'CLAUDE_PROXY_TOOL_RESULT_MAX',
  compression: 'CLAUDE_PROXY_DISABLE_COMPRESSION',        // inverted: env "1" = off
  maxRequests: 'CLAUDE_PROXY_MAX_REQUESTS',
  sessionCache: null,
}

// Default values
const DEFAULTS: Record<SettingKey, string> = {
  httpProxy: '',
  httpsProxy: '',
  noProxy: '',
  caCert: '',
  maxConcurrent: '6',
  maxConcurrentOpus: '6',
  maxConcurrentSonnet: '6',
  maxConcurrentHaiku: '6',
  logLevel: 'error',
  loopDetection: '1',      // "1" = on
  loopMinRepeats: '4',
  keepToolResults: '25',
  toolResultMax: '50000',
  compression: '1',         // "1" = on
  maxRequests: '1000',
  sessionCache: '1',        // "1" = on
}

// In-memory cache — hydrated lazily.
let cache: Record<string, string | null> | null = null
let hydratePromise: Promise<void> | null = null

/** Kick off async DB load if not already running. Sync getters use whatever
 *  is in `cache` at the moment of call — empty until hydration finishes. */
function ensureCache(): Record<string, string | null> {
  if (!cache) cache = {}
  if (!hydratePromise) {
    hydratePromise = (async () => {
      try {
        const d = await getDb()
        const reader = await d.runAndReadAll('SELECT key, value FROM settings')
        const rows = reader.getRowObjects() as { key: string; value: string }[]
        for (const row of rows) {
          cache![row.key] = row.value
        }
      } catch {
        // DB not ready yet — keep cache empty; sync getters fall back to env/defaults.
      }
    })()
  }
  return cache
}

/** Eagerly hydrate the settings cache from the DB. Call once during server
 *  startup before serving requests so subsequent sync `getSetting*` calls
 *  see the persisted values. */
export async function primeSettingsCache(): Promise<void> {
  ensureCache()
  await hydratePromise
}

/**
 * Get a raw setting value. Priority: DB > env var > default.
 */
export function getSetting(key: SettingKey): string | null {
  const c = ensureCache()
  // DB value first
  if (c[key] !== undefined && c[key] !== null && c[key] !== '') return c[key]!
  // Env var fallback
  const envKey = ENV_MAP[key]
  if (envKey) {
    const envVal = process.env[envKey]
    if (envVal) return envVal
  }
  // Default
  return DEFAULTS[key] || null
}

/**
 * Get a numeric setting.
 */
export function getSettingInt(key: SettingKey, fallback: number): number {
  const val = getSetting(key)
  if (!val) return fallback
  const n = parseInt(val, 10)
  return Number.isFinite(n) ? n : fallback
}

/**
 * Get a boolean setting. For loopDetection/compression: DB stores "1"/"0",
 * env vars use inverted convention (DISABLE_X=1 means off).
 */
export function getSettingBool(key: 'loopDetection' | 'compression' | 'sessionCache'): boolean {
  const c = ensureCache()
  // DB value has priority — stores "1"=on, "0"=off
  if (c[key] !== undefined && c[key] !== null && c[key] !== '') {
    return c[key] === '1' || c[key] === 'true'
  }
  // Env var fallback (inverted: DISABLE_X=1 means OFF)
  const envKey = ENV_MAP[key]
  if (envKey) {
    const envVal = process.env[envKey]
    if (envVal === '1' || envVal === 'true') return false // DISABLE = off
  }
  return true // default on
}

/**
 * Set a setting value in the DB (persisted).
 * Pass null or empty string to clear (revert to env/default).
 */
export async function setSetting(key: SettingKey, value: string | null): Promise<void> {
  const c = ensureCache()
  const d = await getDb()
  if (!value || value.trim() === '') {
    await d.run('DELETE FROM settings WHERE key = ?', [key])
    delete c[key]
  } else {
    const trimmed = value.trim()
    // DuckDB equivalent of SQLite INSERT OR REPLACE.
    await d.run(
      'INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT (key) DO UPDATE SET value = excluded.value',
      [key, trimmed]
    )
    c[key] = trimmed
  }
  // Invalidate downstream caches that snapshot the values at startup.
  if (key === 'caCert') {
    try {
      const { resetExtraTrustRoots } = await import('../proxy/native-mitm')
      resetExtraTrustRoots()
    } catch { /* native-mitm not loaded yet */ }
  }
}

/**
 * Reset all settings to defaults (clear DB overrides).
 */
export async function resetSettings(): Promise<void> {
  const c = ensureCache()
  const d = await getDb()
  await d.run('DELETE FROM settings')
  for (const key of Object.keys(c)) delete c[key]
}

/**
 * Get all proxy-related settings (for dashboard display).
 */
export function getProxySettings(): { httpProxy: string | null; httpsProxy: string | null; noProxy: string | null; caCert: string | null } {
  return {
    httpProxy: getSetting('httpProxy') || null,
    httpsProxy: getSetting('httpsProxy') || null,
    noProxy: getSetting('noProxy') || null,
    caCert: getSetting('caCert') || null,
  }
}

/**
 * Get all editable server settings (for dashboard display).
 */
export function getEditableSettings(): Record<string, string | number | boolean | null> {
  return {
    httpProxy: getSetting('httpProxy') || null,
    httpsProxy: getSetting('httpsProxy') || null,
    noProxy: getSetting('noProxy') || null,
    caCert: getSetting('caCert') || null,
    maxConcurrent: getSettingInt('maxConcurrent', 6),
    maxConcurrentOpus: getSettingInt('maxConcurrentOpus', 6),
    maxConcurrentSonnet: getSettingInt('maxConcurrentSonnet', 6),
    maxConcurrentHaiku: getSettingInt('maxConcurrentHaiku', 6),
    logLevel: getSetting('logLevel') || 'error',
    loopDetection: getSettingBool('loopDetection'),
    loopMinRepeats: getSettingInt('loopMinRepeats', 4),
    keepToolResults: getSettingInt('keepToolResults', 25),
    toolResultMax: getSettingInt('toolResultMax', 50000),
    compression: getSettingBool('compression'),
    maxRequests: getSettingInt('maxRequests', 1000),
    sessionCache: getSettingBool('sessionCache'),
  }
}

/**
 * Save uploaded CA cert file to data/certs/ and update setting.
 * Returns the saved file path.
 */
export async function saveCaCert(filename: string, content: Buffer): Promise<string> {
  const certsDir = path.join(process.cwd(), 'data', 'certs')
  fs.mkdirSync(certsDir, { recursive: true })
  const safeName = filename.replace(/[^a-zA-Z0-9._-]/g, '_')
  const filePath = path.join(certsDir, safeName)
  fs.writeFileSync(filePath, content)
  await setSetting('caCert', filePath)
  return filePath
}

/**
 * Inject proxy settings into env object for subprocess.
 */
export function injectProxyEnv(env: Record<string, string>): void {
  const httpProxy = getSetting('httpProxy')
  const httpsProxy = getSetting('httpsProxy')
  const noProxy = getSetting('noProxy')
  const caCert = getSetting('caCert')
  if (httpProxy) { env.HTTP_PROXY = httpProxy; env.http_proxy = httpProxy }
  if (httpsProxy) { env.HTTPS_PROXY = httpsProxy; env.https_proxy = httpsProxy }
  if (caCert) { env.NODE_EXTRA_CA_CERTS = caCert }

  // Ensure localhost/loopback traffic always bypasses proxy — MCP server runs on 127.0.0.1
  if (httpProxy || httpsProxy) {
    const loopback = '127.0.0.1,localhost,::1'
    const parts = [noProxy, env.NO_PROXY, env.no_proxy, loopback].filter(Boolean)
    const merged = [...new Set(parts.join(',').split(',').map(s => s.trim()).filter(Boolean))].join(',')
    env.NO_PROXY = merged
    env.no_proxy = merged

  }
}
