import { randomUUID } from 'node:crypto'
import { FileWatches } from './file-watches.js'
import type { ClientView } from './protocol.js'
import type { FilePanelState } from '../../webview/shared/file-panel-state.js'

interface FileView {
  id: string
  sockets: Set<ClientView>
  watches: FileWatches
  panel?: FilePanelState
}

/** Logical views survive detach; pages retain only an opaque re-attachment ID. */
export class FileViews {
  private views = new Map<string, FileView>()
  constructor(private onError: (error: Error) => void) {}

  create(): FileView {
    if (this.views.size >= 32) throw new Error('Host file-view limit (32); restart the host to release views')
    const sockets = new Set<ClientView>()
    const view: FileView = { id: randomUUID(), sockets,
      watches: new FileWatches({ async postMessage(frame) {
        await Promise.all([...sockets].map(socket => socket.postMessage(frame)))
        return sockets.size > 0
      } }, this.onError) }
    this.views.set(view.id, view)
    return view
  }

  attach(socket: ClientView, id?: unknown): FileView {
    // A new document with no remembered ID adopts the most recent detached
    // view. A second concurrent page gets independent subscriptions copied
    // from the current host view, so unwatch cannot cancel the first page.
    const remembered = typeof id === 'string' ? this.views.get(id) : undefined
    const previous = remembered ?? [...this.views.values()].at(-1)
    let view = remembered?.sockets.size ? undefined : remembered
    if (!view && !remembered) view = [...this.views.values()].reverse().find(candidate => candidate.sockets.size === 0)
    if (!view) {
      view = this.create()
      try {
        if (previous) {
          view.panel = previous.panel && structuredClone(previous.panel)
          const paths = previous.watches.paths()
          for (const path of paths.files) view.watches.receive('file-viewer:watch', path)
          for (const path of paths.trees) view.watches.receive('file-tree:watch', path)
        }
      } catch (error) { view.watches.dispose(); this.views.delete(view.id); throw error }
    }
    view.sockets.add(socket)
    return view
  }

  clear(): void {
    for (const view of this.views.values()) view.watches.dispose()
    this.views.clear()
  }
}
