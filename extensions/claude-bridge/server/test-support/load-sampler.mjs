// Linux-only resource sampler for the load harness: one JSON line per tick
// with the Bridge server process (CPU, RSS, fds, sockets), the fake CLIs and
// the container cgroup. Reads /proc and /sys only.
import fs from 'node:fs'
import { setTimeout as sleep } from 'node:timers/promises'

const periodMs = Number(process.argv[2] ?? 5000)
const read = (p) => {
  try {
    return fs.readFileSync(p, 'utf8')
  } catch {
    return ''
  }
}
const status = (pid, key) => Number(new RegExp(`${key}:\\s+(\\d+)`).exec(read(`/proc/${pid}/status`))?.[1] ?? 0)
const cpuTicks = (pid) => {
  const f = read(`/proc/${pid}/stat`).split(') ')[1]?.split(' ') ?? []
  return Number(f[11] ?? 0) + Number(f[12] ?? 0)
}
function processes() {
  const out = { server: null, fakes: [] }
  for (const pid of fs.readdirSync('/proc').filter((d) => /^\d+$/.test(d))) {
    const cmd = read(`/proc/${pid}/cmdline`).split('\0').join(' ')
    if (cmd.includes('fake-claude.mjs')) out.fakes.push(pid)
    else if (cmd.includes('claude-bridge.ts') && !cmd.includes('npx') && !cmd.includes('tsx/dist/cli')) out.server = pid
  }
  return out
}
function fds(pid) {
  let total = 0
  let sockets = 0
  try {
    for (const fd of fs.readdirSync(`/proc/${pid}/fd`)) {
      total++
      try {
        if (fs.readlinkSync(`/proc/${pid}/fd/${fd}`).startsWith('socket:')) sockets++
      } catch {
        /* closed meanwhile */
      }
    }
  } catch {
    /* gone */
  }
  return { total, sockets }
}
let prev
for (;;) {
  const { server, fakes } = processes()
  const now = Date.now()
  const ticks = server ? cpuTicks(server) : 0
  const fakeTicks = fakes.reduce((s, p) => s + cpuTicks(p), 0)
  const cpu = prev && prev.server === server ? ((ticks - prev.ticks) / 100 / ((now - prev.now) / 1000)) * 100 : null
  const fakeCpu = prev ? ((fakeTicks - prev.fakeTicks) / 100 / ((now - prev.now) / 1000)) * 100 : null
  const f = server ? fds(server) : { total: 0, sockets: 0 }
  console.log(
    JSON.stringify({
      t: now,
      serverPid: server,
      serverCpuPct: cpu === null ? null : Math.round(cpu * 10) / 10,
      serverRssMb: server ? Math.round(status(server, 'VmRSS') / 1024) : null,
      serverHwmMb: server ? Math.round(status(server, 'VmHWM') / 1024) : null,
      serverFds: f.total,
      serverSockets: f.sockets,
      fakeCount: fakes.length,
      fakeCpuPct: fakeCpu === null ? null : Math.round(fakeCpu * 10) / 10,
      cgroupMb: Math.round(Number(read('/sys/fs/cgroup/memory.current') || 0) / 1048576),
      cgroupPeakMb: Math.round(Number(read('/sys/fs/cgroup/memory.peak') || 0) / 1048576),
    }),
  )
  prev = { server, ticks, fakeTicks, now }
  await sleep(periodMs)
}
