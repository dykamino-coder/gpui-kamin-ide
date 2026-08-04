// Renderer freeze watchdog (host side).
//
// The renderer's main thread can wedge PERMANENTLY ("Not Responding", never
// recovers) on reconnect/session-change with a huge session — its own JS can't
// then report anything. The host is a separate process that stays alive, so it
// watches the renderer's heartbeat (kamin:diag:hb, every ~500 ms). When the
// beats stop for FREEZE_MS, the host writes ONE report to <dataDir>/freeze-host.log
// with what the renderer last processed + what the host was pushing at it (the
// flood suspect). Nothing is written while the app is alive.
//
// THIS FILE DOES NOT ISSUE A VERDICT. The heartbeat is a timer, and Chromium
// throttles timers in a hidden window, so its silence cannot distinguish "wedged"
// from "backgrounded" — that is why the native watchdog (freeze_watchdog.rs) added
// the prod/pong channel, and why its reports are the ones that say FREEZE. This
// detector's value is the host→renderer send breadcrumb, which the Rust side
// cannot see. Corroboration, not conclusion.

import { appendFileSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join } from "node:path"

export interface RendererHeartbeat {
  ts: number
  lastCh: string | null
  counts: Record<string, number>
  heapMB?: number
  /** A heavy sync call the renderer was INSIDE as it built this beat. If this is
   *  the last beat before the freeze, it names the call the thread died in. */
  openMark?: string
  openMarkMs?: number
}

const FREEZE_MS = 3000 // silence past this = wedged (Windows flags "Not Responding" ~5s)
// Sends are kept by TIME, not by count. They used to live in an 80-slot ring,
// so every report's channel tally summed to exactly 80 — a number that looked
// like a measurement of the flood but only ever measured the ring. A window
// gives a RATE (n per second), which is the thing that distinguishes a flood
// from ordinary traffic.
const SEND_WINDOW_MS = 10_000
// Safety valve only: a true flood must not grow this array without bound. When
// it trips the report SAYS so — a silent cap is how the ×80 lie happened.
const SEND_HARD_CAP = 20_000
const LOG_CAP_BYTES = 65_536 // keep freeze.log tiny; truncate the head past this
const LOG_KEEP_BYTES = 40_000 // when capped, keep this much of the TAIL (newest reports)
const WATCHDOG_TICK_MS = 1000 // how often to test for silence (FREEZE_MS is the verdict)
const SEND_TAIL = 12 // newest sends quoted verbatim in the report
const MS_PER_SEC = 1000

export interface FreezeWatchdog {
  /** Record a host→renderer send (for the flood breadcrumb). Channel only — see
   *  the impl note on why we no longer measure bytes here. */
  noteSend(channel: string): void
  /** A heartbeat arrived from the renderer — an UNTRUSTED RPC payload, hence
   *  `unknown`: it was typed `RendererHeartbeat` and the caller cast to it, so
   *  the normalizing guards inside read as dead code to both linter and reader
   *  while actually being the only thing keeping a malformed beat from throwing
   *  inside the crash reporter. */
  onHeartbeat(hb: unknown): void
  readonly logPath: string
}

function numericRecord(v: unknown): Record<string, number> {
  if (typeof v !== "object" || v === null) return {}
  const out: Record<string, number> = {}
  for (const [k, val] of Object.entries(v)) if (typeof val === "number") out[k] = val
  return out
}

export function createFreezeWatchdog(dataDir: string): FreezeWatchdog {
  // Distinct from the native Rust watchdog's `freeze.log` (lib.rs) — this Node
  // detector adds the richer host→renderer send-ring but can itself stall if the
  // flood saturates the host loop, so it's the bonus, not the guaranteed catch.
  // The ONE unified diagnostics log (shared with the native watchdog's FREEZE /
  // BACKGROUNDED and the WEBVIEW-CRASH entries) — see src-tauri/src/diag_log.rs.
  const logPath = join(dataDir, "kamin-diag.log")
  let recentSends: { ch: string; ts: number }[] = []
  let sendsDropped = 0
  let lastHb: RendererHeartbeat | null = null
  let frozenLogged = false

  function noteSend(channel: string): void {
    // NO serialization here. This runs on EVERY host→renderer broadcast; a
    // `JSON.stringify` just to measure bytes doubled serialization on the exact
    // flood path this detector exists to observe (the send path stringifies the
    // frame again to actually send it). Record channel + time only — channel
    // FREQUENCY plus the renderer's own per-channel counts identify a flood.
    const now = Date.now()
    recentSends.push({ ch: channel, ts: now })
    // Prune by age, amortized: shift() per send is O(n) on the flood path.
    if (recentSends.length > SEND_HARD_CAP) {
      const cutoff = now - SEND_WINDOW_MS
      const kept = recentSends.filter((s) => s.ts >= cutoff)
      if (kept.length > SEND_HARD_CAP) {
        // Still over cap AFTER pruning = a flood denser than the cap. Drop the
        // oldest half and COUNT what we dropped so the report can't imply the
        // remainder is the whole story.
        const half = Math.floor(kept.length / 2)
        sendsDropped += half
        recentSends = kept.slice(half)
      } else recentSends = kept
    }
  }

  function onHeartbeat(hb: unknown): void {
    // Normalize the untrusted RPC payload: this watchdog is a crash *diagnostic*
    // and must never be the thing that crashes the host. A malformed beat (e.g.
    // missing `counts`) must not later throw inside writeReport's Object.entries.
    const raw = (typeof hb === "object" && hb !== null ? hb : {}) as Partial<Record<keyof RendererHeartbeat, unknown>>
    lastHb = {
      ts: typeof raw.ts === "number" ? raw.ts : Date.now(),
      lastCh: typeof raw.lastCh === "string" ? raw.lastCh : null,
      // Values, not just the container: a non-number slips through to
      // writeReport's sort and yields NaN ordering, silently.
      counts: numericRecord(raw.counts),
      // exactOptionalPropertyTypes: only include heapMB when it's a real number.
      ...(typeof raw.heapMB === "number" ? { heapMB: raw.heapMB } : {}),
      ...(typeof raw.openMark === "string" ? { openMark: raw.openMark } : {}),
      ...(typeof raw.openMarkMs === "number" ? { openMarkMs: raw.openMarkMs } : {}),
    }
    frozenLogged = false // renderer is alive → re-arm for the NEXT freeze
  }

  const timer = setInterval(() => {
    // Never let the detector itself throw into the host's uncaughtException →
    // process.exit(1) path — it would kill the process it's meant to observe.
    try {
      if (!lastHb || frozenLogged) return
      const silent = Date.now() - lastHb.ts
      if (silent >= FREEZE_MS) { frozenLogged = true; writeReport(silent) }
    } catch { /* best-effort diagnostic — swallow */ }
  }, WATCHDOG_TICK_MS)
  ;(timer as { unref?: () => void }).unref?.()

  function writeReport(silentMs: number): void {
    const beat = lastHb
    if (!beat) return
    const beatCounts = Object.entries(beat.counts).sort((a, b) => b[1] - a[1])
      .map(([c, n]) => `${c}×${String(n)}`).join(" ") || "—"
    // Aggregate the recent host→renderer sends by channel — a flood of one
    // channel right before the freeze is the smoking gun (by frequency now, not
    // bytes; see noteSend).
    const now = Date.now()
    const win = recentSends.filter((s) => s.ts >= now - SEND_WINDOW_MS)
    const agg = new Map<string, number>()
    for (const s of win) agg.set(s.ch, (agg.get(s.ch) ?? 0) + 1)
    // Report a RATE, not a bare count. The bare count came out of a fixed ring,
    // so it read as a measurement of the flood while only ever measuring the
    // ring — every report tallied to exactly the ring size.
    const secs = Math.max(1, (now - (win[0]?.ts ?? now)) / MS_PER_SEC)
    const aggStr = [...agg.entries()].sort((x, y) => y[1] - x[1])
      .map(([ch, n]) => `${ch}×${String(n)} (${(n / secs).toFixed(1)}/s)`).join(" ") || "—"
    const tail = win.slice(-SEND_TAIL).reverse().map((s) => s.ch).join(" ") || "—"
    const lines = [
      // NOT a verdict, and it must not read like one. This detector sees only the
      // heartbeat, which Chromium throttles in a hidden window — the exact sensor
      // whose silence the native watchdog now refuses to call a freeze. It has no
      // access to the pong channel (that lives in the Rust process), so all it can
      // honestly report is that the beats stopped and what the host was pushing
      // when they did. freeze.log holds the verdict; this holds the corroboration.
      `[${new Date().toISOString()}] [HOST-BEAT] renderer heartbeat quiet ${String(silentMs)}ms`,
      `  NOT a freeze verdict — a hidden window throttles this beat too.`,
      `  See the FREEZE/BACKGROUNDED entries (native watchdog, pong channel) for whether this was real.`,
      `  lastProcessed=${beat.lastCh ?? "—"}  heap=${String(beat.heapMB ?? "?")}MB  beatCounts=[${beatCounts}]`,
      ...(beat.openMark !== undefined
        ? [`  DIED INSIDE: ${beat.openMark} (already ${String(beat.openMarkMs ?? 0)}ms at the last beat)`]
        : []),
      `  host→renderer sends in the last ${String(Math.round(secs))}s: ${aggStr}`,
      ...(sendsDropped > 0
        ? [`  NOTE: ${String(sendsDropped)} older sends dropped (flood over cap) — counts are a LOWER BOUND`]
        : []),
      `  newest sends: ${tail}`,
    ]
    const report = `${lines.join("\n")}\n`
    try {
      let prev = ""
      try {
        if (statSync(logPath).size > LOG_CAP_BYTES) prev = readFileSync(logPath, "utf8").slice(-LOG_KEEP_BYTES)
        else prev = readFileSync(logPath, "utf8")
      } catch { /* new file */ }
      writeFileSync(logPath, `${prev + report  }\n`)
    } catch {
      try { appendFileSync(logPath, `${report  }\n`) } catch { /* best-effort */ }
    }
    try { process.stderr.write(report) } catch { /* */ }
  }

  return { noteSend, onHeartbeat, logPath }
}
