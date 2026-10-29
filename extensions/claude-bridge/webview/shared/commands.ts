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
