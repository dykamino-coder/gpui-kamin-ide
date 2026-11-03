// Admission failures describe a pre-dispatch boundary, never a tool outcome.
export type TaskAdmissionReason =
  'active_owner' | 'active_global' | 'retained_owner' | 'retained_global' | 'retained_bytes' | 'input_bytes'

export class TaskAdmissionError extends Error {
  constructor(readonly reason: TaskAdmissionReason) {
    super(`Task capacity exceeded: ${reason}`)
    this.name = 'TaskAdmissionError'
  }
}

export function isTaskRetentionPressure(error: unknown): error is TaskAdmissionError {
  return error instanceof TaskAdmissionError && error.reason.startsWith('retained_')
}

export class TaskPersistenceQueueError extends Error {
  constructor() {
    super('Task persistence queue full')
    this.name = 'TaskPersistenceQueueError'
  }
}
