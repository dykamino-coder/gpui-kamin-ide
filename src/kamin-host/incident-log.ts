import { join } from "node:path"
import { RollingLogWriter } from "./rolling-log.js"

const HOST_LOG_MAX_BYTES = 5_242_880
const INCIDENT_LOG_MAX_BYTES = 1_048_576
const INCIDENT_LOG_BACKUPS = 3

interface CommonIncident {
  schema: 1
  ts: string
  appVersion: string
}

function common(): CommonIncident {
  return {
    schema: 1,
    ts: new Date().toISOString(),
    appVersion: process.env.KAMIN_APP_VERSION ?? "unknown",
  }
}

export interface HostIncidentLogs {
  writeRaw(chunk: string | Buffer): void
  writeExtensionExit(pid: number | null, exitCode: number | null, signal: string | null): void
  close(): void
}

export function createHostIncidentLogs(dataDir: string): HostIncidentLogs {
  let raw: RollingLogWriter | null = null
  let incident: RollingLogWriter | null = null
  try {
    // Raw extension output can contain user/plugin data. Keep only the bounded
    // current run; never carry it into a durable backup generation.
    raw = new RollingLogWriter(join(dataDir, "host.log"), {
      maxBytes: HOST_LOG_MAX_BYTES,
      backups: 0,
      rotateOnOpen: true,
    })
  } catch { /* best-effort diagnostics */ }
  try {
    incident = new RollingLogWriter(join(dataDir, "incident.log"), {
      maxBytes: INCIDENT_LOG_MAX_BYTES,
      backups: INCIDENT_LOG_BACKUPS,
      rotateOnOpen: true,
    })
    incident.write(`[incident] ${JSON.stringify({
      ...common(),
      processRole: "kamin-host",
      pid: process.pid,
      event: "host-log-start",
    })}\n`)
  } catch { /* best-effort diagnostics */ }

  return {
    writeRaw: (chunk) => raw?.write(chunk),
    writeExtensionExit: (pid, exitCode, signal) => incident?.write(`[incident] ${JSON.stringify({
      ...common(),
      processRole: "extension-host",
      pid,
      event: "process-exit",
      exitCode,
      signal,
    })}\n`),
    close: () => { raw?.close(); incident?.close() },
  }
}
