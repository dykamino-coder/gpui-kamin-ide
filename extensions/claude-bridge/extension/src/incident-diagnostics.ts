import { createHmac, randomBytes } from "node:crypto"
import { mkdirSync } from "node:fs"
import { join } from "node:path"

import { RollingLogWriter } from "../../../../src/kamin-host/rolling-log.js"

const MAX_TRACKED_TABS = 256
const MAX_TRACKED_RENDERERS = 16
const MAX_COUNTER = 1_000_000_000
const MIN_SAMPLE_INTERVAL_MS = 10_000
const INCIDENT_LOG_MAX_BYTES = 1 * 1024 * 1024
const INCIDENT_LOG_BACKUPS = 3
const TAB_REF_KEY = randomBytes(32)

type ConnectionStatus = "disconnected" | "connecting" | "connected" | "error"
type ConnectionCause = "none" | "auth" | "timeout" | "network" | "session" | "remote-close" | "unknown"
type RendererRole = "chat" | "tools" | "customize" | "unknown"

export interface SafeConnectionTransition {
  event: "connection-transition"
  tabRef: string
  status: ConnectionStatus
  cause: ConnectionCause
  retryAttempt: number
}

export interface SafeRendererSample {
  event: "renderer-sample"
  role: RendererRole
  heapMB: number | null
  retainedTabs: number
  retainedEntries: number
  activeEntries: number
  storeWindow: number
  scrollUpMax: number
  windowState: "within-configured-window" | "over-configured-window" | "unknown"
}

function boundedCount(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 0
  return Math.min(MAX_COUNTER, Math.max(0, Math.trunc(value)))
}

function boundedHeap(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0) return null
  return Math.min(MAX_COUNTER, Math.round(value * 10) / 10)
}

function safeStatus(value: unknown): ConnectionStatus {
  if (value === "disconnected" || value === "connecting" || value === "connected" || value === "error") return value
  return "error"
}

function safeRole(value: unknown): RendererRole {
  if (value === "chat" || value === "tools" || value === "customize") return value
  return "unknown"
}

function classifyCause(value: unknown, status: ConnectionStatus, closeCode: unknown): ConnectionCause {
  if (status !== "error" && status !== "disconnected") return "none"
  if (closeCode === 1006 || closeCode === 1012 || closeCode === 1013 || closeCode === 1014) return "network"
  if (typeof closeCode === "number") return "remote-close"
  if (typeof value !== "string" || value.length === 0) return "unknown"
  const text = value.toLowerCase()
  if (text.includes("401") || text.includes("403") || text.includes("auth") || text.includes("token")) return "auth"
  if (text.includes("timeout") || text.includes("timed out")) return "timeout"
  if (text.includes("session")) return "session"
  if (text.includes("closed") || text.includes("close code")) return "remote-close"
  if (text.includes("network") || text.includes("socket") || text.includes("econn") || text.includes("dns")) return "network"
  return "unknown"
}

function tabRef(value: unknown): string {
  const id = typeof value === "string" ? value : "missing"
  return createHmac("sha256", TAB_REF_KEY).update(id).digest("hex").slice(0, 12)
}

export function normalizeConnectionTransition(tabId: unknown, raw: unknown): SafeConnectionTransition {
  const state = raw && typeof raw === "object" ? raw as Record<string, unknown> : {}
  const status = safeStatus(state.status)
  return {
    event: "connection-transition",
    tabRef: tabRef(tabId),
    status,
    cause: classifyCause(state.error, status, state.closeCode),
    retryAttempt: boundedCount(state.retryAttempt),
  }
}

export function normalizeRendererSample(raw: unknown): SafeRendererSample {
  const sample = raw && typeof raw === "object" ? raw as Record<string, unknown> : {}
  const windowState = sample.windowState === "within-configured-window" || sample.windowState === "over-configured-window"
    ? sample.windowState
    : "unknown"
  return {
    event: "renderer-sample",
    role: safeRole(sample.role),
    heapMB: boundedHeap(sample.heapMB),
    retainedTabs: boundedCount(sample.retainedTabs),
    retainedEntries: boundedCount(sample.retainedEntries),
    activeEntries: boundedCount(sample.activeEntries),
    storeWindow: boundedCount(sample.storeWindow),
    scrollUpMax: boundedCount(sample.scrollUpMax),
    windowState,
  }
}

export function formatIncidentLine(record: SafeConnectionTransition | SafeRendererSample): string {
  return `[incident] ${JSON.stringify({
    schema: 1,
    ts: new Date().toISOString(),
    appVersion: process.env.KAMIN_APP_VERSION ?? "unknown",
    processRole: "extension-host",
    pid: process.pid,
    ...record,
  })}`
}

const lastConnectionByTab = new Map<string, string>()
const lastSampleByRole = new Map<RendererRole, number>()
let installed = false
let incidentLog: RollingLogWriter | null = null

function emit(record: SafeConnectionTransition | SafeRendererSample): void {
  incidentLog?.write(`${formatIncidentLine(record)}\n`)
}

export function recordBridgeOutbound(channel: string, args: unknown[]): void {
  if (channel !== "connection-state-changed") return
  const record = normalizeConnectionTransition(args[0], args[1])
  const signature = `${record.status}:${record.cause}:${String(record.retryAttempt)}`
  if (lastConnectionByTab.get(record.tabRef) === signature) return
  lastConnectionByTab.set(record.tabRef, signature)
  while (lastConnectionByTab.size > MAX_TRACKED_TABS) {
    const oldest = lastConnectionByTab.keys().next().value
    if (oldest === undefined) break
    lastConnectionByTab.delete(oldest)
  }
  emit(record)
}

export function recordRendererSample(raw: unknown, now = Date.now()): void {
  const record = normalizeRendererSample(raw)
  const last = lastSampleByRole.get(record.role) ?? 0
  if (now - last < MIN_SAMPLE_INTERVAL_MS) return
  lastSampleByRole.set(record.role, now)
  while (lastSampleByRole.size > MAX_TRACKED_RENDERERS) {
    const oldest = lastSampleByRole.keys().next().value
    if (oldest === undefined) break
    lastSampleByRole.delete(oldest)
  }
  emit(record)
}

export function installIncidentDiagnostics(
  logDir: string,
  subscribe: (listener: (sample: unknown) => void) => void,
): void {
  if (installed) return
  installed = true
  try {
    mkdirSync(logDir, { recursive: true })
    incidentLog = new RollingLogWriter(join(logDir, "incident.log"), {
      maxBytes: INCIDENT_LOG_MAX_BYTES,
      backups: INCIDENT_LOG_BACKUPS,
      rotateOnOpen: true,
    })
  } catch { /* diagnostics must never break extension activation */ }
  subscribe((sample) => recordRendererSample(sample))
}
