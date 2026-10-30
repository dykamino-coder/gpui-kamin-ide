export interface RunningCommand {
  id: string
  tool: string
  command: string
  description?: string
  background: boolean
  status: 'running' | 'stopping' | 'disconnected' | 'completed' | 'failed' | 'stopped'
  startedAt: number
  finishedAt?: number
  exitCode?: number | null
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
  description?: string
  startedAt: number
  finishedAt?: number
  exitCode?: number | null
  stopped?: boolean
  hiddenFromCommands?: boolean
  running: boolean
  output: string
}
export interface CommandReport {
  calls: Array<{ id: string; output: string }>
  shells: ShellSnapshot[]
}
