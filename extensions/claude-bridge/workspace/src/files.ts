import { resolve } from 'node:path'
import { FILE_INVOKES, filePath, invokeFile } from './file-operations.js'
import { FILE_SENDS } from './file-watches.js'
import { FileViews } from './file-views.js'
import type { ClientView, HostAttachment, WorkspaceBackend } from './protocol.js'
import { readFilePanelState } from '../../webview/shared/file-panel-state.js'

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
  private fileViews: FileViews

  constructor(private backend: WorkspaceBackend, options: WorkspaceFilesOptions) {
    this.root = resolve(options.root ?? '/workspace')
    this.fileViews = new FileViews(error => options.onWatchError(error))
  }

  attach(view: ClientView): HostAttachment {
    if (this.disposed) throw new Error('Workspace files disposed')
    const child = this.backend.attach(view)
    let logical: ReturnType<FileViews['create']> | undefined
    let disposed = false
    const attachment: HostAttachment = {
      receive: async frame => {
        if (disposed) throw new Error('File view disposed')
        if (frame.kind === 'send' && frame.channel === 'workspace:sync') {
          if (!logical) {
            const identity = frame.args[0] as { fileViewId?: unknown } | undefined
            logical = this.fileViews.attach(view, identity?.fileViewId)
          }
          await child.receive(frame)
          if (!disposed) {
            await view.postMessage({ kind: 'event', channel: 'workspace:files-state',
              args: [{ id: logical.id, ...logical.watches.paths(), panel: logical.panel }] })
            await logical.watches.snapshot({ postMessage: frame => disposed ? Promise.resolve(false) : view.postMessage(frame) })
          }
        } else if (frame.kind === 'send' && frame.channel === 'workspace:file-panel') {
          if (frame.args.length !== 1) throw new Error('Expected one FilePanel state')
          const panel = readFilePanelState(frame.args[0])
          if (!logical) logical = this.fileViews.attach(view, frame.fileViewId)
          logical.panel = panel
        } else if (frame.kind === 'invoke' && (FILE_INVOKES as readonly string[]).includes(frame.channel)) {
          let reply: Record<string, unknown>
          try { reply = { ok: true, result: await invokeFile(this.root, frame.channel, frame.args) } }
          catch (error) { reply = { ok: false, error: error instanceof Error ? error.message : String(error) } }
          if (!disposed) await view.postMessage({ kind: 'invoke-reply', id: frame.id, generation: frame.generation, ...reply })
        } else if (frame.kind === 'send' && (FILE_SENDS as readonly string[]).includes(frame.channel)) {
          if (frame.args.length !== 1) throw new Error('Expected one file watch path')
          if (!logical) logical = this.fileViews.attach(view, frame.fileViewId)
          logical.watches.receive(frame.channel, filePath(this.root, frame.args[0]))
        } else await child.receive(frame)
      },
      dispose: () => {
        if (disposed) return
        disposed = true
        this.views.delete(attachment)
        logical?.sockets.delete(view)
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
    this.fileViews.clear()
    const errors: unknown[] = []
    for (const view of this.views) {
      try { view.dispose() } catch (error) { errors.push(error) }
    }
    if (errors.length) throw new AggregateError(errors, 'File backend disposal failed')
  }
}
