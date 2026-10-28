// Resource budget of the embedded DuckDB inside the Bridge process.
//
// DuckDB's own defaults are 80% of the machine's RAM and one thread per core.
// Inside Bridge that memory is shared with the V8 heap, Buffers, node-pty and
// every Claude CLI child, none of which DuckDB knows about — the default lets
// the analytics index alone claim most of the container. The limit is derived
// from the memory actually available to this process (cgroup limit when one is
// set, otherwise physical RAM) and can be overridden explicitly.

import fs from 'fs'
import os from 'os'

const MiB = 1024 * 1024

/** Memory available to this process: the cgroup v2/v1 limit when set, else RAM. */
export function processMemoryLimitBytes(readFile: (p: string) => string = (p) => fs.readFileSync(p, 'utf8')): number {
  const total = os.totalmem()
  for (const p of ['/sys/fs/cgroup/memory.max', '/sys/fs/cgroup/memory/memory.limit_in_bytes']) {
    try {
      const raw = readFile(p).trim()
      if (raw === 'max') continue
      const n = Number(raw)
      // v1 reports a huge sentinel for "unlimited".
      if (Number.isFinite(n) && n > 0 && n < total) return n
    } catch {
      /* not in a cgroup with a limit */
    }
  }
  return total
}

export interface DuckDbBudget {
  memory_limit: string
  threads: string
}

/** DuckDB settings for this process. `OCB_DUCKDB_MEMORY_LIMIT` (DuckDB syntax,
 *  e.g. `1GB`) and `OCB_DUCKDB_THREADS` override the derived values. */
export function duckDbBudget(
  env: NodeJS.ProcessEnv = process.env,
  limitBytes: number = processMemoryLimitBytes(),
  cpus: number = os.availableParallelism(),
): DuckDbBudget {
  // 15% of the process budget, within [256 MiB, 2 GiB]: enough for the stats
  // queries and a compaction COPY (DuckDB spills to disk beyond it).
  const derived = Math.min(2048, Math.max(256, Math.floor((limitBytes * 0.15) / MiB)))
  const threads = Math.max(1, Math.min(4, Math.floor(cpus / 2)))
  return {
    memory_limit: env.OCB_DUCKDB_MEMORY_LIMIT?.trim() || `${derived}MB`,
    threads: env.OCB_DUCKDB_THREADS?.trim() || String(threads),
  }
}
