// Host-side TreeDataProvider registry (tree views). Mirrors the Webviews
// registry: extensions register a provider for a contributed view id; the
// renderer lazily pulls children over the WS and the host broadcasts a refresh
// when the provider fires onDidChangeTreeData.
//
// The element-handle problem: getChildren returns arbitrary `T` objects and
// getTreeItem(element) maps them — these can't cross the WS. Each view keeps a
// handle→element Map; the renderer only ever sees opaque handles and passes
// them back to fetch a node's children. Handles are never evicted within a
// session (bounded by nodes fetched); a refresh just re-fetches from the root.
import type { TreeNodeDto, TreeViewMetaDto } from "../../api/types.js"
import { DataTransfer } from "./classes-core.js"
import { Disposable, type EventEmitter } from "./shared.js"
import { asString, toDto, type TreeItemLike } from "./tree-item-dto.js"

type Broadcast = (channel: string, payload: unknown) => void
type ProviderResult<T> = T | undefined | null | Promise<T | undefined | null>

interface TreeDataProviderLike {
  getChildren: (element?: unknown) => ProviderResult<unknown[]>
  getTreeItem: (element: unknown) => TreeItemLike | Promise<TreeItemLike>
  onDidChangeTreeData?: (listener: (e: unknown) => void) => { dispose: () => void }
  /** Optional — enables deep `reveal` of a node whose ancestors aren't expanded. */
  getParent?: (element: unknown) => ProviderResult<unknown>
}

// Guard against a buggy getParent that never terminates (cycle / self-parent).
const MAX_DEPTH = 1000

const NOOP_TOKEN = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose: () => { /* never fires */ } }) }

/** TreeDragAndDropController — handleDrag/handleDrop both run host-side, so the
 *  DataTransfer never crosses the WS (the renderer only sends source/target handles). */
interface DragController {
  dropMimeTypes?: readonly string[]
  dragMimeTypes?: readonly string[]
  handleDrag?: (source: readonly unknown[], dataTransfer: unknown, token: unknown) => void | Promise<void>
  handleDrop?: (target: unknown, dataTransfer: unknown, token: unknown) => void | Promise<void>
}

/** TreeView event emitters (createTreeView only; registerTreeDataProvider omits them). */
interface ViewEmitters {
  selection: EventEmitter<{ selection: readonly unknown[] }>
  expand: EventEmitter<{ element: unknown }>
  collapse: EventEmitter<{ element: unknown }>
  visibility: EventEmitter<{ visible: boolean }>
  /** onDidChangeCheckboxState — `items` is [element, TreeItemCheckboxState][]. */
  checkbox: EventEmitter<{ items: [unknown, number][] }>
}

interface ViewEntry {
  provider: TreeDataProviderLike
  byHandle: Map<string, unknown>
  /** Reverse of byHandle (element identity → handle) so reveal's lookup and the
   *  getChildren handle-reuse are O(1) instead of O(n) per node. */
  byElement: Map<unknown, string>
  seq: number
  changeSub: { dispose: () => void } | null
  emitters: ViewEmitters | null
  meta: TreeViewMetaDto
  dnd: DragController | null
  /** The DataTransfer the current drag's handleDrag populated, read on drop. */
  activeDrag: DataTransfer | null
}

// Max handles retained per view. A provider that returns fresh element objects
// (no stable TreeItem.id) on every refresh — a git view refreshing every few
// seconds — misses byElement every time and mints a new handle per node per
// refresh, so byHandle/byElement grow without bound (every generation's element
// objects pinned). Evict the OLDEST handles (insertion order = stale
// generations first; the current refresh's are newest) once over the cap. A
// dropped stale handle just makes getChildren return [] → the renderer re-fetches
// from the root. Stable-id providers never approach the cap.
const TREE_HANDLE_CAP = 5000
function evictStaleHandles(entry: ViewEntry): void {
  const over = entry.byHandle.size - TREE_HANDLE_CAP
  if (over <= 0) return
  let n = 0
  for (const [h, el] of entry.byHandle) {
    if (n++ >= over) break
    entry.byHandle.delete(h)
    if (entry.byElement.get(el) === h) entry.byElement.delete(el)
  }
}

export class TreeViews {
  private readonly views = new Map<string, ViewEntry>()

  constructor(private readonly broadcast: Broadcast) {}

  /** registerTreeDataProvider (no emitters) or createTreeView (with emitters,
   *  and optionally a drag-and-drop controller). */
  register(viewId: string, provider: TreeDataProviderLike, emitters: ViewEmitters | null, dnd: DragController | null = null): Disposable {
    this.views.get(viewId)?.changeSub?.dispose()
    const entry: ViewEntry = { provider, byHandle: new Map(), byElement: new Map(), seq: 0, changeSub: null, emitters, meta: {}, dnd, activeDrag: null }
    entry.changeSub = provider.onDidChangeTreeData?.(() => {
      // Do NOT clear the handle maps here: a refresh re-fetches top-down, and a
      // deep level can query its handle before the root has re-materialised it
      // (parallel re-fetch). Handles stay stable (reused by element identity in
      // getChildren), so the renderer's expansion + deep references survive.
      this.broadcast("kamin:tree:changed", { viewId })
    }) ?? null
    this.views.set(viewId, entry)
    // Tell the renderer whether rows should be draggable (a view may be mounted
    // already when its extension is enabled at runtime).
    if (dnd) this.broadcast("kamin:tree:dnd", { viewId, enabled: true })
    return new Disposable(() => {
      entry.changeSub?.dispose()
      if (this.views.get(viewId) === entry) this.views.delete(viewId)
    })
  }

  /** Renderer pulls a node's children (root when handle is null/undefined —
   *  the WS serializes the renderer's `undefined` root arg to null). */
  async getChildren(viewId: string, handle?: string | null): Promise<TreeNodeDto[]> {
    const entry = this.views.get(viewId)
    if (!entry) return []
    // The renderer's root call passes no handle, which arrives as null over the
    // WS (JSON drops `undefined`) — treat null/undefined alike as "roots". A
    // provided-but-unknown handle is stale (survived a refresh) → return nothing.
    const isRoot = handle === undefined || handle === null
    if (!isRoot && !entry.byHandle.has(handle)) return []
    const element = isRoot ? undefined : entry.byHandle.get(handle)
    const children = (await entry.provider.getChildren(element)) ?? []
    const out: TreeNodeDto[] = []
    for (const child of children) {
      const item = await entry.provider.getTreeItem(child)
      // Reuse an existing handle for this element (TreeItem.id, else identity)
      // so handles stay stable across re-fetches — re-fetching the same node
      // twice (e.g. host reveal + renderer pull) must agree, and byHandle must
      // not grow unbounded.
      const h = asString(item.id) ?? entry.byElement.get(child) ?? `n${String(++entry.seq)}`
      entry.byHandle.set(h, child)
      entry.byElement.set(child, h)
      out.push(toDto(item, h))
    }
    evictStaleHandles(entry)
    return out
  }

  /** Renderer reports the user's selection (handles) → fire onDidChangeSelection. */
  reportSelection(viewId: string, handles: readonly string[]): void {
    const entry = this.views.get(viewId)
    if (!entry?.emitters) return
    entry.emitters.selection.fire({ selection: handles.map((h) => entry.byHandle.get(h)).filter((e) => e !== undefined) })
  }

  reportExpansion(viewId: string, handle: string, expanded: boolean): void {
    const entry = this.views.get(viewId)
    const element = entry?.byHandle.get(handle)
    if (!entry?.emitters || element === undefined) return
    ;(expanded ? entry.emitters.expand : entry.emitters.collapse).fire({ element })
  }

  reportVisibility(viewId: string, visible: boolean): void {
    this.views.get(viewId)?.emitters?.visibility.fire({ visible })
  }

  /** Renderer reports a checkbox toggle → fire onDidChangeCheckboxState with the
   *  [element, state] pair (VS Code's TreeCheckboxChangeEvent shape). */
  reportCheckbox(viewId: string, handle: string, state: number): void {
    const entry = this.views.get(viewId)
    const element = entry?.byHandle.get(handle)
    if (!entry?.emitters || element === undefined) return
    entry.emitters.checkbox.fire({ items: [[element, state]] })
  }

  /** Whether this view has a drag-and-drop controller (renderer pulls on mount). */
  hasDnd(viewId: string): boolean {
    return this.views.get(viewId)?.dnd != null
  }

  /** A drag started on `sourceHandles`: run handleDrag, which populates a fresh
   *  DataTransfer we stash for the matching drop (both run host-side). */
  async handleDrag(viewId: string, sourceHandles: readonly string[]): Promise<void> {
    const entry = this.views.get(viewId)
    if (!entry?.dnd) return
    const source = sourceHandles.map((h) => entry.byHandle.get(h)).filter((e) => e !== undefined)
    const dt = new DataTransfer()
    entry.activeDrag = dt
    await entry.dnd.handleDrag?.(source, dt, NOOP_TOKEN)
  }

  /** A drop on `targetHandle` (null = root): run handleDrop with the stashed
   *  DataTransfer, then clear it. The extension typically fires onDidChangeTreeData. */
  async handleDrop(viewId: string, targetHandle: string | null): Promise<void> {
    const entry = this.views.get(viewId)
    if (!entry?.dnd) return
    const target = targetHandle === null ? undefined : entry.byHandle.get(targetHandle)
    const dt = entry.activeDrag ?? new DataTransfer()
    entry.activeDrag = null
    await entry.dnd.handleDrop?.(target, dt, NOOP_TOKEN)
  }

  /** createTreeView display props — merge a patch (undefined fields clear) and
   *  push to the renderer. `undefined` in a patch field is meaningful (the
   *  extension cleared it), so we assign rather than spread-skip. */
  setMeta(viewId: string, patch: { [K in keyof TreeViewMetaDto]?: TreeViewMetaDto[K] | undefined }): void {
    const entry = this.views.get(viewId)
    if (!entry) return
    // An undefined patch field clears it — rebuild without the key so the
    // broadcast DTO stays clean (absent = cleared, never explicit undefined).
    const merged = { ...entry.meta, ...patch }
    const meta: TreeViewMetaDto = {}
    if (merged.message !== undefined) meta.message = merged.message
    if (merged.title !== undefined) meta.title = merged.title
    if (merged.description !== undefined) meta.description = merged.description
    if (merged.badge !== undefined) meta.badge = merged.badge
    entry.meta = meta
    this.broadcast("kamin:tree:meta", { viewId, meta })
  }

  /** Current display props (renderer pulls on mount — a setMeta broadcast may
   *  have fired before the view was mounted). */
  getMeta(viewId: string): TreeViewMetaDto {
    return this.views.get(viewId)?.meta ?? {}
  }

  /** `TreeView.reveal`: select/scroll a node, expanding its ancestors. If the
   *  node isn't materialized yet and the provider implements getParent, walk the
   *  ancestor chain and materialize top-down so even a deep, collapsed node can
   *  be revealed (`expandPath` tells the renderer which ancestors to open).
   *  Without getParent only roots are re-materialized (shallow reveal). */
  async reveal(viewId: string, element: unknown, options?: { select?: boolean; focus?: boolean; expand?: boolean | number }): Promise<void> {
    const entry = this.views.get(viewId)
    if (!entry) return
    let handle = entry.byElement.get(element)
    let expandPath: string[] = []
    if (handle === undefined) {
      expandPath = await this.materializePath(viewId, entry, element)
      handle = entry.byElement.get(element)
    }
    if (handle === undefined) return
    this.broadcast("kamin:tree:reveal", {
      viewId,
      handle,
      expandPath,
      // VS Code defaults: select true, focus false, expand false.
      select: options?.select !== false,
      focus: options?.focus === true,
      expand: options?.expand ?? false,
    })
  }

  /** Materialize the chain down to `element` and return the ancestor handles to
   *  expand. Uses getParent when present (deep reveal); otherwise just refetches
   *  roots (shallow). After this, `element` is in byElement if reachable. */
  private async materializePath(viewId: string, entry: ViewEntry, element: unknown): Promise<string[]> {
    if (!entry.provider.getParent) {
      await this.getChildren(viewId) // shallow: roots only
      return []
    }
    // Build [topAncestor … parent, element] via getParent, then fetch top-down.
    const chain: unknown[] = [element]
    let cur = await entry.provider.getParent(element)
    while (cur !== undefined && cur !== null && chain.length < MAX_DEPTH) {
      chain.unshift(cur)
      cur = await entry.provider.getParent(cur)
    }
    await this.getChildren(viewId) // roots
    const expandPath: string[] = []
    for (let i = 0; i < chain.length - 1; i++) {
      const aHandle = entry.byElement.get(chain[i])
      if (aHandle === undefined) break // chain broke (parent not under root)
      expandPath.push(aHandle)
      await this.getChildren(viewId, aHandle) // materialize its children
    }
    return expandPath
  }
}

export type { ViewEmitters, TreeDataProviderLike, DragController }
