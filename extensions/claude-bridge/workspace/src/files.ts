import { resolve } from 'node:path'
import { FILE_INVOKES, filePath, invokeFile } from './file-operations.js'
import { FILE_SENDS, FileWatches } from './file-watches.js'
import type { ClientView, HostAttachment, WorkspaceBackend } from './protocol.js'

export interface WorkspaceFilesOptions {
  root?: string
  /** Forward asynchronous watch errors to the host's shared logger/UI. */
  onWatchError(error: Error): void
}

/** Container environment services for the existing file panel, not agent tools. */
export class WorkspaceFiles implements WorkspaceBackend {
  private root: string
  private views = new Set<HostAttachment>()
  private disposed = false

  constructor(private backend: WorkspaceBackend, private options: WorkspaceFilesOptions) {
    this.root = resolve(options.root ?? '/workspace')
  }

  attach(view: ClientView): HostAttachment {
    if (this.disposed) throw new Error('Workspace files disposed')
    const child = this.backend.attach(view)
    const watches = new FileWatches(view, error => this.options.onWatchError(error))
    let disposed = false
    const attachment: HostAttachment = {
      receive: async frame => {
        if (disposed) throw new Error('File view disposed')
        if (frame.kind === 'invoke' && (FILE_INVOKES as readonly string[]).includes(frame.channel)) {
          let reply: Record<string, unknown>
          try { reply = { ok: true, result: await invokeFile(this.root, frame.channel, frame.args) } }
          catch (error) { reply = { ok: false, error: error instanceof Error ? error.message : String(error) } }
          if (!disposed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ...reply })
        } else if (frame.kind === 'send' && (FILE_SENDS as readonly string[]).includes(frame.channel)) {
          if (frame.args.length !== 1) throw new Error('Expected one file watch path')
          watches.receive(frame.channel, filePath(this.root, frame.args[0]))
        } else await child.receive(frame)
      },
      dispose: () => {
        if (disposed) return
        disposed = true
        this.views.delete(attachment)
        watches.dispose()
        child.dispose()
      },
    }
    this.views.add(attachment)
    return attachment
  }

  /** Dispose views before the embedding application disposes the shared host. */
  dispose(): void {
    if (this.disposed) return
    this.disposed = true
    const errors: unknown[] = []
    for (const view of this.views) {
      try { view.dispose() } catch (error) { errors.push(error) }
    }
    if (errors.length) throw new AggregateError(errors, 'File backend disposal failed')
  }
}
