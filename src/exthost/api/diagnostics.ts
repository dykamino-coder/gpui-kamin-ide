// Diagnostic registry (B6c). A single host-wide instance owns every
// extension's `languages.createDiagnosticCollection`. Each collection is a
// thin façade over a per-owner store; mutations broadcast `kamin:diag:set`
// to the renderer, which maps them onto Monaco model markers. `getDiagnostics`
// aggregates across all collections by uri, mirroring VS Code's global view.
import type { DiagnosticDto } from "../../api/language-feature-dtos.js"
import { rangeDto } from "./range-dto.js"
import { EventEmitter } from "./shared.js"

// DiagnosticDto (the WS shape) lives in the shared single-source contract;
// re-exported so exthost/index.ts keeps importing it from here.
export type { DiagnosticDto } from "../../api/language-feature-dtos.js"

/** Minimal vscode.Uri surface we rely on (fsPath as the mirror key). */
interface UriLike { fsPath?: string; toString: () => string }

type Diag = unknown // vscode.Diagnostic — opaque to the registry
type SetArg = UriLike | (readonly [UriLike, readonly Diag[] | undefined])[]

export interface DiagnosticCollection extends Iterable<readonly [UriLike, readonly Diag[]]> {
  readonly name: string
  set: (uriOrEntries: SetArg, diagnostics?: readonly Diag[]) => void
  delete: (uri: UriLike) => void
  clear: () => void
  get: (uri: UriLike) => readonly Diag[] | undefined
  has: (uri: UriLike) => boolean
  forEach: (cb: (uri: UriLike, diagnostics: readonly Diag[], collection: DiagnosticCollection) => void, thisArg?: unknown) => void
  dispose: () => void
}

interface StoreEntry { uri: UriLike; diagnostics: readonly Diag[] }

function keyOf(uri: UriLike): string {
  return typeof uri.fsPath === "string" ? uri.fsPath : uri.toString()
}

function toDiagnosticDtos(diags: readonly Diag[]): DiagnosticDto[] {
  const out: DiagnosticDto[] = []
  for (const raw of diags) {
    if (!raw || typeof raw !== "object") continue
    const d = raw as { range?: unknown; message?: unknown; severity?: unknown; source?: unknown; code?: unknown }
    const range = rangeDto(d.range)
    if (!range) continue
    const dto: DiagnosticDto = {
      range,
      message: typeof d.message === "string" ? d.message : "",
      severity: typeof d.severity === "number" ? d.severity : 0,
    }
    if (typeof d.source === "string") dto.source = d.source
    const code = d.code
    if (typeof code === "string" || typeof code === "number") dto.code = code
    else if (code && typeof code === "object" && "value" in code) {
      const v = code.value
      if (typeof v === "string" || typeof v === "number") dto.code = v
    }
    out.push(dto)
  }
  return out
}

export class Diagnostics {
  private seq = 0
  private readonly byOwner = new Map<string, Map<string, StoreEntry>>()
  private readonly changeEmitter = new EventEmitter<{ uris: UriLike[] }>()
  readonly onDidChangeDiagnostics = this.changeEmitter.event

  constructor(private readonly broadcast: (channel: string, payload: unknown) => void) {}

  createCollection(name?: string): DiagnosticCollection {
    // Unique owner per collection so two collections never clobber each
    // other's Monaco markers (Monaco keys markers by owner string).
    const owner = `${name ?? "kamin-diag"}#${String(++this.seq)}`
    const store = new Map<string, StoreEntry>()
    // Capture the registry's map + emitter so the collection's closures never
    // depend on `this` (the literal's methods would otherwise rebind it).
    const { byOwner, changeEmitter } = this
    byOwner.set(owner, store)
    const fire = (uris: UriLike[]): void => { if (uris.length > 0) changeEmitter.fire({ uris }) }
    const emit = (key: string, diags: readonly Diag[]): void =>
      { this.broadcast("kamin:diag:set", { owner, uri: key, diagnostics: toDiagnosticDtos(diags) }) }

    const setOne = (uri: UriLike, diagnostics: readonly Diag[] | undefined): void => {
      const key = keyOf(uri)
      if (diagnostics && diagnostics.length > 0) store.set(key, { uri, diagnostics })
      else store.delete(key)
      emit(key, diagnostics ?? [])
    }

    const collection: DiagnosticCollection = {
      name: name ?? owner,
      set(uriOrEntries, diagnostics) {
        if (Array.isArray(uriOrEntries)) {
          // d.ts §7194: tuples of the same uri MERGE within a batch; an
          // `undefined` entry clears prior (but not subsequent) ones for it.
          const merged = new Map<string, { uri: UriLike; diags: Diag[] }>()
          for (const [u, d] of uriOrEntries) {
            const key = keyOf(u)
            if (!d) { merged.set(key, { uri: u, diags: [] }); continue }
            const cur = merged.get(key) ?? { uri: u, diags: [] }
            cur.diags.push(...d)
            merged.set(key, cur)
          }
          for (const { uri, diags } of merged.values()) setOne(uri, diags)
          fire([...merged.values()].map((e) => e.uri))
        } else { setOne(uriOrEntries, diagnostics ?? undefined); fire([uriOrEntries]) }
      },
      delete(uri) { setOne(uri, undefined); fire([uri]) },
      clear() {
        const uris = [...store.values()].map((e) => e.uri)
        for (const key of [...store.keys()]) { store.delete(key); emit(key, []) }
        fire(uris)
      },
      get: (uri) => store.get(keyOf(uri))?.diagnostics,
      has: (uri) => store.has(keyOf(uri)),
      forEach(cb, thisArg) { for (const e of store.values()) cb.call(thisArg, e.uri, e.diagnostics, collection) },
      [Symbol.iterator]: () => {
        const entries = [...store.values()].map((e) => [e.uri, e.diagnostics] as const)
        return entries[Symbol.iterator]()
      },
      dispose: () => {
        const uris = [...store.values()].map((e) => e.uri)
        for (const key of [...store.keys()]) emit(key, [])
        store.clear()
        byOwner.delete(owner)
        fire(uris)
      },
    }
    return collection
  }

  /** Renderer snapshot: every (owner, uri) pair's diagnostics as plain DTOs.
   *  The Problems panel pulls this on (re)connect because `kamin:diag:set` only
   *  broadcasts deltas — a panel opened late would otherwise start empty. Same
   *  shape as the broadcast payload so the renderer replays each as a `set`. */
  snapshotDtos(): { owner: string; uri: string; diagnostics: DiagnosticDto[] }[] {
    const out: { owner: string; uri: string; diagnostics: DiagnosticDto[] }[] = []
    for (const [owner, store] of this.byOwner) {
      for (const e of store.values()) {
        out.push({ owner, uri: keyOf(e.uri), diagnostics: toDiagnosticDtos(e.diagnostics) })
      }
    }
    return out
  }

  /** vscode.languages.getDiagnostics — per-uri or the full aggregated list. */
  getDiagnostics(uri: UriLike): readonly Diag[]
  getDiagnostics(): (readonly [UriLike, readonly Diag[]])[]
  getDiagnostics(uri?: UriLike): readonly Diag[] | (readonly [UriLike, readonly Diag[]])[] {
    if (uri) {
      const key = keyOf(uri)
      const out: Diag[] = []
      for (const store of this.byOwner.values()) {
        const e = store.get(key)
        if (e) out.push(...e.diagnostics)
      }
      return out
    }
    const merged = new Map<string, { uri: UriLike; diags: Diag[] }>()
    for (const store of this.byOwner.values()) {
      for (const e of store.values()) {
        const k = keyOf(e.uri)
        const cur = merged.get(k) ?? { uri: e.uri, diags: [] }
        cur.diags.push(...e.diagnostics)
        merged.set(k, cur)
      }
    }
    return [...merged.values()].map((v) => [v.uri, v.diags] as const)
  }
}
