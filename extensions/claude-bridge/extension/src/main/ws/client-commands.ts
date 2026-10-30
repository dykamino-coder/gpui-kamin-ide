// Merge server tasks with the tab's authoritative local legacy shell registry.
import type { CommandSnapshot } from '../../shared/commands'
import { localBackgroundShellSnapshot, stopLocalBackgroundShell, clearLocalCompletedShells } from '../mcp/tools/shell'

export class ClientCommands {
  private remote?: { snapshot: CommandSnapshot; at: number }

  constructor(
    private readonly tabId: string,
    private readonly publish: (event: string, value: unknown) => void,
  ) {}

  receive(snapshot: CommandSnapshot): void {
    this.remote = { snapshot, at: Date.now() }
    this.list(snapshot.sessionId)
  }

  reset(): void {
    this.remote = undefined
  }

  list(sessionId: string): void {
    const shells = localBackgroundShellSnapshot(this.tabId)
    const serverAvailable = this.remote?.snapshot.sessionId === sessionId && Date.now() - this.remote.at < 5000
    const commands: CommandSnapshot['commands'] = serverAvailable
      ? this.remote!.snapshot.commands.filter((c) => !shells.some((s) => s.id === c.id))
      : []
    for (const shell of shells) {
      if (!shell.hiddenFromCommands) {
        commands.push({
          id: shell.id,
          tool: shell.tool,
          command: shell.command,
          description: shell.description,
          finishedAt: shell.finishedAt,
          exitCode: shell.exitCode,
          startedAt: shell.startedAt,
          background: true,
          status: shell.running
            ? shell.stopped
              ? 'stopping'
              : 'running'
            : shell.stopped
              ? 'stopped'
              : shell.exitCode === 0
                ? 'completed'
                : 'failed',
        })
      }
    }
    commands.sort((a, b) => Number(b.background) - Number(a.background) || a.startedAt - b.startedAt)
    this.publish('commands:snapshot', {
      sessionId,
      commands,
      serverAvailable,
    } satisfies CommandSnapshot)
  }

  clear(): void {
    clearLocalCompletedShells(this.tabId)
    if (this.remote)
      this.remote.snapshot = {
        ...this.remote.snapshot,
        commands: this.remote.snapshot.commands.filter((c) =>
          ['running', 'stopping', 'disconnected'].includes(c.status),
        ),
      }
  }

  localAction(action: 'stop' | 'output', id: string): boolean {
    const shell = localBackgroundShellSnapshot(this.tabId).find((s) => s.id === id)
    if (!shell) return false
    if (action === 'stop') void stopLocalBackgroundShell(this.tabId, id)
    else this.publish('commands:output', { id, output: shell.output })
    return true
  }
}
