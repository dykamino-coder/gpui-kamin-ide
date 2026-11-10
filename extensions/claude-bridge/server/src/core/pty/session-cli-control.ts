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
  private oscWorkingTurnId: string | null = null
  private oscDoneTurnId: string | null = null
  private turnId: string | null = null
  private modReady = false
  private submissions?: CliSubmissions

  constructor(
    private readonly session: PtySession,
    private readonly mod?: CliModTransport,
  ) {
    controls.set(session, this)
    this.parser = new ProgramStatusParser(
      (state) => this.onProgramStatus(state),
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
          if (this.osc === 'working' && this.oscWorkingTurnId === null) this.oscWorkingTurnId = turnId
          // A terminal blocked report from the previous turn cannot survive a boundary.
          if (this.osc === 'blocked') this.osc = undefined
          this.snapshot()
        }
      }
      mod.onUnavailable = () => {
        this.modReady = false
        this.turnId = null
        this.oscWorkingTurnId = null
        this.oscDoneTurnId = null
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
      data: '\r\n[Bridge: CLI mod unavailable or incompatible (requires 2.1.322); using native terminal delivery and OSC status.]\r\n',
    })
  }

  private onProgramStatus(state: ProgramStatus): void {
    // Pair a main PTY working/done cycle with the mod turn that was active
    // when it started. A delayed done cannot release a newer mod turn.
    if (state === 'working' && this.oscWorkingTurnId === null) {
      this.oscWorkingTurnId = this.turnId
      this.oscDoneTurnId = null
    } else if (state === 'done' && this.oscWorkingTurnId !== null) {
      this.oscDoneTurnId = this.oscWorkingTurnId
      this.oscWorkingTurnId = null
    } else if (state === 'idle' || state === 'error' || state === 'clear') {
      this.oscWorkingTurnId = null
      if (state === 'clear') this.oscDoneTurnId = null
    }
    this.osc = state === 'clear' ? undefined : state
    this.snapshot()
  }

  private modTurnWorking(): boolean {
    return this.turnId !== null && this.turnId !== this.oscDoneTurnId
  }

  private busy(): boolean {
    // OSC is synchronous with PTY output; the mod heartbeat can arrive later.
    return this.modTurnWorking() || this.osc === 'working' || this.osc === 'blocked'
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
    const isWorking = hasMod ? this.modTurnWorking() : this.osc === 'working' || waiting
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
    // After a server or CLI restart the client may still show receipts this
    // process never saw; list what is tracked so it can stop waiting for them.
    sendToClient(this.session.ws, {
      type: 'session:submission',
      sessionId: this.session.id,
      id: '',
      known: this.submissions?.ids() ?? [],
    })
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
    if (!this.turnId || !this.modTurnWorking()) return false
    return this.mod?.abort(this.turnId, () => this.session.pty.write('\x03')) ?? false
  }
}

export function sessionCliControl(session: PtySession): SessionCliControl | undefined {
  return controls.get(session)
}
