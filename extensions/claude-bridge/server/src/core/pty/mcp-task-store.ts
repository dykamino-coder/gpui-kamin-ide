// Durable task owner. Serialized, bounded admission and atomic settlement happen
// before HTTP handles / WS acknowledgements; transport loss never repeats a tool.
import { promises as fs } from 'node:fs'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import { TaskAdmissionError, TaskPersistenceQueueError } from './mcp-task-admission'

// Sized for one server's ceiling of about 50 Bridge sessions. Commands run on the
// users' machines, so a working task costs the server only its record.
const SESSION_CEILING = 50
export const TASK_LIMITS = {
  activePerOwner: 8,
  active: SESSION_CEILING * 8,
  retained: 2048,
  perOwner: 256,
  // Includes worst-case reservations for 400 working results plus the old
  // 128 MiB store on upgrade. Terminal payloads are not held in the RAM index.
  bytes: 512 * 1024 * 1024,
  resultBytes: 512 * 1024,
  inputBytes: 64 * 1024,
  ttlMs: 86_400_000,
  executionMs: 1_800_000,
  backgroundExecutionMs: 86_340_000, // Leave cancellation grace before the 24-hour retention expires.
  // A foreground command that settles within this window answers inline, like
  // native Bash's default 2-minute timeout; only longer runs stay a task.
  foregroundGraceMs: 120_000,
  queued: 1024,
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
  description?: string
  hiddenFromCommands?: boolean
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
    description: _description,
    hiddenFromCommands: _hidden,
    ...view
  } = task
  return view
}

export function taskDeadline(task: TaskRecord): number {
  return task.executionDeadlineMs ?? Date.parse(task.createdAt) + TASK_LIMITS.executionMs
}

export class McpTaskStore {
  private records = new Map<string, TaskRecord>()
  private sizes = new Map<string, number>()
  private bytes = 0
  private tail: Promise<unknown> = Promise.resolve()
  private queued = 0
  private loaded = false
  private inlineOwners = new Map<string, string>()
  /** Waiters for a record leaving `working`, woken right after the record is saved. */
  private settledWaiters = new Map<string, Set<() => void>>()
  constructor(
    private directory: string,
    private now = Date.now,
  ) {}

  private run<T>(work: () => Promise<T>): Promise<T> {
    if (this.queued >= TASK_LIMITS.queued) return Promise.reject(new TaskPersistenceQueueError())
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
    const sizes = new Map<string, number>()
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
      loaded.set(task.taskId, this.metadata(task))
      sizes.set(task.taskId, size)
    }
    this.records = loaded
    this.sizes = sizes
    this.bytes = bytes
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
    this.bytes -= this.sizes.get(id) ?? 0
    this.sizes.delete(id)
    this.wake(id)
  }
  private metadata(task: TaskRecord): TaskRecord {
    const { result, ...metadata } = task
    return {
      ...metadata,
      ...(result ? { result: { isError: result.isError, exitCode: result.exitCode } } : {}),
    }
  }
  private async read(task: TaskRecord): Promise<TaskRecord> {
    if (task.status !== 'completed' || !task.result) return structuredClone(task)
    return JSON.parse(await fs.readFile(path.join(this.directory, `${task.taskId}.json`), 'utf8')) as TaskRecord
  }
  private cost(task: TaskRecord, size: number): number {
    // Input is released on settlement, but the full original command is retained.
    const maximumRecord = TASK_LIMITS.resultBytes + TASK_LIMITS.inputBytes + 8192
    return task.status === 'working' ? Math.max(size, maximumRecord) : size
  }
  private accountedBytes(): number {
    let bytes = 0
    for (const [id, task] of this.records) bytes += this.cost(task, this.sizes.get(id)!)
    return bytes
  }
  private checkActive(owner: string): void {
    const working = [...this.records.values()].filter((task) => task.status === 'working')
    const inlines = [...this.inlineOwners.values()]
    if (working.length + inlines.length >= TASK_LIMITS.active) throw new TaskAdmissionError('active_global')
    if (
      working.filter((task) => task.owner === owner).length + inlines.filter((id) => id === owner).length >=
      TASK_LIMITS.activePerOwner
    )
      throw new TaskAdmissionError('active_owner')
  }
  // Ordinary SSE calls cannot bypass working limits when durable retention is full.
  // Their leases are process-local, just like the existing ordinary request path.
  acquireInline(owner: string): Promise<() => void> {
    return this.run(async () => {
      this.checkActive(owner)
      const id = randomUUID()
      this.inlineOwners.set(id, owner)
      return () => {
        this.inlineOwners.delete(id)
      }
    })
  }
  private async save(task: TaskRecord): Promise<void> {
    const text = JSON.stringify(task)
    const size = Buffer.byteLength(text)
    const growth = size - (this.sizes.get(task.taskId) ?? 0)
    const previous = this.records.get(task.taskId)
    const previousCost = previous ? this.cost(previous, this.sizes.get(task.taskId)!) : 0
    if (this.accountedBytes() - previousCost + this.cost(task, size) > TASK_LIMITS.bytes)
      throw new TaskAdmissionError('retained_bytes')
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
    this.records.set(task.taskId, this.metadata(task))
    this.bytes += growth
    this.sizes.set(task.taskId, size)
    if (task.status !== 'working') this.wake(task.taskId)
  }
  private wake(id: string): void {
    const waiters = this.settledWaiters.get(id)
    if (!waiters) return
    this.settledWaiters.delete(id)
    for (const wake of waiters) wake()
  }
  /** Calls `wake` once when the task is saved in a terminal state; returns the
   *  unsubscribe. Callers re-read the record afterwards, so a spurious wake is safe. */
  onSettled(id: string, wake: () => void): () => void {
    let waiters = this.settledWaiters.get(id)
    if (!waiters) this.settledWaiters.set(id, (waiters = new Set()))
    waiters.add(wake)
    return () => {
      waiters!.delete(wake)
      if (waiters!.size === 0 && this.settledWaiters.get(id) === waiters) this.settledWaiters.delete(id)
    }
  }
  create(owner: string, toolName: string, input: Record<string, unknown>): Promise<TaskRecord> {
    return this.run(async () => {
      const inputBytes = Buffer.byteLength(JSON.stringify(input))
      if (inputBytes > TASK_LIMITS.inputBytes) throw new TaskAdmissionError('input_bytes')
      this.checkActive(owner)
      const all = [...this.records.values()]
      if (all.length >= TASK_LIMITS.retained) throw new TaskAdmissionError('retained_global')
      if (all.filter((task) => task.owner === owner).length >= TASK_LIMITS.perOwner)
        throw new TaskAdmissionError('retained_owner')
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
        description: typeof input.description === 'string' ? input.description.slice(0, 1024) : undefined,
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
      return task?.owner === owner ? this.read(task) : undefined
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
  // Project metadata without copying retained result bodies on every UI poll.
  commands(owner: string): Promise<TaskRecord[]> {
    return this.run(async () =>
      [...this.records.values()]
        .filter((task) => task.owner === owner && !task.hiddenFromCommands)
        .map(({ result, ...task }) => ({
          ...structuredClone(task),
          result: result ? { isError: result.isError, exitCode: result.exitCode } : undefined,
        })),
    )
  }
  clearCommands(owner: string): Promise<void> {
    return this.run(async () => {
      for (const task of this.records.values()) {
        if (task.owner !== owner || task.status === 'working' || task.hiddenFromCommands) continue
        await this.save({
          ...(await this.read(task)),
          hiddenFromCommands: true,
        })
      }
    })
  }
  update(owner: string, id: string, change: (task: TaskRecord) => void): Promise<TaskRecord | undefined> {
    return this.run(async () => {
      const previous = this.records.get(id)
      if (!previous || previous.owner !== owner) return undefined
      if (previous.status !== 'working') return this.read(previous)
      const task = structuredClone(previous)
      change(task)
      // Every terminal path releases input, including execution deadlines.
      if (task.status !== 'working' && typeof previous.input.command === 'string') task.command = previous.input.command
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
          : {
              content: [
                {
                  type: 'text',
                  text: typeof result === 'string' ? result : (JSON.stringify(result) ?? ''),
                },
              ],
            }
      if (Buffer.byteLength(JSON.stringify(normalized)) > TASK_LIMITS.resultBytes)
        normalized = {
          isError: true,
          content: [
            {
              type: 'text',
              text: 'Tool result exceeded task storage limit; narrow the command output.',
            },
          ],
        }
      task.status = 'completed'
      task.result = { ...normalized, resultType: 'complete' }
      task.input = {}
    })
  }
}
