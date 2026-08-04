// ============================================================================
// Structured Logging — organized by type into subdirectories
// logs/
//   errors/          ← ALWAYS written, even with logging disabled
//   sessions/        ← session debug/verbose logs
//   captured/        ← captured prompts (CAPTURE button)
//   sdk/             ← SDK initialization logs
//   prompts/         ← system prompts, processed prompts, tools
// ============================================================================

import { appendFile } from "fs/promises"
import { mkdirSync } from "fs"
import path from "path"
import { args } from "../utils/args"
import type { LogLevel } from "../utils/args"

// Log level hierarchy (higher number = more verbose)
const LOG_LEVEL_VALUES: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  verbose: 4,
}

// Resolve flags: --log-level > --verbose > --debug > env vars > default (error)
function resolveLogLevel(): LogLevel {
  if (args.logLevel) return args.logLevel
  if (args.verbose || process.env.VERBOSE === "true" || process.env.VERBOSE === "1") return "verbose"
  if (args.debug || process.env.DEBUG === "true" || process.env.DEBUG === "1" || process.env.CLAUDE_PROXY_DEBUG === "1") return "debug"
  const envLevel = process.env.CLAUDE_PROXY_LOG_LEVEL as LogLevel | undefined
  if (envLevel && envLevel in LOG_LEVEL_VALUES) return envLevel
  return "error"
}

const ACTIVE_LOG_LEVEL = resolveLogLevel()
const ACTIVE_LOG_VALUE = LOG_LEVEL_VALUES[ACTIVE_LOG_LEVEL]

// Backward-compat exports
export const DEBUG = ACTIVE_LOG_VALUE >= LOG_LEVEL_VALUES.debug
export const VERBOSE = ACTIVE_LOG_VALUE >= LOG_LEVEL_VALUES.verbose

const LOG_ROOT = path.join(process.cwd(), "logs")

export type LogSubdir = 'errors' | 'sessions' | 'captured' | 'sdk' | 'prompts'

const SUBDIRS: LogSubdir[] = ['errors', 'sessions', 'captured', 'sdk', 'prompts']

const createdDirs = new Set<string>()

function ensureDir(dir: string): void {
  if (createdDirs.has(dir)) return
  // This runs at module load — BEFORE the process-level uncaughtException
  // handler is installed — so an EACCES/EROFS here would crash boot outright.
  // Swallow it: logging to disk is best-effort, the server must still start.
  try {
    mkdirSync(dir, { recursive: true })
    createdDirs.add(dir)
  } catch { /* log dir unwritable — file logging silently disabled */ }
}

// Always create errors/ subdirectory
ensureDir(path.join(LOG_ROOT, 'errors'))

// Create other subdirectories only if logging is enabled
if (ACTIVE_LOG_VALUE > LOG_LEVEL_VALUES.error) {
  for (const sub of SUBDIRS) {
    ensureDir(path.join(LOG_ROOT, sub))
  }
}

function formatTimestamp(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0")
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}_${pad(d.getHours())}-${pad(d.getMinutes())}-${pad(d.getSeconds())}`
}

function formatDate(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0")
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

// Session state
const SESSION_START = new Date()
const SESSION_FILE = formatTimestamp(SESSION_START) + ".log"

/** @deprecated Use subdirectory-based logging. Kept for backward compat. */
export const LOG_PATH = path.join(LOG_ROOT, 'sessions', SESSION_FILE)

// Batching state for session logs
let logBuffer: string[] = []
let flushScheduled = false

function scheduleFlush(): void {
  if (flushScheduled || logBuffer.length === 0) return
  flushScheduled = true
  setImmediate(async () => {
    const batch = logBuffer.join("")
    logBuffer = []
    flushScheduled = false
    try {
      await appendFile(LOG_PATH, batch)
    } catch {
      // Ignore write errors
    }
  })
}

// Batching state for error logs
let errorBuffer: string[] = []
let errorFlushScheduled = false

function getErrorLogPath(): string {
  return path.join(LOG_ROOT, 'errors', `${formatDate(new Date())}.log`)
}

function scheduleErrorFlush(): void {
  if (errorFlushScheduled || errorBuffer.length === 0) return
  errorFlushScheduled = true
  setImmediate(async () => {
    const batch = errorBuffer.join("")
    errorBuffer = []
    errorFlushScheduled = false
    try {
      await appendFile(getErrorLogPath(), batch)
    } catch {
      // Ignore write errors
    }
  })
}

/**
 * Internal log function with level checking.
 */
function log(level: LogLevel, msg: string, data?: unknown, reqId?: string): void {
  if (LOG_LEVEL_VALUES[level] > ACTIVE_LOG_VALUE) return

  const prefix = reqId ? ` [${reqId}]` : ''
  const line = `[${new Date().toISOString()}] [${level.toUpperCase()}]${prefix} ${msg}${data ? ': ' + JSON.stringify(data) : ''}\n`
  logBuffer.push(line)
  scheduleFlush()
}

/**
 * Log an error (ALWAYS written to logs/errors/, even with logging disabled).
 * Also written to session log if logging is enabled.
 */
export function errorLog(msg: string, data?: unknown, reqId?: string): void {
  const prefix = reqId ? ` [${reqId}]` : ''
  const line = `[${new Date().toISOString()}] [ERROR]${prefix} ${msg}${data ? ': ' + JSON.stringify(data) : ''}\n`

  // Always write to errors/ subdirectory
  errorBuffer.push(line)
  scheduleErrorFlush()

  // Also write to session log if logging is enabled
  logBuffer.push(line)
  scheduleFlush()
}

/**
 * Log a warning (SDK crashes, disconnections, tool failures, retries).
 * Always logged when log level >= warn.
 */
export function warnLog(msg: string, data?: unknown, reqId?: string): void {
  log('warn', msg, data, reqId)
}

/**
 * Log an info message.
 */
export function infoLog(msg: string, data?: unknown, reqId?: string): void {
  log('info', msg, data, reqId)
}

/**
 * Log a debug message (only when --debug, DEBUG=true, or --log-level debug+)
 */
export function debugLog(msg: string, data?: unknown): void {
  log('debug', msg, data)
}

/**
 * Log a verbose message (only when --verbose, VERBOSE=true, or --log-level verbose)
 */
export function verboseLog(msg: string, data?: unknown): void {
  log('verbose', msg, data)
}

/**
 * Write to a specific file in a subdirectory under logs/
 * @param filename - The filename to write to
 * @param content - The content to write
 * @param subdir - Subdirectory: 'errors' | 'sessions' | 'captured' | 'sdk' | 'prompts'
 */
export function writeLogFile(filename: string, content: string, subdir: LogSubdir = 'sessions'): void {
  // Always write if subdir is 'errors' or 'captured', otherwise check log level
  const alwaysWrite = subdir === 'errors' || subdir === 'captured'
  if (!alwaysWrite && ACTIVE_LOG_VALUE < LOG_LEVEL_VALUES.debug) return

  const dirPath = path.join(LOG_ROOT, subdir)
  ensureDir(dirPath)
  const filePath = path.join(dirPath, filename)
  appendFile(filePath, content).catch(() => {})
}
