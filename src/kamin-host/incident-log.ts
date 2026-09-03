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

export interface HostIncidentLogOptions {
  /** Test seam: the incident generation cap. Production keeps the default. */
  incidentMaxBytes?: number
}

/** Every incident line goes through here: one `[incident] <json>\n` record,
 *  written atomically across rotation (`RollingLogWriter.writeRecord`). A
 *  record that cannot fit a whole generation is replaced by a bounded marker
 *  naming the event and its size, so the trail stays machine-readable and the
 *  retention cap stays honest. */
function emitIncident(writer: RollingLogWriter | null, payload: Record<string, unknown>): void {
  if (!writer) return
  const record = `[incident] ${JSON.stringify({ ...common(), ...payload })}\n`
  if (writer.writeRecord(record)) return
  // The marker is deliberately smaller than any real record (no appVersion,
  // role or pid): a marker that could itself be oversized would vanish too.
  const dropped = `[incident] ${JSON.stringify({
    schema: 1,
    ts: new Date().toISOString(),
    event: "record-dropped",
    droppedEvent: typeof payload.event === "string" ? payload.event : "unknown",
    bytes: Buffer.byteLength(record, "utf8"),
  })}\n`
  writer.writeRecord(dropped)
}

export function createHostIncidentLogs(dataDir: string, options: HostIncidentLogOptions = {}): HostIncidentLogs {
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
      maxBytes: options.incidentMaxBytes ?? INCIDENT_LOG_MAX_BYTES,
      backups: INCIDENT_LOG_BACKUPS,
      rotateOnOpen: true,
    })
    emitIncident(incident, {
      processRole: "kamin-host",
      pid: process.pid,
      event: "host-log-start",
    })
  } catch { /* best-effort diagnostics */ }

  return {
    writeRaw: (chunk) => raw?.write(chunk),
    writeExtensionExit: (pid, exitCode, signal) => {
      emitIncident(incident, {
        processRole: "extension-host",
        pid,
        event: "process-exit",
        exitCode,
        signal,
      })
    },
    close: () => { raw?.close(); incident?.close() },
  }
}
