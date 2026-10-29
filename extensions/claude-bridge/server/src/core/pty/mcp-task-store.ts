// Durable task owner. Serialized, bounded admission and atomic settlement happen
// before HTTP handles / WS acknowledgements; transport loss never repeats a tool.
import { promises as fs } from 'node:fs'
import path from 'node:path'
import { randomUUID } from 'node:crypto'

export const TASK_LIMITS = {
  active: 16,
  activePerOwner: 4,
  retained: 256,
  perOwner: 64,
  bytes: 64 * 1024 * 1024,
  resultBytes: 512 * 1024,
  inputBytes: 64 * 1024,
  ttlMs: 86_400_000,
  executionMs: 1_800_000,
  backgroundExecutionMs: 86_340_000, // Leave cancellation grace before the 24-hour retention expires.
  // A foreground command that settles within this window answers inline, like
  // native Bash's default 2-minute timeout; only longer runs stay a task.
  foregroundGraceMs: 120_000,
  queued: 32,
}
export interface TaskRecord {
  taskId: string
  owner: string
  toolName: string
  input: Record<string, unknown>
  status: 'working' | 'completed' | 'failed' | 'cancelled'
  createdAt: string
  lastUpdatedAt: string
  ttlMs: number
  pollIntervalMs: number
  executionDeadlineMs?: number
  background?: boolean
  command?: string
  cancelRequested?: boolean
  delivered?: boolean
  result?: Record<string, unknown>
  error?: { code: number; message: string }
}
export function taskView(task: TaskRecord): Record<string, unknown> {
  const {
    owner: _owner,
    input: _input,
    toolName: _tool,
    delivered: _delivered,
    cancelRequested: _cancel,
    executionDeadlineMs: _deadline,
    background: _background,
    command: _command,
    ...view
  } = task
  return view
}

export function taskDeadline(task: TaskRecord): number {
  return task.executionDeadlineMs ?? Date.parse(task.createdAt) + TASK_LIMITS.executionMs
}

export class McpTaskStore {
  private records = new Map<string, TaskRecord>()
  private tail: Promise<unknown> = Promise.resolve()
  private queued = 0
  private loaded = false
  constructor(
    private directory: string,
    private now = Date.now,
  ) {}

  private run<T>(work: () => Promise<T>): Promise<T> {
    if (this.queued >= TASK_LIMITS.queued) return Promise.reject(new Error('Task persistence queue full'))
    this.queued++
    const result = this.tail.then(async () => {
      await this.load()
      await this.expire()
      return work()
    })
    this.tail = result
      .catch(() => {})
      .finally(() => {
        this.queued--
      })
    return result
  }
  private async load(): Promise<void> {
    if (this.loaded) return
    await fs.mkdir(this.directory, { recursive: true, mode: 0o700 })
    const files = (await fs.readdir(this.directory)).filter((name) => /^[a-f0-9-]{36}\.json$/.test(name))
    if (files.length > TASK_LIMITS.retained) throw new Error('Task storage capacity exceeded')
    const loaded = new Map<string, TaskRecord>()
    let bytes = 0
    for (const name of files) {
      const file = path.join(this.directory, name)
      const size = (await fs.stat(file)).size
      bytes += size
      if (size > TASK_LIMITS.resultBytes + TASK_LIMITS.inputBytes + 8192 || bytes > TASK_LIMITS.bytes)
        throw new Error('Task storage byte capacity exceeded')
      const task = JSON.parse(await fs.readFile(file, 'utf8')) as TaskRecord
      if (
        task.taskId + '.json' !== name ||
        typeof task.owner !== 'string' ||
        typeof task.toolName !== 'string' ||
        !task.input ||
        typeof task.input !== 'object' ||
        Array.isArray(task.input) ||
        !Number.isFinite(Date.parse(task.createdAt)) ||
        !Number.isFinite(task.ttlMs) ||
        task.ttlMs <= 0 ||
        task.ttlMs > TASK_LIMITS.ttlMs ||
        (task.executionDeadlineMs !== undefined &&
          (!Number.isFinite(task.executionDeadlineMs) ||
            task.executionDeadlineMs > Date.parse(task.createdAt) + task.ttlMs ||
            task.executionDeadlineMs < Date.parse(task.createdAt))) ||
        !['working', 'completed', 'failed', 'cancelled'].includes(task.status)
      )
        throw new Error('Invalid task record')
      loaded.set(task.taskId, task)
    }
    this.records = loaded
    this.loaded = true
  }
  private async expire(): Promise<void> {
    for (const [id, task] of this.records) {
      if (this.now() < Date.parse(task.createdAt) + task.ttlMs) continue
      await this.remove(id)
    }
  }
  private async remove(id: string): Promise<void> {
    await fs.unlink(path.join(this.directory, `${id}.json`)).catch((error: NodeJS.ErrnoException) => {
      if (error.code !== 'ENOENT') throw error
    })
    this.records.delete(id)
  }
  // Retention bounds storage, not admission: settled outcomes stay readable until
  // room is needed, then the least recently settled ones make way for new work.
  // Only working tasks count against admission (INC-2026-0082).
  private async evictSettled(owner: string, incomingBytes: number): Promise<void> {
    const settled = [...this.records.values()]
      .filter((task) => task.status !== 'working')
      .sort((a, b) => Date.parse(a.lastUpdatedAt) - Date.parse(b.lastUpdatedAt))
    let bytes = incomingBytes
    for (const task of this.records.values()) bytes += Buffer.byteLength(JSON.stringify(task))
    const ownCount = () => [...this.records.values()].filter((task) => task.owner === owner).length
    for (const task of settled) {
      const full =
        this.records.size >= TASK_LIMITS.retained ||
        (task.owner === owner && ownCount() >= TASK_LIMITS.perOwner) ||
        bytes > TASK_LIMITS.bytes
      if (!full) continue
      bytes -= Buffer.byteLength(JSON.stringify(task))
      await this.remove(task.taskId)
    }
  }
  private async save(task: TaskRecord): Promise<void> {
    const text = JSON.stringify(task)
    let bytes = Buffer.byteLength(text)
    for (const other of this.records.values())
      if (other.taskId !== task.taskId) bytes += Buffer.byteLength(JSON.stringify(other))
    if (bytes > TASK_LIMITS.bytes) throw new Error('Task storage byte capacity exceeded')
    const file = path.join(this.directory, `${task.taskId}.json`)
    const temporary = file + '.tmp'
    try {
      const handle = await fs.open(temporary, 'w', 0o600)
      try {
        await handle.writeFile(text)
        await handle.sync()
      } finally {
        await handle.close()
      }
      await fs.rename(temporary, file)
      // POSIX rename durability needs the directory entry flushed as well.
      if (process.platform !== 'win32') {
        const dir = await fs.open(this.directory, 'r')
        try {
          await dir.sync()
        } finally {
          await dir.close()
        }
      }
    } finally {
      await fs.unlink(temporary).catch((error: NodeJS.ErrnoException) => {
        if (error.code !== 'ENOENT') throw error
      })
    }
    this.records.set(task.taskId, task)
  }
  create(owner: string, toolName: string, input: Record<string, unknown>): Promise<TaskRecord> {
    return this.run(async () => {
      const inputBytes = Buffer.byteLength(JSON.stringify(input))
      if (inputBytes > TASK_LIMITS.inputBytes) throw new Error('Task input too large')
      const working = [...this.records.values()].filter((task) => task.status === 'working')
      if (
        working.length >= TASK_LIMITS.active ||
        working.filter((task) => task.owner === owner).length >= TASK_LIMITS.activePerOwner
      )
        throw new Error('Task capacity exceeded')
      await this.evictSettled(owner, inputBytes + 2048)
      const all = [...this.records.values()]
      if (
        all.length >= TASK_LIMITS.retained ||
        all.filter((task) => task.owner === owner).length >= TASK_LIMITS.perOwner
      )
        throw new Error('Task capacity exceeded')
      const timestamp = new Date(this.now()).toISOString()
      const task: TaskRecord = {
        taskId: randomUUID(),
        owner,
        toolName,
        input,
        status: 'working',
        createdAt: timestamp,
        lastUpdatedAt: timestamp,
        ttlMs: TASK_LIMITS.ttlMs,
        pollIntervalMs: 2000,
        background: input.run_in_background === true,
        command: typeof input.command === 'string' ? input.command.slice(0, 512) : toolName,
        executionDeadlineMs:
          this.now() + (input.run_in_background === true ? TASK_LIMITS.backgroundExecutionMs : TASK_LIMITS.executionMs),
      }
      await this.save(task)
      return structuredClone(task)
    })
  }
  get(owner: string, id: string): Promise<TaskRecord | undefined> {
    return this.run(async () => {
      const task = this.records.get(id)
      return task?.owner === owner ? structuredClone(task) : undefined
    })
  }
  hasWorkingOwner(owner: string): boolean {
    return [...this.records.values()].some(
      (task) => task.owner === owner && task.status === 'working' && this.now() < taskDeadline(task) + 60_000,
    )
  }
  working(): Promise<TaskRecord[]> {
    return this.run(async () =>
      [...this.records.values()].filter((task) => task.status === 'working').map((task) => structuredClone(task)),
    )
  }
  update(owner: string, id: string, change: (task: TaskRecord) => void): Promise<TaskRecord | undefined> {
    return this.run(async () => {
      const previous = this.records.get(id)
      if (!previous || previous.owner !== owner) return undefined
      if (previous.status !== 'working') return structuredClone(previous)
      const task = structuredClone(previous)
      change(task)
      task.lastUpdatedAt = new Date(this.now()).toISOString()
      await this.save(task)
      return structuredClone(task)
    })
  }
  settle(owner: string, id: string, result: unknown, denied?: string): Promise<TaskRecord | undefined> {
    return this.update(owner, id, (task) => {
      if (task.cancelRequested) {
        task.status = 'cancelled'
        task.input = {}
        return
      }
      if (denied) {
        task.status = 'failed'
        task.error = { code: -32000, message: denied.slice(0, 1024) }
        task.input = {}
        return
      }
      let normalized =
        result &&
        typeof result === 'object' &&
        !Array.isArray(result) &&
        Array.isArray((result as Record<string, unknown>).content)
          ? (result as Record<string, unknown>)
          : { content: [{ type: 'text', text: typeof result === 'string' ? result : (JSON.stringify(result) ?? '') }] }
      if (Buffer.byteLength(JSON.stringify(normalized)) > TASK_LIMITS.resultBytes)
        normalized = {
          isError: true,
          content: [{ type: 'text', text: 'Tool result exceeded task storage limit; narrow the command output.' }],
        }
      task.status = 'completed'
      task.result = { ...normalized, resultType: 'complete' }
      task.input = {}
    })
  }
}
