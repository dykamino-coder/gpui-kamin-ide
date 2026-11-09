// The browser carries existing BridgeHost frames; no alternate tool protocol.
export interface InvokeFrame {
  kind: 'invoke'
  id: number
  generation: string
  channel: string
  args: unknown[]
}

export interface SendFrame {
  kind: 'send'
  channel: string
  args: unknown[]
  /** Browser's opaque host logical file view; never authentication. */
  fileViewId?: string
}

export interface DiagnosticFrame {
  kind: 'invoke-diagnostic'
  id: number
  generation: string
  channel: string
  stage: string
}

export type ClientFrame = InvokeFrame | SendFrame | DiagnosticFrame
export type HostFrame = Record<string, unknown>

export interface ClientView {
  /** Returns false after close or when the transport's bounded buffer is full. */
  postMessage(frame: HostFrame): Promise<boolean>
}

export interface HostAttachment {
  /** Unsupported channels must reject explicitly, never resolve null success. */
  receive(frame: ClientFrame): Promise<void>
  dispose(): void
}

/** One runtime per workspace; each socket gets a separately owned view. */
export interface WorkspaceBackend {
  attach(view: ClientView): HostAttachment
}

export function parseClientFrame(raw: string): ClientFrame {
  const frame: unknown = JSON.parse(raw)
  if (!frame || typeof frame !== 'object' || Array.isArray(frame)) throw new Error('Invalid frame')
  const f = frame as Record<string, unknown>
  if (f.kind === 'invoke-diagnostic') {
    if (!['renderer-sent', 'renderer-received', 'renderer-duplicate', 'renderer-unknown',
      'renderer-other-document', 'renderer-ended', 'renderer-send-threw'].includes(String(f.stage)) ||
      !Number.isSafeInteger(f.id) || (f.id as number) < 0 ||
      typeof f.channel !== 'string' || f.channel.length > 256 ||
      typeof f.generation !== 'string' || f.generation.length > 256) throw new Error('Invalid diagnostic')
    return f as unknown as DiagnosticFrame
  }
  if (f.kind !== 'invoke' && f.kind !== 'send') throw new Error('Invalid frame kind')
  if (typeof f.channel !== 'string' || !f.channel || f.channel.length > 256) throw new Error('Invalid channel')
  if (!Array.isArray(f.args) || f.args.length > 32) throw new Error('Invalid arguments')
  if (f.kind === 'invoke' && (
    !Number.isSafeInteger(f.id) || (f.id as number) < 1 ||
    typeof f.generation !== 'string' || !f.generation || f.generation.length > 128
  )) throw new Error('Invalid request identity')
  return f as unknown as ClientFrame
}
