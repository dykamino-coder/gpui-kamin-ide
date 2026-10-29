// Единственный писатель статистической DuckDB.
//
// Зачем. DuckDB роняет ВЕСЬ процесс, если у транзакции не удался COMMIT и
// откат наткнулся на индекс: `duckdb::FatalException` в `RevertCommit` →
// `terminate called`, Node это не перехватывает. Ровно так упал сервер
// 6.3.131: два воркера сборщика JSONL писали в `jsonl_events` с разных
// соединений, второй COMMIT получил дубликат первичного ключа, откат упёрся в
// тот же ключ. Единственный способ сделать это невозможным — не иметь двух
// одновременных пишущих транзакций. Чтение остаётся параллельным — MVCC.
//
// Владелец записи (INC-2026-0078, F06):
//  - очередь FIFO с ОГРАНИЧЕННЫМ допуском: при переполнении новая работа
//    сразу получает `WriteQueueFullError`, а не копит замыкания и данные;
//  - ожидание в очереди ограничено сроком и `AbortSignal`: снятая работа
//    не исполняется;
//  - голова под наблюдением: дольше `headTimeoutMs` — её текущий нативный
//    запрос прерывается (`interrupt`), работа завершается ошибкой, и только
//    ПОСЛЕ этого ход получает следующий. Следующий писатель никогда не
//    стартует поверх ещё работающего — «Promise.race и дальше» нарушил бы
//    сериализацию;
//  - повторный вход из уже владеющего контекста (AsyncLocalStorage) исполняется
//    сразу — вложенный вызов не может заблокировать сам себя.

import { AsyncLocalStorage } from 'node:async_hooks'

export class WriteQueueFullError extends Error {
  constructor(limit: number) {
    super(`Stats write queue is full (${String(limit)} pending)`)
    this.name = 'WriteQueueFullError'
  }
}

export class WriteQueueTimeoutError extends Error {
  constructor(waitedMs: number) {
    super(`Stats write waited ${String(waitedMs)} ms in the queue and was dropped`)
    this.name = 'WriteQueueTimeoutError'
  }
}

export interface WriteOptions {
  /** Abort while still queued: the work is not run. */
  signal?: AbortSignal
  /** Max time to wait for the turn (default: queue default). */
  waitTimeoutMs?: number
  /** Short label for diagnostics (oldest/head owner). */
  label?: string
}

interface Interruptible {
  interrupt(): void
}

interface Job {
  fn: () => Promise<unknown>
  resolve: (v: unknown) => void
  reject: (e: unknown) => void
  enqueuedAt: number
  label: string
  timer?: ReturnType<typeof setTimeout>
  onAbort?: () => void
  signal?: AbortSignal
  startedAt?: number
  /** Native connection of the call currently executing for this job. */
  activeConn?: Interruptible | null
  interrupted?: boolean
}

const limits = {
  maxPending: Number(process.env.OCB_STATS_WRITE_MAX_PENDING) || 512,
  waitTimeoutMs: Number(process.env.OCB_STATS_WRITE_WAIT_MS) || 120_000,
  headTimeoutMs: Number(process.env.OCB_STATS_WRITE_HEAD_MS) || 120_000,
}

const owner = new AsyncLocalStorage<Job>()
const queue: Job[] = []
let head: Job | null = null
let headWatch: ReturnType<typeof setInterval> | null = null

/** Test/ops hook: adjust the queue limits. */
export function configureStatsWriteQueue(next: Partial<typeof limits>): void {
  Object.assign(limits, next)
}

/** True inside a running write job (its calls need no new turn). */
export function insideStatsWrite(): boolean {
  return owner.getStore() !== undefined
}

/** Record the native connection a write call runs on, so a stuck head can be
 *  interrupted. Returns a release function. */
export function trackWriteCall(conn: Interruptible): () => void {
  const job = owner.getStore()
  if (!job) return () => {}
  job.activeConn = conn
  return () => {
    if (job.activeConn === conn) job.activeConn = null
  }
}

/** Выполнить пишущую операцию строго после всех ранее поставленных. */
export function withStatsWrite<T>(fn: () => Promise<T>, opts: WriteOptions = {}): Promise<T> {
  if (owner.getStore()) return fn()
  if (opts.signal?.aborted) return Promise.reject(opts.signal.reason ?? new Error('aborted'))
  if (queue.length + (head ? 1 : 0) >= limits.maxPending) {
    return Promise.reject(new WriteQueueFullError(limits.maxPending))
  }
  return new Promise<T>((resolve, reject) => {
    const job: Job = {
      fn,
      resolve: resolve as (v: unknown) => void,
      reject,
      enqueuedAt: Date.now(),
      label: opts.label ?? 'write',
      signal: opts.signal,
    }
    const wait = opts.waitTimeoutMs ?? limits.waitTimeoutMs
    const drop = (err: unknown): void => {
      const i = queue.indexOf(job)
      if (i === -1) return // already running or done
      queue.splice(i, 1)
      cleanup(job)
      reject(err)
    }
    job.timer = setTimeout(() => drop(new WriteQueueTimeoutError(Date.now() - job.enqueuedAt)), wait)
    if (opts.signal) {
      job.onAbort = () => drop(opts.signal!.reason ?? new Error('aborted'))
      opts.signal.addEventListener('abort', job.onAbort, { once: true })
    }
    queue.push(job)
    pump()
  })
}

function cleanup(job: Job): void {
  if (job.timer) clearTimeout(job.timer)
  if (job.onAbort) job.signal?.removeEventListener('abort', job.onAbort)
}

function pump(): void {
  if (head || queue.length === 0) return
  const job = queue.shift()!
  cleanup(job)
  head = job
  job.startedAt = Date.now()
  ensureWatch()
  let p: Promise<unknown>
  try {
    p = owner.run(job, () => job.fn())
  } catch (e) {
    p = Promise.reject(e)
  }
  p.then(
    (v) => job.resolve(v),
    (e) => job.reject(e),
  ).finally(() => {
    head = null
    pump()
  })
}

function ensureWatch(): void {
  if (headWatch) return
  headWatch = setInterval(() => {
    if (!head) {
      if (queue.length === 0 && headWatch) {
        clearInterval(headWatch)
        headWatch = null
      }
      return
    }
    const age = Date.now() - (head.startedAt ?? Date.now())
    if (age > limits.headTimeoutMs && !head.interrupted && head.activeConn) {
      // Ask DuckDB to cancel the running statement; the job then fails on its
      // own and only then releases the turn.
      head.interrupted = true
      try {
        head.activeConn.interrupt()
      } catch {
        /* connection already gone */
      }
    }
  }, 1000)
  headWatch.unref?.()
}

/** Сколько писателей ждут или исполняются — для диагностики и тестов. */
export function statsWriteDepth(): number {
  return queue.length + (head ? 1 : 0)
}

/** Queue diagnostics: depth, oldest wait and the running head. */
export function statsWriteQueueInfo(): {
  depth: number
  oldestWaitMs: number
  head: { label: string; runningMs: number; interrupted: boolean } | null
} {
  const now = Date.now()
  return {
    depth: statsWriteDepth(),
    oldestWaitMs: queue.length ? now - queue[0]!.enqueuedAt : 0,
    head: head
      ? { label: head.label, runningMs: now - (head.startedAt ?? now), interrupted: !!head.interrupted }
      : null,
  }
}
