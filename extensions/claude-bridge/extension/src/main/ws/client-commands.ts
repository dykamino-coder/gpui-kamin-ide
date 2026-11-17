// Merge server tasks with the tab's authoritative local legacy shell registry.
import type { CommandSnapshot } from '../../shared/commands'
import { localBackgroundShellSnapshot, stopLocalBackgroundShell, clearLocalCompletedShells } from '../mcp/tools/shell'

// The server answers each of its own `commands:request` rounds with a snapshot:
// every second while something runs, every 10 s when idle. A snapshot younger
// than SERVER_FRESH_MS therefore means the server still serves this session.
const SERVER_FRESH_MS = 15_000
// Ask the server explicitly only when its own rounds have not refreshed us.
const SERVER_ASK_AFTER_MS = 12_000

export class ClientCommands {
  private remote?: { snapshot: CommandSnapshot; at: number }
  private published?: string

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
    this.published = undefined
  }

  /** Whether a `commands:list` request is worth sending: the server's own
   *  rounds keep the snapshot fresh, so an idle chat sends none. */
  needsServerList(sessionId: string): boolean {
    return this.remote?.snapshot.sessionId !== sessionId || Date.now() - this.remote.at > SERVER_ASK_AFTER_MS
  }

  /** Publish the merged snapshot when it differs from the last one published,
   *  or always with `force` (a view that just mounted has none). */
  list(sessionId: string, force = false): void {
    const shells = localBackgroundShellSnapshot(this.tabId)
    const serverAvailable = this.remote?.snapshot.sessionId === sessionId && Date.now() - this.remote.at < SERVER_FRESH_MS
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
    const snapshot = { sessionId, commands, serverAvailable } satisfies CommandSnapshot
    const key = JSON.stringify(snapshot)
    if (!force && key === this.published) return
    this.published = key
    this.publish('commands:snapshot', snapshot)
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
