import { createHmac, randomBytes } from 'node:crypto'

const key = randomBytes(32)
const boot = randomBytes(8).toString('hex')
const MAX_INPUT_CHARS = 256
const MAX_ID = Number.MAX_SAFE_INTEGER
const rendererStages = new Set(['renderer-sent', 'renderer-received', 'renderer-duplicate', 'renderer-unknown', 'renderer-other-document', 'renderer-ended', 'renderer-send-threw'])
const knownChannels = new Set(['get-config', 'tab:list', 'tab:get-active', 'tabs:restore-state',
  'hooks:set-plugin-approval', 'hooks:get-plugin-approval', 'hooks:list-pending-plugin-approvals',
  'plugins:list-marketplaces', 'plugins:list-installed', 'mcp:get-servers', 'session:get-saved'])

export interface SafeInvokeBoundary {
  event: 'invoke-boundary'
  boot: string
  view: number
  documentRef: string
  channelRef: string
  channel: string
  id: number
  stage: 'host-received' | 'handler-resolved' | 'handler-rejected' | 'handler-unregistered'
    | 'reply-accepted' | 'reply-false' | 'reply-rejected' | 'reply-source-disposed' | 'reply-document-replaced'
    | 'view-detached' | 'renderer-sent' | 'renderer-received' | 'renderer-duplicate' | 'renderer-unknown'
    | 'renderer-other-document' | 'renderer-ended' | 'renderer-send-threw'
  hidden: boolean
  elapsedMs: number
  count: number
}
interface ViewState { id: number; documentRef: string; detached: boolean; hidden: boolean }

function ref(domain: string, value: unknown): string {
  // Reject unbounded input rather than storing/hash-processing a payload.
  const input = typeof value === 'string' && value.length <= MAX_INPUT_CHARS ? value : 'unavailable'
  return createHmac('sha256', key).update(domain).update(input).digest('hex').slice(0, 24)
}
function safeId(value: unknown): number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 && value <= MAX_ID ? value : 0
}

/** Observes only; no deadlines, retries, cancellation or reply reconciliation. */
export class InvokeDiagnostics<T extends object> {
  private readonly views = new WeakMap<T, ViewState>()
  private sequence = 0
  private readonly counts = new Map<SafeInvokeBoundary['stage'], number>()
  constructor(private readonly emit: (event: SafeInvokeBoundary) => void) {}

  attach(view: T): void {
    this.views.set(view, { id: ++this.sequence, documentRef: ref('document', undefined), detached: false, hidden: false })
  }
  private state(view: T): ViewState {
    if (!this.views.has(view)) this.attach(view)
    return this.views.get(view)!
  }
  setHidden(view: T, hidden: boolean): void { this.state(view).hidden = hidden }
  detach(view: T): void {
    const state = this.state(view)
    state.detached = true
    this.record(state, state.documentRef, '', 0, 'view-detached', 0)
  }
  private record(state: ViewState, documentRef: string, channel: unknown, id: unknown, stage: SafeInvokeBoundary['stage'], elapsedMs: number): void {
    const count = Math.min(1_000_000_000, (this.counts.get(stage) ?? 0) + 1)
    this.counts.set(stage, count)
    try {
      this.emit({ event: 'invoke-boundary', boot, view: state.id, documentRef,
        channelRef: ref('channel', channel), channel: typeof channel === 'string' && knownChannels.has(channel) ? channel : 'other',
        id: safeId(id), stage, hidden: state.hidden, count,
        elapsedMs: Math.min(1_000_000_000, Math.max(0, Math.trunc(elapsedMs))) })
    } catch { /* diagnostic sink failure must not alter handler or delivery */ }
  }
  renderer(view: T, raw: unknown): void {
    if (!raw || typeof raw !== 'object') return
    const message = raw as Record<string, unknown>
    if (typeof message.stage !== 'string' || !rendererStages.has(message.stage)) return
    const state = this.state(view)
    const documentRef = ref('document', message.generation)
    if (message.stage === 'renderer-sent') state.documentRef = documentRef
    this.record(state, documentRef, message.channel, message.id, message.stage as SafeInvokeBoundary['stage'], 0)
  }
  begin(view: T, id: unknown, channel: unknown, generation: unknown) {
    const state = this.state(view)
    const documentRef = ref('document', generation)
    state.documentRef = documentRef
    const started = Date.now()
    const record = (stage: SafeInvokeBoundary['stage']) => this.record(state, documentRef, channel, id, stage, Date.now() - started)
    record('host-received')
    return {
      handler: (outcome: 'resolved' | 'rejected' | 'unregistered') => record(`handler-${outcome}`),
      reply: async (send: () => PromiseLike<boolean>): Promise<void> => {
        if (state.detached) record('reply-source-disposed')
        if (state.documentRef !== documentRef) record('reply-document-replaced')
        // Preserve the existing attempt even for hidden/replaced views. This
        // phase diagnoses the boundary; it does not assert transport recovery.
        try { record(await send() ? 'reply-accepted' : 'reply-false') }
        catch { record('reply-rejected') }
      },
    }
  }
}
