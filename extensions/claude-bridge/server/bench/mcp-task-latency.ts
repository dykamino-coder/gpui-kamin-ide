// Latency the durable MCP task path adds to a foreground Bash/PowerShell call:
// store writes (fsync) and the wait for settlement. The client host is
// simulated by settling the task `work` ms after dispatch; the reported
// overhead is (await return) − (settle saved).
// Usage: npx tsx bench/mcp-task-latency.ts [--runs N] [--work ms]
import { mkdtempSync, rmSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { performance } from 'node:perf_hooks'

const home = mkdtempSync(path.join(os.tmpdir(), 'kamin-task-bench-'))
process.env.HOME = home
process.env.USERPROFILE = home

function arg(name: string, fallback: number): number {
  const i = process.argv.indexOf(name)
  return i > 0 ? Number(process.argv[i + 1]) : fallback
}
const RUNS = arg('--runs', 30)
const WORK = arg('--work', 30)

function pct(values: number[], p: number): number {
  const sorted = [...values].sort((a, b) => a - b)
  return sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]!
}

async function main(): Promise<void> {
  const { mcpTasks, awaitMcpTask } = await import('../src/core/pty/mcp-tasks')
  // The removed implementation, kept here as the baseline.
  const poll200 = async (owner: string, id: string, graceMs: number) => {
    const deadline = Date.now() + graceMs
    for (;;) {
      const task = await mcpTasks.get(owner, id)
      if (!task || task.status !== 'working' || Date.now() >= deadline) return task
      await new Promise((r) => setTimeout(r, Math.min(200, Math.max(0, deadline - Date.now()))))
    }
  }
  const rows: string[] = ['| step | p50 ms | p95 ms |', '|---|---:|---:|']
  const create: number[] = []
  const deliver: number[] = []
  const settle: number[] = []
  const waits: Record<string, number[]> = { 'await, 200 ms poll (before)': [], 'await, settle wake-up (after)': [] }
  for (const [label, wait] of [
    ['await, 200 ms poll (before)', poll200],
    ['await, settle wake-up (after)', (o: string, id: string, g: number) => awaitMcpTask(o, id, g)],
  ] as const) {
    for (let i = 0; i < RUNS; i++) {
      let t = performance.now()
      const task = await mcpTasks.create('owner', 'Bash', { command: 'echo hi' })
      create.push(performance.now() - t)
      t = performance.now()
      await mcpTasks.update('owner', task.taskId, (r) => {
        r.delivered = true
      })
      deliver.push(performance.now() - t)
      let settledAt = 0
      setTimeout(async () => {
        const s = performance.now()
        await mcpTasks.settle('owner', task.taskId, { content: [{ type: 'text', text: 'hi' }] })
        settledAt = performance.now()
        settle.push(settledAt - s)
      }, WORK)
      await wait('owner', task.taskId, 120_000)
      waits[label]!.push(performance.now() - settledAt)
    }
  }
  rows.push(`| store create (fsync) | ${pct(create, 50).toFixed(1)} | ${pct(create, 95).toFixed(1)} |`)
  rows.push(`| store mark delivered (fsync) | ${pct(deliver, 50).toFixed(1)} | ${pct(deliver, 95).toFixed(1)} |`)
  rows.push(`| store settle (fsync) | ${pct(settle, 50).toFixed(1)} | ${pct(settle, 95).toFixed(1)} |`)
  for (const [label, values] of Object.entries(waits)) {
    rows.push(`| ${label}: settle → return | ${pct(values, 50).toFixed(1)} | ${pct(values, 95).toFixed(1)} |`)
  }
  console.log(rows.join('\n'))
}

main()
  .catch((err) => {
    console.error(err)
    process.exitCode = 1
  })
  .finally(() => rmSync(home, { recursive: true, force: true }))
