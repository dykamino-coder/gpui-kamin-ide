/**
 * Tiny logger for the server side.
 *
 * Honours `LOG_LEVEL` env (`debug`/`info`/`warn`/`error`, default `info`)
 * and emits ISO timestamps so journald / docker logs stay grep-friendly.
 * Use `logger.child(scope)` for module-local prefixes.
 */
export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

const ORDER: Record<LogLevel, number> = { debug: 10, info: 20, warn: 30, error: 40 }

function envLevel(): LogLevel {
  const raw = (process.env.LOG_LEVEL || '').toLowerCase()
  if (raw === 'debug' || raw === 'info' || raw === 'warn' || raw === 'error') return raw
  return 'info'
}

let currentLevel: LogLevel = envLevel()
export function setLogLevel(level: LogLevel): void {
  currentLevel = level
}

function shouldLog(level: LogLevel): boolean {
  return ORDER[level] >= ORDER[currentLevel]
}

function fmt(level: LogLevel, scope: string | undefined, msg: unknown, extra: unknown[]): unknown[] {
  const ts = new Date().toISOString()
  const tag = scope ? `[${scope}]` : ''
  return [`${ts} ${level.toUpperCase().padEnd(5)} ${tag}`, msg, ...extra]
}

export interface Logger {
  debug(msg: unknown, ...extra: unknown[]): void
  info(msg: unknown, ...extra: unknown[]): void
  warn(msg: unknown, ...extra: unknown[]): void
  error(msg: unknown, ...extra: unknown[]): void
  child(scope: string): Logger
}

function makeLogger(scope?: string): Logger {
  return {
    debug(msg, ...extra) {
      if (shouldLog('debug')) console.debug(...fmt('debug', scope, msg, extra))
    },
    info(msg, ...extra) {
      if (shouldLog('info')) console.log(...fmt('info', scope, msg, extra))
    },
    warn(msg, ...extra) {
      if (shouldLog('warn')) console.warn(...fmt('warn', scope, msg, extra))
    },
    error(msg, ...extra) {
      if (shouldLog('error')) console.error(...fmt('error', scope, msg, extra))
    },
    child(child) {
      return makeLogger(scope ? `${scope}:${child}` : child)
    },
  }
}

export const logger = makeLogger()
