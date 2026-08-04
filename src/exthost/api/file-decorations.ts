// Host-side FileDecorationProvider registry. Extensions register providers that
// badge/colour/tooltip files (git status, problems, …); the renderer's file
// tree pulls a decoration per visible row over the WS, and the host broadcasts
// a refresh when a provider fires onDidChangeFileDecorations.
//
// Multiple providers may decorate the same uri; VS Code shows one badge — we
// return the first provider that yields a decoration (registration order).
import type { FileDecorationDto } from "../../api/types.js"
import { Disposable, Uri } from "./shared.js"

type Broadcast = (channel: string, payload: unknown) => void
type ProviderResult<T> = T | undefined | null | Promise<T | undefined | null>

interface FileDecorationLike {
  badge?: unknown
  tooltip?: unknown
  color?: unknown
  propagate?: unknown
}

interface FileDecorationProviderLike {
  provideFileDecoration: (uri: unknown, token: unknown) => ProviderResult<FileDecorationLike>
  onDidChangeFileDecorations?: (listener: (e: unknown) => void) => { dispose: () => void }
}

interface Registration {
  provider: FileDecorationProviderLike
  changeSub: { dispose: () => void } | null
}

const NOOP_TOKEN = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose: () => { /* never fires */ } }) }

const asString = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined)

export class FileDecorations {
  private readonly providers = new Set<Registration>()

  constructor(private readonly broadcast: Broadcast) {}

  register(provider: FileDecorationProviderLike): Disposable {
    const reg: Registration = { provider, changeSub: null }
    reg.changeSub = provider.onDidChangeFileDecorations?.((e) => {
      // e is undefined (all) | Uri | Uri[] — normalize to fs-path list or null.
      this.broadcast("kamin:fileDecoration:changed", { uris: toFsPaths(e) })
    }) ?? null
    this.providers.add(reg)
    // A newly-registered provider (e.g. its extension was just enabled) must be
    // queried for already-rendered rows — refresh so its decorations appear.
    this.broadcast("kamin:fileDecoration:changed", { uris: null })
    return new Disposable(() => {
      reg.changeSub?.dispose()
      this.providers.delete(reg)
      // The provider is gone (e.g. its extension was disabled) — tell the tree
      // to re-query so its decorations clear instead of lingering stale.
      this.broadcast("kamin:fileDecoration:changed", { uris: null })
    })
  }

  /** First decoration any provider yields for the fs path, or null. */
  async provide(fsPath: string): Promise<FileDecorationDto | null> {
    const uri = Uri.file(fsPath)
    for (const { provider } of this.providers) {
      const deco = await provider.provideFileDecoration(uri, NOOP_TOKEN)
      if (deco) return toDto(deco)
    }
    return null
  }
}

/** onDidChangeFileDecorations payload → fs-path list, or null = refresh all. */
function toFsPaths(e: unknown): string[] | null {
  if (e === undefined || e === null) return null
  const list = Array.isArray(e) ? e : [e]
  const out: string[] = []
  for (const u of list) {
    const p = (u as { fsPath?: unknown }).fsPath
    if (typeof p === "string") out.push(p)
  }
  return out
}

function toDto(deco: FileDecorationLike): FileDecorationDto {
  const dto: FileDecorationDto = {}
  // VS Code caps the badge at ~2 chars; keep it short so a row stays tidy.
  const badge = asString(deco.badge)
  const tooltip = asString(deco.tooltip)
  // color is a ThemeColor instance → take its id for the renderer to map.
  const color = asString((deco.color as { id?: unknown } | undefined)?.id)
  if (badge) dto.badge = badge.slice(0, 2)
  if (tooltip) dto.tooltip = tooltip
  if (color) dto.color = color
  if (deco.propagate === true) dto.propagate = true
  return dto
}

export type { FileDecorationProviderLike }
