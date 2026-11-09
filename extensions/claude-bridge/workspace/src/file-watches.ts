import { statSync, watch, type FSWatcher } from 'node:fs'
import { basename, dirname, resolve } from 'node:path'
import { fileText, readContainerFile } from './file-operations.js'
import type { ClientView } from './protocol.js'

export const FILE_SENDS = Object.freeze([
  'file-viewer:watch', 'file-viewer:unwatch', 'file-tree:watch', 'file-tree:unwatch',
] as const)

interface Subscription {
  watcher: FSWatcher
  timer?: ReturnType<typeof setTimeout>
  active: boolean
  reading: boolean
  dirty: boolean
}

/** Host-owned logical-view subscriptions, independent of socket lifetime. */
export class FileWatches {
  private files = new Map<string, Subscription>()
  private trees = new Map<string, Subscription>()
  private disposed = false

  constructor(private view: ClientView, private onError: (error: Error) => void) {}

  paths(): { files: string[]; trees: string[] } {
    return { files: [...this.files.keys()], trees: [...this.trees.keys()] }
  }

  /** Fresh reads only, targeted to the attaching page; never replay mutations. */
  async snapshot(view: ClientView): Promise<void> {
    for (const path of this.files.keys()) {
      try {
        const { bytes, mtimeMs } = await readContainerFile(path)
        if (!this.disposed && this.files.has(path)) await view.postMessage({ kind: 'event',
          channel: 'file-viewer:external-change', args: [{ path, content: fileText(bytes), mtimeMs }] })
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ENOENT' && !this.disposed) {
          this.onError(error instanceof Error ? error : new Error(String(error)))
        }
      }
    }
    for (const path of this.trees.keys()) {
      if (!this.disposed) await view.postMessage({ kind: 'event', channel: 'file-tree:change',
        args: [{ root: path, changedDir: path, resync: true }] })
    }
  }

  receive(channel: string, path: string): void {
    if (this.disposed) throw new Error('File view disposed')
    const tree = channel.startsWith('file-tree:')
    const subscriptions = tree ? this.trees : this.files
    if (channel.endsWith(':unwatch')) { this.remove(subscriptions, path); return }
    if (subscriptions.has(path)) return
    if (subscriptions.size >= (tree ? 8 : 64)) throw new Error('File watch subscription limit')
    if (tree && !statSync(path).isDirectory()) throw new Error('File tree watch requires a directory')
    // Watching the parent keeps a file subscription alive across atomic saves.
    const watcher = watch(tree ? path : dirname(path), { recursive: tree, persistent: false }, (_event, name) => {
      if (this.disposed || !subscription.active) return
      const changed = name === null ? undefined : resolve(tree ? path : dirname(path), name)
      if (!tree && changed && basename(changed) !== basename(path)) return
      if (tree) {
        this.post(subscription, 'file-tree:change', { root: path, changedDir: changed ? dirname(changed) : path, changedPath: changed })
      } else {
        subscription.dirty = true
        if (subscription.timer) clearTimeout(subscription.timer)
        subscription.timer = setTimeout(() => {
          subscription.timer = undefined
          void this.refresh(subscription, path)
        }, 40)
        subscription.timer.unref()
      }
    })
    const subscription: Subscription = { watcher, active: true, reading: false, dirty: false }
    watcher.on('error', error => {
      if (!subscription.active || this.disposed) return
      this.remove(subscriptions, path)
      this.onError(error)
    })
    subscriptions.set(path, subscription)
  }

  dispose(): void {
    if (this.disposed) return
    this.disposed = true
    for (const [path] of this.files) this.remove(this.files, path)
    for (const [path] of this.trees) this.remove(this.trees, path)
  }

  private remove(subscriptions: Map<string, Subscription>, path: string): void {
    const subscription = subscriptions.get(path)
    if (!subscription) return
    subscription.active = false
    if (subscription.timer) clearTimeout(subscription.timer)
    subscription.watcher.close()
    subscriptions.delete(path)
  }

  private post(subscription: Subscription, channel: string, value: unknown): void {
    if (!subscription.active || this.disposed) return
    void this.view.postMessage({ kind: 'event', channel, args: [value] }).catch(error => {
      if (subscription.active && !this.disposed) this.onError(error instanceof Error ? error : new Error(String(error)))
    })
  }

  private async refresh(subscription: Subscription, path: string): Promise<void> {
    if (subscription.reading || !subscription.active || this.disposed) return
    subscription.reading = true
    try {
      while (subscription.dirty && subscription.active && !this.disposed) {
        subscription.dirty = false
        try {
          const { bytes, mtimeMs } = await readContainerFile(path)
          this.post(subscription, 'file-viewer:external-change', { path, content: fileText(bytes), mtimeMs })
        } catch (error) {
          // A rename/unlink has no text content. Never replace an unsaved editor
          // with fabricated empty bytes; the parent watch observes recreation.
          if ((error as NodeJS.ErrnoException).code !== 'ENOENT' && subscription.active && !this.disposed) {
            this.onError(error instanceof Error ? error : new Error(String(error)))
          }
        }
      }
    } finally { subscription.reading = false }
  }
}
