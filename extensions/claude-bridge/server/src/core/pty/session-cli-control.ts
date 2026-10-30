// Main-turn lifecycle and terminal status share one precedence decision.
import type { PtySession } from './types'
import { sendToClient } from './session-io'
import { getSessionInputSnapshot, setSessionPromptReady, submitCoordinatedText } from './session-input-coordinator'
import { ProgramStatusParser, type ProgramStatus } from './program-status'
import { CliSubmissions } from './cli-submissions'
import type { CliModTransport } from './cli-mod-transport'

const controls = new WeakMap<PtySession, SessionCliControl>()

export class SessionCliControl {
  readonly parser: ProgramStatusParser
  private osc: ProgramStatus | undefined
  private turnId: string | null = null
  private modReady = false
  private submissions?: CliSubmissions

  constructor(
    private readonly session: PtySession,
    private readonly mod?: CliModTransport,
  ) {
    controls.set(session, this)
    this.parser = new ProgramStatusParser(
      (state) => {
        this.osc = state === 'clear' ? undefined : state
        this.snapshot()
      },
      () => session.pty.write('\x1b]7501;?\x1b\\'),
    )
    if (mod) {
      const timeout = setTimeout(() => {
        if (session.state === 'running' && !mod?.healthy()) this.warnFallback()
      }, 15_000)
      timeout.unref()
    }
    if (mod) {
      this.submissions = new CliSubmissions(
        session,
        mod,
        () => this.busy(),
        () => this.interrupt(),
      )
      mod.onTurn = (turnId) => {
        const changed = !this.modReady || turnId !== this.turnId
        this.modReady = true
        this.turnId = turnId
        if (changed) {
          // A terminal blocked report from the previous turn cannot survive a boundary.
          if (this.osc === 'blocked') this.osc = undefined
          this.snapshot()
        }
      }
      mod.onUnavailable = () => {
        this.modReady = false
        this.turnId = null
        this.submissions?.uncertain()
        this.snapshot()
        this.warnFallback()
      }
      mod.onUncertain = () => {
        this.submissions?.uncertain()
        sendToClient(session.ws, {
          type: 'session:output',
          sessionId: session.id,
          data: '\r\n[Bridge: mod delivery unconfirmed; check the transcript before resending.]\r\n',
        })
      }
    }
  }

  private warnFallback(): void {
    sendToClient(this.session.ws, {
      type: 'session:output',
      sessionId: this.session.id,
      data: '\r\n[Bridge: CLI mod unavailable or incompatible (requires 2.1.312); using native terminal delivery and OSC status.]\r\n',
    })
  }

  private busy(): boolean {
    // OSC is synchronous with PTY output; the mod heartbeat can arrive later.
    return this.turnId !== null || this.osc === 'working' || this.osc === 'blocked'
  }

  authoritative(): boolean {
    return !!(this.mod?.healthy() && this.modReady) || this.osc !== undefined
  }

  snapshot(): void {
    const hasMod = !!this.mod?.healthy() && this.modReady
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id: '',
      capable: hasMod && (this.submissions?.capable ?? true),
    })
    if (!hasMod && this.osc === undefined) return
    const waiting = this.osc === 'blocked'
    const isWorking = hasMod ? this.turnId !== null : this.osc === 'working' || waiting
    const promptReady = !isWorking && !waiting
    setSessionPromptReady(this.session, promptReady)
    sendToClient(this.session.ws, {
      type: 'session:activity',
      sessionId: this.session.id,
      hookDriven: true,
      isWorking,
      waiting,
      promptReady,
      rawTitle: '',
      cliMod: hasMod,
    })
  }

  replay(): void {
    this.snapshot()
    this.submissions?.replay()
  }

  rawInput(data: string): void {
    // Up can withdraw a native queued item without an addressed receipt. Stop
    // correlating this process rather than attach a later UUID to the wrong ID.
    this.submissions?.rawInput(data)
  }

  action(id: string, action: string): void {
    this.submissions?.action(id, action)
  }

  submit(text: string, submissionId?: string): boolean {
    const input = getSessionInputSnapshot(this.session)
    // Engine submit intentionally does not expand mentions, pastes or native commands.
    if (
      /^\s*\//.test(text) ||
      /(^|\s)@\S/.test(text) ||
      /\[Image(?:\s|#|\])/.test(text) ||
      /(?:^|\n)(?:\/|[A-Za-z]:[\\/])\S/.test(text) ||
      (!submissionId &&
        (input.rawInputDirty || input.activeKind || input.queuedSubmissions || input.pendingMaintenance.length))
    )
      return false
    if (submissionId) return this.submissions?.submit(submissionId, text) ?? false
    // Older clients retain native mid-turn semantics, including their queue mirror.
    if (this.busy()) return false
    return this.mod?.submit(text, () => submitCoordinatedText(this.session, text)) ?? false
  }

  interrupt(): boolean {
    if (!this.turnId) return false
    return this.mod?.abort(this.turnId, () => this.session.pty.write('\x03')) ?? false
  }
}

export function sessionCliControl(session: PtySession): SessionCliControl | undefined {
  return controls.get(session)
}
