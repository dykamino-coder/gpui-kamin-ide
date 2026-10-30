export interface RunningCommand {
  id: string
  tool: string
  command: string
  background: boolean
  status: 'running' | 'stopping' | 'disconnected'
  startedAt: number
}
export interface CommandSnapshot {
  sessionId: string
  commands: RunningCommand[]
  /** False when only local shells are known (older or disconnected server). */
  serverAvailable?: boolean
}
export interface ShellSnapshot {
  id: string
  tool: string
  command: string
  startedAt: number
  running: boolean
  output: string
}
export interface CommandReport {
  calls: Array<{ id: string; output: string }>
  shells: ShellSnapshot[]
}
