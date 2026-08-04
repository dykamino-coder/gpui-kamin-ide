// `vscode.workspace` document surface (B5): textDocuments, openTextDocument,
// and the onDid{Open,Close,Change,Save}TextDocument events. Split from
// ns-workspace.ts to hold the 250-LOC ceiling. Backed by the host document
// mirror (DocumentHost) which the renderer's Monaco doc-sync feeds.
import { URI } from "vscode-uri"
import type { HostDocument } from "../host-services.js"
import { Range } from "./classes-core.js"
import type { NsHooks } from "./ns-builders.js"
import { EventEmitter } from "./shared.js"
import { TextDocument } from "./text-document.js"

/** vscode.TextDocumentContentChangeEvent (B5b) — real incremental changes. */
interface ContentChange {
  range: Range
  rangeOffset: number
  rangeLength: number
  text: string
}
interface TextDocumentChangeEvent {
  document: TextDocument
  contentChanges: readonly ContentChange[]
  reason: undefined
}

export function buildDocuments(h: NsHooks) {
  const docHost = h.workspaceHost.documents
  const fsHost = h.workspaceHost.fs
  let untitledSeq = 0
  // One TextDocument identity per uri — extensions compare documents by ===.
  const cache = new Map<string, TextDocument>()
  // Persist a document's CURRENT mirror content to disk (backs TextDocument.save
  // + workspace.saveAll). The mirror is fed by the renderer's Monaco doc-sync, so
  // this writes exactly what the extension sees via getText(). Fires onDidSave
  // like a real save (the renderer's own Ctrl+S path fires it via docHost). Note:
  // this does NOT clear the editor's dirty dot (a renderer-routed save would) —
  // the bytes on disk are correct, which is what matters for the no-op fix.
  const saveDoc = async (uri: string): Promise<boolean> => {
    const cur = docHost.get(uri)
    if (!cur) return false
    try {
      await fsHost.writeFile(URI.file(uri).fsPath, new TextEncoder().encode(cur.content))
      onSave.fire(docFor(uri))
      return true
    } catch { return false }
  }
  const docFor = (uri: string): TextDocument => {
    let d = cache.get(uri)
    if (!d) { d = new TextDocument(URI.file(uri), () => docHost.get(uri), () => saveDoc(uri)); cache.set(uri, d) }
    return d
  }

  const onOpen = new EventEmitter<TextDocument>()
  const onClose = new EventEmitter<TextDocument>()
  const onSave = new EventEmitter<TextDocument>()
  const onChange = new EventEmitter<TextDocumentChangeEvent>()
  docHost.onDidOpen((d) => { onOpen.fire(docFor(d.uri)) })
  docHost.onDidSave((d) => { onSave.fire(docFor(d.uri)) })
  docHost.onDidChange((d, changes) => {
    onChange.fire({
      document: docFor(d.uri),
      contentChanges: changes.map((c) => ({
        range: new Range(c.range.startLine, c.range.startChar, c.range.endLine, c.range.endChar),
        rangeOffset: c.rangeOffset,
        rangeLength: c.rangeLength,
        text: c.text,
      })),
      reason: undefined,
    })
  })
  docHost.onDidClose((d) => { const doc = docFor(d.uri); cache.delete(d.uri); onClose.fire(doc) })

  // `_options` (e.g. { encoding }) is accepted for d.ts arity; encoding
  // negotiation isn't wired yet (always UTF-8).
  const openTextDocument = async (arg?: unknown, _options?: { encoding?: string }): Promise<TextDocument> => {
    const uri = arg instanceof URI ? arg : typeof arg === "string" ? URI.file(arg) : undefined
    if (uri) {
      // `untitled:` URIs are blank scratch buffers — never hit disk.
      if (uri.scheme === "untitled") {
        const snap: HostDocument = { uri: uri.toString(), languageId: "plaintext", version: 1, content: "" }
        return new TextDocument(uri, () => snap)
      }
      const path = uri.fsPath
      if (docHost.get(path)) return docFor(path)
      // Not open in an editor — load from disk and register it in the mirror
      // so it appears in textDocuments and gets change/close events.
      try {
        const content = new TextDecoder().decode(await fsHost.readFile(path))
        docHost.open({ uri: path, languageId: "plaintext", version: 1, content })
        return docFor(path)
      } catch {
        return new TextDocument(uri, () => undefined)
      }
    }
    const opts = (arg ?? {}) as { content?: string; language?: string }
    // Unique untitled name per call so two openTextDocument({content}) calls
    // don't collide on one identity (which would drop the second's content).
    const name = `Untitled-${String(++untitledSeq)}`
    const docUri = URI.from({ scheme: "untitled", path: name })
    const snap: HostDocument = {
      uri: docUri.toString(), languageId: opts.language ?? "plaintext", version: 1, content: opts.content ?? "",
    }
    return new TextDocument(docUri, () => snap)
  }

  return {
    /** Shared TextDocument-by-uri lookup so `window.activeTextEditor.document`
     *  has the same identity as `workspace.textDocuments` (B5b-2a). */
    docFor,
    list: (): TextDocument[] => docHost.list().map((d) => docFor(d.uri)),
    openTextDocument,
    onDidOpen: onOpen.event,
    onDidClose: onClose.event,
    onDidChange: onChange.event,
    onDidSave: onSave.event,
  }
}

export type DocumentsApi = ReturnType<typeof buildDocuments>
