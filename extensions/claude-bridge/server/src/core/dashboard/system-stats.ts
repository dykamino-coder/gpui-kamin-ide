// Live system + per-process resource sampling for the dashboard.
//
// - System CPU% is an aggregate over `os.cpus()` idle/total deltas between two
//   background samples (a single-shot read has no delta → 0). RAM% is host
//   free/total (in a container this reflects the host, which is what "server
//   load" means for this dashboard).
// - Per-session CPU%/RSS walks the PROCESS TREE rooted at the session's PTY pid
//   (the claude CLI spawns node + shell + MCP children, so the direct pid alone
//   undercounts). Linux-only via `/proc`; returns null elsewhere (dev on
//   Win/macOS) so the UI shows "—".
//
// Both samplers run on a shared 2s interval started lazily on first import.
import * as os from 'os'
import { readdir, readFile } from 'fs/promises'

const SAMPLE_INTERVAL_MS = 2000
// Linux clock ticks/sec (getconf CLK_TCK) — 100 on effectively every modern
// kernel; the container is Linux, so hardcoding avoids a getconf spawn.
const CLK_TCK = 100
const PAGE_SIZE = 4096 // bytes; /proc stat rss is in pages

// ─── System CPU (sampled) ───────────────────────────────────────────
function cpuTotals(): { idle: number; total: number } {
  let idle = 0
  let total = 0
  for (const cpu of os.cpus()) {
    for (const v of Object.values(cpu.times)) total += v
    idle += cpu.times.idle
  }
  return { idle, total }
}
let lastCpu = cpuTotals()
let systemCpuPercent = 0

function sampleSystemCpu(): void {
  const now = cpuTotals()
  const dIdle = now.idle - lastCpu.idle
  const dTotal = now.total - lastCpu.total
  lastCpu = now
  systemCpuPercent = dTotal > 0 ? Math.max(0, Math.min(100, 100 * (1 - dIdle / dTotal))) : 0
}

export interface SystemStats {
  cpuPercent: number
  memPercent: number
  memUsedBytes: number
  memTotalBytes: number
}

export function getSystemStats(): SystemStats {
  const memTotalBytes = os.totalmem()
  const memUsedBytes = memTotalBytes - os.freemem()
  return {
    cpuPercent: Math.round(systemCpuPercent),
    memPercent: memTotalBytes > 0 ? Math.round((memUsedBytes / memTotalBytes) * 100) : 0,
    memUsedBytes,
    memTotalBytes,
  }
}

// ─── Per-process tree (Linux /proc) ─────────────────────────────────
interface PidInfo { cpuPercent: number; rssBytes: number; ppid: number }
let latestByPid = new Map<number, PidInfo>()
let latestAt = 0
let prevTicks = new Map<number, number>() // pid → cumulative utime+stime

/** Read every /proc/<pid>/stat once. Returns pid → {utime+stime ticks, ppid,
 *  rss pages}. Empty on non-Linux (readdir('/proc') throws) — the caller then
 *  reports null for per-session stats. */
async function sampleProc(): Promise<Map<number, { ticks: number; ppid: number; rss: number }>> {
  const out = new Map<number, { ticks: number; ppid: number; rss: number }>()
  let entries: string[]
  try {
    entries = await readdir('/proc')
  } catch {
    return out // not Linux / no /proc
  }
  await Promise.all(entries.filter((n) => /^\d+$/.test(n)).map(async (name) => {
    try {
      const raw = await readFile(`/proc/${name}/stat`, 'utf8')
      // `comm` (field 2) can contain spaces and parens — parse after the LAST ')'.
      const rp = raw.lastIndexOf(')')
      if (rp < 0) return
      const rest = raw.slice(rp + 2).trim().split(/\s+/)
      // rest indices are 0-based from field 3: state(0) ppid(1) … utime(11)
      // stime(12) … rss(21). See proc(5).
      const ppid = Number.parseInt(rest[1] ?? '', 10)
      const utime = Number.parseInt(rest[11] ?? '', 10)
      const stime = Number.parseInt(rest[12] ?? '', 10)
      const rss = Number.parseInt(rest[21] ?? '', 10)
      if (Number.isNaN(ppid) || Number.isNaN(utime) || Number.isNaN(rss)) return
      out.set(Number.parseInt(name, 10), { ticks: utime + stime, ppid, rss })
    } catch { /* pid vanished mid-read */ }
  }))
  return out
}

async function sampleProcTree(): Promise<void> {
  const sample = await sampleProc()
  const now = Date.now()
  const dt = latestAt ? (now - latestAt) / 1000 : 0
  const byPid = new Map<number, PidInfo>()
  for (const [pid, s] of sample) {
    const prev = prevTicks.get(pid)
    const cpuPercent = dt > 0 && prev !== undefined
      ? Math.max(0, ((s.ticks - prev) / (CLK_TCK * dt)) * 100)
      : 0
    byPid.set(pid, { cpuPercent, rssBytes: s.rss * PAGE_SIZE, ppid: s.ppid })
  }
  prevTicks = new Map([...sample].map(([pid, s]) => [pid, s.ticks]))
  latestByPid = byPid
  latestAt = now
}

export interface ProcTreeStats { cpuPercent: number; memBytes: number }

/** Sum CPU% + RSS over the process tree rooted at `rootPid`. CPU% is per-core
 *  (like top: a process pinning one core reads ~100%). Null when /proc hasn't
 *  been sampled (non-Linux) or the pid isn't present. */
export function getProcessTreeStats(rootPid: number | undefined): ProcTreeStats | null {
  if (rootPid === undefined || latestByPid.size === 0) return null
  // Adjacency: ppid → children, built from the latest sample.
  const children = new Map<number, number[]>()
  for (const [pid, info] of latestByPid) {
    const arr = children.get(info.ppid)
    if (arr) arr.push(pid)
    else children.set(info.ppid, [pid])
  }
  let cpuPercent = 0
  let memBytes = 0
  const stack = [rootPid]
  const seen = new Set<number>()
  let found = false
  while (stack.length > 0) {
    const pid = stack.pop() as number
    if (seen.has(pid)) continue
    seen.add(pid)
    const info = latestByPid.get(pid)
    if (info) { cpuPercent += info.cpuPercent; memBytes += info.rssBytes; found = true }
    for (const ch of children.get(pid) ?? []) stack.push(ch)
  }
  if (!found) return null
  return { cpuPercent: Math.round(cpuPercent), memBytes }
}

// ─── Lifecycle ──────────────────────────────────────────────────────
let started = false
/** Idempotent: start the background samplers once (first dashboard hit). */
export function startSystemStatsSampler(): void {
  if (started) return
  started = true
  sampleSystemCpu()
  void sampleProcTree()
  const timer = setInterval(() => {
    // Guard the sampler: a synchronous throw in sampleSystemCpu (os/proc read)
    // on a bare setInterval would surface as an uncaughtException. Sampling is
    // best-effort; skip this tick on error rather than take the process down.
    try { sampleSystemCpu() } catch { /* skip this CPU sample */ }
    void sampleProcTree()
  }, SAMPLE_INTERVAL_MS)
  timer.unref?.() // never keep the process alive for sampling
}
