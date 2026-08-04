import type { JSX } from 'preact'
import { useEffect, useRef } from 'preact/hooks'
import { EditorState, EditorSelection, Compartment } from '@codemirror/state'
import { EditorView, lineNumbers, highlightActiveLine, highlightActiveLineGutter, drawSelection, keymap, dropCursor, rectangularSelection, crosshairCursor } from '@codemirror/view'
import { defaultKeymap, history, historyKeymap, indentWithTab, redo, moveLineUp, moveLineDown } from '@codemirror/commands'
import { searchKeymap, highlightSelectionMatches, search } from '@codemirror/search'
import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap } from '@codemirror/autocomplete'
import { bracketMatching, foldGutter, foldKeymap, indentOnInput } from '@codemirror/language'
import { lintKeymap } from '@codemirror/lint'
import { useBridge } from '../../hooks/useBridge'
import { activeTabId } from '../../signals/tabs'
import {
  activeOpenFilePath,
  closeFileInTab,
  editorStates,
  getOpenFiles,
  makeStateKey,
  markFileDirty,
  markFileMtime,
  setActiveSelection,
} from '../../signals/file-viewer'
import { resolvedTheme } from '../../theme/apply-theme'
import { buildEditorTheme } from './file-editor-theme'
import { languageForPath } from './file-editor-lang'
import { FilePreview, isPreviewablePath } from './FilePreview'
import { HtmlInlinePreview, hasInlinePreview } from './HtmlInlinePreview'
import { previewModeByKey, getPreviewMode, setPreviewMode } from '../../signals/file-viewer'

/** Window-keyed lookup for the confirm modal helper that App.tsx
 *  registers globally. We call it on external-change conflict so the
 *  user gets the standard "do you want to reload from disk?" dialog. */
type ConfirmFn = (opts: { title: string; bodyHtml: string; confirmLabel?: string; isDanger?: boolean }) => Promise<boolean>

/** Single CodeMirror 6 editor — owns one EditorView and swaps in
 *  per-(tab, file) EditorState whenever the active file changes. The
 *  state cache lives in `editorStates` (module Map) so undo/redo,
 *  scroll, and selection all survive tab + file switching without any
 *  resets. */
export function FileEditor(): JSX.Element {
  const bridge = useBridge()
  const containerRef = useRef<HTMLDivElement | null>(null)
  const viewRef = useRef<EditorView | null>(null)
  const themeCompartmentRef = useRef<Compartment>(new Compartment())
  const langCompartmentRef = useRef<Compartment>(new Compartment())
  /** Active key (tabId+path) currently mounted in the EditorView. We
   *  swap states by writing to viewRef.current.setState(); this ref
   *  tracks what's there so the autosave/external-change handlers know
   *  which file they're operating on. */
  const activeKeyRef = useRef<string | null>(null)
  const activePathRef = useRef<string | null>(null)
  const activeTabIdRef = useRef<string | null>(null)
  const watchedPathsRef = useRef<Set<string>>(new Set())
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  /** Suppress the docChanged listener while we're swapping states or
   *  applying an external reload — both fire transactions that aren't
   *  user-driven and shouldn't trigger autosave or dirty-flagging. */
  const suppressChangeRef = useRef<boolean>(false)

  const tabId = activeTabId.value
  const filePath = activeOpenFilePath.value
  activeTabIdRef.current = tabId
  activePathRef.current = filePath

  // Autosave debounce — same idiom as Sidebar.persist().
  function flushSave(): void {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current)
      saveTimerRef.current = null
    }
    const view = viewRef.current
    const tab = activeTabIdRef.current
    const path = activePathRef.current
    if (!view || !tab || !path) return
    const content = view.state.doc.toString()
    bridge.fileViewerWrite(path, content)
      .then((res) => {
        markFileMtime(tab, path, res.mtimeMs)
        markFileDirty(tab, path, false)
      })
      .catch(() => { /* keep dirty flag — user can retry by editing again */ })
  }
  function scheduleSave(): void {
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    saveTimerRef.current = setTimeout(flushSave, 600)
  }
  // Ctrl+S handler — flushes the pending autosave immediately. Used
  // by the keymap below; `flushSave` reads refs at call time so it
  // doesn't capture stale tab/path closures.
  const flushSaveRef = useRef(flushSave)
  flushSaveRef.current = flushSave
  useEffect(() => {
    const w = window as unknown as { __filePanelFlushSave?: () => void; __filePanelCloseActive?: () => void }
    w.__filePanelFlushSave = () => flushSaveRef.current()
    w.__filePanelCloseActive = () => {
      const tab = activeTabIdRef.current
      const path = activePathRef.current
      if (tab && path) closeFileInTab(tab, path)
    }
    return () => {
      delete w.__filePanelFlushSave
      delete w.__filePanelCloseActive
    }
  }, [])

  // Initial mount — create a single EditorView with empty state.
  useEffect(() => {
    if (!containerRef.current || viewRef.current) return
    const themeC = themeCompartmentRef.current
    const langC = langCompartmentRef.current
    const startState = EditorState.create({
      doc: '',
      extensions: [
        themeC.of(buildEditorTheme()),
        langC.of([]),
        ...buildBaseExtensions((view, tabId, path) => {
          markFileDirty(tabId, path, true)
          scheduleSave()
          void view
        }),
      ],
    })
    const view = new EditorView({ state: startState, parent: containerRef.current })
    viewRef.current = view

    // Live theme tracking — swap the chrome compartment when the app's
    // resolvedTheme flips so colours follow the layout.
    //
    // Subscriber fires SYNCHRONOUSLY on `.value =` inside apply-theme,
    // BEFORE Chromium recalculates styles for the new `data-theme`
    // attribute. If we read CSS variables now via getComputedStyle, we
    // get the OLD theme's values back and the editor stays in the
    // previous palette until the file is reopened (the bug user reported).
    // Defer one frame so the cascade has applied first.
    const unsubTheme = resolvedTheme.subscribe(() => {
      requestAnimationFrame(() => {
        const v2 = viewRef.current
        if (!v2) return
        v2.dispatch({ effects: themeC.reconfigure(buildEditorTheme()) })
      })
    })

    // External-change listener — only one lives at the document level
    // for all editors; we filter by path against our currently-loaded
    // file. Conflict resolution prompts the user before clobbering
    // whatever they've typed.
    const offExternal = bridge.onFileViewerExternalChange(async ({ path, content, mtimeMs }) => {
      const tab = activeTabIdRef.current
      if (!tab) return
      // Find which open file in *any* tab this affects — but for MVP
      // we only react when the file is currently shown, since that's
      // the only spot where collision matters visually. Background
      // tabs sync on next open via the read in `loadFile`.
      if (path !== activePathRef.current) {
        markFileMtime(tab, path, mtimeMs)
        return
      }
      const view = viewRef.current
      if (!view) return
      const localContent = view.state.doc.toString()
      if (localContent === content) {
        markFileMtime(tab, path, mtimeMs)
        return
      }
      // Conflict resolution policy:
      //  - If the editor isn't dirty (user hasn't typed anything since
      //    the last save), an external change is non-conflicting — just
      //    silently reload, no modal. This is the common case (agent
      //    edited the file the user is *viewing*).
      //  - If the editor *is* dirty, two writers raced and we ask the
      //    user before clobbering their unsaved keystrokes.
      const fileEntry = getOpenFiles(tab).find(f => f.path === path)
      const isDirty = !!fileEntry?.dirty
      let accept = !isDirty
      if (isDirty) {
        const confirm = (window as unknown as { __showConfirmModal?: ConfirmFn }).__showConfirmModal
        accept = confirm
          ? await confirm({
              title: 'File changed on disk',
              bodyHtml: `<code>${escapeHtml(path)}</code> was modified outside the editor while you have unsaved changes. Reload from disk?<br><br><span style="color:var(--text-muted);font-size:11px">Your local edits will be lost. Cancel to keep yours — the next keystroke will overwrite the on-disk version.</span>`,
              confirmLabel: 'Reload',
              isDanger: true,
            })
          : false
      }
      if (!accept) return
      suppressChangeRef.current = true
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: content } })
      suppressChangeRef.current = false
      markFileMtime(tab, path, mtimeMs)
      markFileDirty(tab, path, false)
    })

    return () => {
      unsubTheme()
      offExternal()
      // Unwatch every path we ever asked main to watch.
      for (const p of watchedPathsRef.current) bridge.fileViewerUnwatch(p)
      watchedPathsRef.current.clear()
      view.destroy()
      viewRef.current = null
    }
  }, [])

  // File / tab change → save current state to cache + load (or fetch)
  // the new file's state and mount it on the EditorView.
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    if (!tabId || !filePath) {
      // Nothing to show — clear editor + drop the selection signal so
      // the input-bar attach status bar doesn't keep showing a stale
      // file from a previous tab.
      suppressChangeRef.current = true
      view.setState(EditorState.create({ doc: '', extensions: [
        themeCompartmentRef.current.of(buildEditorTheme()),
        langCompartmentRef.current.of([]),
        ...buildBaseExtensions(() => {}),
      ] }))
      suppressChangeRef.current = false
      activeKeyRef.current = null
      if (tabId) setActiveSelection(tabId, null)
      return
    }
    const newKey = makeStateKey(tabId, filePath)
    if (newKey === activeKeyRef.current) return

    // Binary preview file — skip the text-load path entirely; FilePreview
    // owns the rendering. Still publish a stub selection so the input
    // status bar updates to the new path (caret only, no text).
    if (isPreviewablePath(filePath)) {
      activeKeyRef.current = newKey
      setActiveSelection(tabId, { path: filePath, startLine: 1, endLine: 1, text: '' })
      return
    }

    // Refresh the selection signal immediately to whatever the new
    // file's main range is (read from cached state if we have one,
    // else fall back to caret at line 1). CodeMirror's `selectionSet`
    // event won't fire on `setState`, so without this the input bar
    // would keep showing the previous file's selection until the user
    // clicks inside the editor.
    publishSelectionFor(tabId, filePath, editorStates.get(newKey) ?? null)

    // Save the outgoing state so reopening the same file restores undo
    // history, scroll, and selection. (CM6's EditorState is immutable,
    // we cache the latest reference; the view will keep dispatching
    // transactions against it until the next swap.)
    if (activeKeyRef.current) {
      editorStates.set(activeKeyRef.current, view.state)
    }

    const cached = editorStates.get(newKey)
    if (cached) {
      suppressChangeRef.current = true
      view.setState(cached)
      suppressChangeRef.current = false
      activeKeyRef.current = newKey
      view.focus()
      return
    }

    // Fresh load — read from disk, build state, mount.
    activeKeyRef.current = newKey
    const localTabId = tabId
    const localPath = filePath
    suppressChangeRef.current = true
    view.setState(EditorState.create({
      doc: '… loading …',
      extensions: [
        themeCompartmentRef.current.of(buildEditorTheme()),
        langCompartmentRef.current.of([]),
        ...buildBaseExtensions(() => {}),
      ],
    }))
    suppressChangeRef.current = false

    Promise.all([
      bridge.fileViewerRead(localPath),
      languageForPath(localPath),
    ]).then(([read, lang]) => {
      // The user may have switched files again before this resolved —
      // bail if we're stale.
      if (activeKeyRef.current !== newKey) return
      const v2 = viewRef.current
      if (!v2) return
      const newState = EditorState.create({
        doc: read.content,
        extensions: [
          themeCompartmentRef.current.of(buildEditorTheme()),
          langCompartmentRef.current.of(lang ?? []),
          ...buildBaseExtensions((_v, t, p) => {
            markFileDirty(t, p, true)
            scheduleSave()
          }, localTabId, localPath, suppressChangeRef),
        ],
      })
      suppressChangeRef.current = true
      v2.setState(newState)
      suppressChangeRef.current = false
      editorStates.set(newKey, newState)
      markFileMtime(localTabId, localPath, read.mtimeMs)
      markFileDirty(localTabId, localPath, false)
      // Publish the freshly-loaded file's L1 caret to the selection
      // signal so the input-bar status bar swaps to the new file
      // without waiting for the user to click inside the editor.
      publishSelectionFor(localTabId, localPath, newState)
      v2.focus()

      // Subscribe main to fs.watch this path if we haven't already.
      if (!watchedPathsRef.current.has(localPath)) {
        bridge.fileViewerWatch(localPath)
        watchedPathsRef.current.add(localPath)
      }
    }).catch((err: unknown) => {
      const v2 = viewRef.current
      if (!v2) return
      v2.dispatch({ changes: { from: 0, to: v2.state.doc.length, insert: `Failed to read ${localPath}\n\n${String(err)}` } })
    })
  }, [tabId, filePath])

  // Binary file (image / PDF) — show native FilePreview, no toggle.
  const binaryPreview = !!filePath && isPreviewablePath(filePath)
  // Renderable code file (HTML so far, MD/SVG planned) — user picks
  // between code editor and rendered preview via the toggle.
  const canTogglePreview = !!filePath && hasInlinePreview(filePath)
  // Subscribe to the signal so the toggle re-renders on state change.
  void previewModeByKey.value
  const mode = filePath && tabId ? getPreviewMode(tabId, filePath) : 'code'
  const showRendered = canTogglePreview && mode === 'preview'

  return (
    <div style="flex:1;min-height:0;display:flex;flex-direction:column;position:relative;background:var(--bg-mantle)">
      {canTogglePreview && (
        <CodePreviewToggle
          mode={mode}
          onPick={(m) => { if (filePath && tabId) setPreviewMode(tabId, filePath, m) }}
        />
      )}
      <div
        ref={containerRef}
        onClick={(e: any) => {
          if ((e.target as HTMLElement).closest('.cm-panel')) return
          viewRef.current?.focus()
        }}
        style={`flex:1;min-height:0;overflow:hidden;${(filePath && !binaryPreview && !showRendered) ? '' : 'display:none'}`}
      />
      {filePath && binaryPreview && <FilePreview filePath={filePath} />}
      {filePath && showRendered && <HtmlInlinePreview filePath={filePath} />}
      {!filePath && <FileEditorPlaceholder />}
    </div>
  )
}

/** Tiny segmented toggle in the editor's top-right corner — Code |
 *  Preview. Only rendered for file types where rendering makes sense
 *  (HTML now, MD next). State lives in `previewModeByKey` keyed by
 *  (tabId, path) so each open file remembers its mode independently. */
function CodePreviewToggle({ mode, onPick }: { mode: 'code' | 'preview'; onPick: (m: 'code' | 'preview') => void }): JSX.Element {
  const seg = (active: boolean): string => `
    flex:0 0 auto;display:inline-flex;align-items:center;gap:5px;
    padding:3px 10px;border:none;
    background:${active ? 'color-mix(in srgb, var(--accent-primary) 14%, transparent)' : 'transparent'};
    color:${active ? 'var(--accent-primary)' : 'var(--text-muted)'};
    font-family:inherit;font-size:11px;font-weight:500;cursor:pointer;
    border-radius:0;
  `
  return (
    <div style="position:absolute;top:8px;right:12px;z-index:1;display:inline-flex;align-items:center;border:1px solid var(--bg-surface);border-radius:var(--radius-sm);overflow:hidden;background:var(--bg-base)">
      <button type="button" onClick={() => onPick('code')} style={seg(mode === 'code')} data-tooltip="Show source code">
        <i class="fas fa-code" style="font-size:10px" />
        <span>Code</span>
      </button>
      <span style="width:1px;height:18px;background:var(--bg-surface)" />
      <button type="button" onClick={() => onPick('preview')} style={seg(mode === 'preview')} data-tooltip="Render the file">
        <i class="fas fa-eye" style="font-size:10px" />
        <span>Preview</span>
      </button>
    </div>
  )
}

/** Empty-state when no file is open in the active chat tab — invites
 *  the user to click a Read/Write entry from the right panel. Shown
 *  on top of the (hidden) CodeMirror container so the layout doesn't
 *  shift when the first file opens. */
function FileEditorPlaceholder(): JSX.Element {
  return (
    <div style="position:absolute;inset:0;display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;padding:32px;color:var(--text-muted);text-align:center;pointer-events:none">
      <div
        style="flex:0 0 auto;display:flex;align-items:center;justify-content:center;width:56px;height:56px;min-width:56px;border-radius:50%;background:color-mix(in srgb, var(--accent-primary) 10%, transparent);color:var(--accent-primary)"
      >
        <i class="fas fa-file-code" style="font-size:22px" />
      </div>
      <div style="font-size:13px;font-weight:600;color:var(--text-secondary)">No file open</div>
      <div style="font-size:11px;line-height:1.55;max-width:300px;color:var(--text-muted)">
        Click any entry under <span style="color:var(--accent-primary);font-weight:600">Read</span> or
        {' '}<span style="color:var(--accent-primary);font-weight:600">Write</span>
        {' '}in the right panel to open it here.<br />
        Each chat keeps its own set of open files — switching chats restores them, and undo/redo per file survives.
      </div>
      <div style="display:grid;grid-template-columns:auto auto;gap:6px 18px;font-size:10px;color:var(--text-disabled);text-align:left">
        <Hint k="Ctrl+S">save now</Hint>
        <Hint k="Ctrl+Z / Ctrl+Shift+Z">undo / redo</Hint>
        <Hint k="Ctrl+F">find</Hint>
        <Hint k="Ctrl+H">find &amp; replace</Hint>
        <Hint k="Ctrl+G">go to next match</Hint>
        <Hint k="Ctrl+D">select next occurrence</Hint>
        <Hint k="Ctrl+/">toggle comment</Hint>
        <Hint k="Ctrl+]">indent / outdent</Hint>
        <Hint k="Alt/Ctrl+Click">add cursor</Hint>
        <Hint k="Ctrl+Alt+↑/↓">cursor above/below</Hint>
        <Hint k="Ctrl+Shift+↑/↓">move line</Hint>
        <Hint k="Ctrl+Space">autocomplete</Hint>
        <Hint k="Ctrl+W">close file</Hint>
      </div>
    </div>
  )
}

/** Base extensions every state needs. The change-listener factory is
 *  parameterised because the state we build for the loading placeholder
 *  shouldn't autosave or dirty-flag, but the real one should. */
function buildBaseExtensions(
  onUserChange: (view: EditorView, tabId: string, filePath: string) => void,
  tabId?: string,
  filePath?: string,
  suppressChangeRef?: { current: boolean },
) {
  return [
    // Multi-cursor support — without `allowMultipleSelections` CM6
    // collapses every selection to a single range and Ctrl/Alt-click
    // does nothing visible. `clickAddsSelectionRange` accepts both
    // Ctrl and Alt as the modifier key (Alt is CM's default, Ctrl
    // matches VSCode/JetBrains habits).
    EditorState.allowMultipleSelections.of(true),
    EditorView.clickAddsSelectionRange.of((e) => e.altKey || e.ctrlKey || e.metaKey),
    rectangularSelection(),
    crosshairCursor(),
    dropCursor(),
    lineNumbers(),
    foldGutter(),
    drawSelection(),
    history(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    autocompletion(),
    highlightActiveLine(),
    highlightActiveLineGutter(),
    highlightSelectionMatches(),
    search({ top: true }),
    EditorView.lineWrapping,
    EditorView.updateListener.of((u) => {
      // Doc changes drive autosave + dirty flag (see callsite).
      if (u.docChanged && !suppressChangeRef?.current && tabId && filePath) {
        onUserChange(u.view, tabId, filePath)
      }
      // Selection changes drive the input-bar attach feature so the
      // chat can carry the user's currently-highlighted lines as
      // context. We mirror to the signal regardless of attachActiveFile;
      // the input bar reads the signal and decides whether to send.
      if (u.selectionSet && tabId && filePath) {
        const sel = u.state.selection.main
        const startLine = u.state.doc.lineAt(sel.from).number
        const endLine = u.state.doc.lineAt(sel.to).number
        const text = sel.empty ? '' : u.state.sliceDoc(sel.from, sel.to)
        setActiveSelection(tabId, { path: filePath, startLine, endLine, text })
      }
    }),
    keymap.of([
      // Ctrl/Cmd+S → flush pending autosave right now. Returning true
      // so the browser's own save dialog never fires.
      {
        key: 'Mod-s',
        run: () => {
          const fn = (window as unknown as { __filePanelFlushSave?: () => void }).__filePanelFlushSave
          if (fn) fn()
          return true
        },
      },
      // Ctrl/Cmd+W → close current file (matches the FileViewerTabs ×).
      {
        key: 'Mod-w',
        run: () => {
          const w = window as unknown as { __filePanelCloseActive?: () => void }
          w.__filePanelCloseActive?.()
          return true
        },
      },
      // Ctrl/Cmd+Shift+Z → redo (in addition to default Ctrl+Y, matches
      // VSCode / most other editors). Bound BEFORE historyKeymap so this
      // wins over any reserved-but-unused mapping.
      { key: 'Mod-Shift-z', run: redo, preventDefault: true },
      // Ctrl+Alt+Up / Ctrl+Alt+Down → add cursor above / below at the
      // same visual column. Stops at document edge. Matches VSCode.
      { key: 'Ctrl-Alt-ArrowUp', run: addCursorVertical(-1), preventDefault: true },
      { key: 'Ctrl-Alt-ArrowDown', run: addCursorVertical(1), preventDefault: true },
      // Ctrl+Shift+Up / Ctrl+Shift+Down → move current line(s) up/down.
      // Override of CM6's default Alt+Up/Down — bound BEFORE defaultKeymap
      // so this wins over the inherited alt mapping.
      { key: 'Mod-Shift-ArrowUp', run: moveLineUp, preventDefault: true },
      { key: 'Mod-Shift-ArrowDown', run: moveLineDown, preventDefault: true },
      indentWithTab,
      ...closeBracketsKeymap,
      // defaultKeymap minus the Alt+ArrowUp/Down move-line bindings —
      // we've reassigned move-line to Ctrl+Shift+Up/Down (bound above).
      // Filter by key string to drop the conflicting entries cleanly.
      ...defaultKeymap.filter(b => b.key !== 'Alt-ArrowUp' && b.key !== 'Alt-ArrowDown'),
      ...searchKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      ...lintKeymap,
    ]),
  ]
}

function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, ch => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]!))
}

/** Push the selection of `state` (or a default L1 caret if no state)
 *  into `activeSelectionByTab` so the input-bar status reflects the
 *  newly-active file immediately, before any CodeMirror selection
 *  event has had a chance to fire. */
function publishSelectionFor(tabId: string, filePath: string, state: EditorState | null): void {
  if (!state) {
    setActiveSelection(tabId, { path: filePath, startLine: 1, endLine: 1, text: '' })
    return
  }
  const sel = state.selection.main
  const startLine = state.doc.lineAt(sel.from).number
  const endLine = state.doc.lineAt(sel.to).number
  const text = sel.empty ? '' : state.sliceDoc(sel.from, sel.to)
  setActiveSelection(tabId, { path: filePath, startLine, endLine, text })
}

/** Add a cursor on the line above (-1) or below (+1) at the same
 *  column for every existing selection range. Stops at document edges.
 *  CM6 doesn't ship this command in `@codemirror/commands` so we roll
 *  it ourselves — same behaviour as VSCode's Ctrl+Alt+Up/Down. */
function addCursorVertical(dir: -1 | 1): (view: EditorView) => boolean {
  return (view) => {
    const { state } = view
    const newRanges = state.selection.ranges.slice()
    for (const range of state.selection.ranges) {
      const line = state.doc.lineAt(range.head)
      const col = range.head - line.from
      const targetLineNum = line.number + dir
      if (targetLineNum < 1 || targetLineNum > state.doc.lines) continue
      const target = state.doc.line(targetLineNum)
      const newPos = Math.min(target.from + col, target.to)
      // Skip if there's already a cursor exactly there (avoid duplicates).
      if (newRanges.some(r => r.empty && r.head === newPos)) continue
      newRanges.push(EditorSelection.cursor(newPos))
    }
    if (newRanges.length === state.selection.ranges.length) return false
    view.dispatch({ selection: EditorSelection.create(newRanges, newRanges.length - 1) })
    return true
  }
}

function Hint({ k, children }: { k: string; children: any }): JSX.Element {
  return (
    <span style="display:flex;align-items:center;gap:6px">
      <kbd style="font-family:inherit;padding:1px 6px;border:1px solid var(--bg-surface);border-radius:var(--radius-xs);background:var(--bg-base);color:var(--text-muted);font-size:10px;white-space:nowrap">{k}</kbd>
      <span style="color:var(--text-muted)">{children}</span>
    </span>
  )
}
