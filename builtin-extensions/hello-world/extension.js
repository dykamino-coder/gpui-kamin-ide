// Hello-world extension. Plain JS so we don't need a TS compile step in
// the skeleton. Real extensions are typically TS compiled to JS before
// being packaged as VSIX, so this matches the consumed shape.
const vscode = require("vscode")
// Phase 3: the additive `kaminide` virtual module. Feature-detected so the
// extension still loads on a host that doesn't provide it.
let kaminide
try { kaminide = require("kaminide") } catch { kaminide = undefined }

function activate(context) {
  // kaminide.sessions (Phase 3): observe + control the host's Projects/Sessions.
  // Wrapped so an API mishap can't abort the rest of activate() (which would
  // unregister every command below it).
  if (kaminide) {
    try {
      // makeSession — create + activate a named session, prove control round-trips.
      context.subscriptions.push(vscode.commands.registerCommand("hello-world.makeSession", async () => {
        const session = await kaminide.sessions.createSession({ name: "From Extension" })
        await vscode.window.showInformationMessage(
          `kaminide created session "${session.name}" (${session.id.slice(0, 8)}) — now active: ${kaminide.sessions.active?.name}`,
        )
      }))
      // listSessions — read the live snapshot.
      context.subscriptions.push(vscode.commands.registerCommand("hello-world.listSessions", async () => {
        const names = kaminide.sessions.all.map((s) => s.name).join(", ")
        await vscode.window.showInformationMessage(`kaminide sessions (${kaminide.sessions.all.length}): ${names}`)
      }))
      // onDidChangeActiveSession — held as a subscription only (event direction
      // proven); the toast on every switch was pure noise (пользователь и так
      // видит активный чат) — removed.
      context.subscriptions.push(kaminide.sessions.onDidChangeActiveSession(() => {}))
    } catch (err) {
      void vscode.window.showWarningMessage(`kaminide API unavailable: ${err.message}`)
    }
  }

  const disposable = vscode.commands.registerCommand("hello-world.sayHello", () => {
    return vscode.window.showInformationMessage("Hello from KaminIDE — extension activate flow works.")
  })
  context.subscriptions.push(disposable)

  // #11: environmentVariableCollection. Prepend a marker to PATH + set a var;
  // open a NEW integrated terminal afterwards and `echo $KAMIN_ENV_DEMO` (or
  // check PATH) shows the mutation reached the shell's spawn env.
  context.subscriptions.push(vscode.commands.registerCommand("hello-world.mutateEnv", () => {
    context.environmentVariableCollection.replace("KAMIN_ENV_DEMO", "hello-from-extension")
    context.environmentVariableCollection.prepend("PATH", "C:\\kamin-demo;")
    return vscode.window.showInformationMessage("Env mutated — open a NEW terminal and run: echo %KAMIN_ENV_DEMO%")
  }))

  // Language-feature demo (B6d): a document formatter that UPPER-CASES the whole
  // buffer. Proves register*Provider flows host→Monaco end-to-end — run
  // "Format Document" and the text uppercases.
  context.subscriptions.push(vscode.languages.registerDocumentFormattingEditProvider("*", {
    provideDocumentFormattingEdits(document) {
      const text = document.getText()
      const fullRange = new vscode.Range(document.positionAt(0), document.positionAt(text.length))
      return [vscode.TextEdit.replace(fullRange, text.toUpperCase())]
    },
  }))

  // Completion demo (#20): rich CompletionItemLabel (detail + description) and a
  // SnippetString insertText (tab-stops). Type "kam" to see "kaminHello" with a
  // "(greeting)" detail + "HelloModule" description; accepting expands the snippet.
  context.subscriptions.push(vscode.languages.registerCompletionItemProvider("*", {
    provideCompletionItems() {
      const item = new vscode.CompletionItem(
        { label: "kaminHello", detail: " (greeting)", description: "HelloModule" },
        vscode.CompletionItemKind.Function,
      )
      item.insertText = new vscode.SnippetString("kaminHello(${1:name})$0")
      return [item]
    },
  }))

  // Language-feature demo (B6e): references / document-highlight / folding.
  // Deterministic fixtures so Playwright can assert the host→Monaco chain:
  //  • references → one Location at line 0 (Find All References peek).
  //  • highlights → the word under the cursor's whole line (occurrence boxes).
  //  • folding   → fold lines 0..1 (a fold arrow in the gutter).
  context.subscriptions.push(vscode.languages.registerReferenceProvider("*", {
    provideReferences(document) {
      return [new vscode.Location(document.uri, new vscode.Range(0, 0, 0, 1))]
    },
  }))
  context.subscriptions.push(vscode.languages.registerDocumentHighlightProvider("*", {
    provideDocumentHighlights(document, position) {
      const line = document.lineAt(position.line)
      return [new vscode.DocumentHighlight(line.range, vscode.DocumentHighlightKind.Write)]
    },
  }))
  context.subscriptions.push(vscode.languages.registerFoldingRangeProvider("*", {
    provideFoldingRanges() {
      // Comment-kind so "Fold All Comments" categorizes it (B6e kind round-trip).
      return [new vscode.FoldingRange(0, 1, vscode.FoldingRangeKind.Comment)]
    },
  }))

  // Language-feature demo (B6f): declaration / type-definition / implementation —
  // all Location-based, each points at a distinct line so peek/navigation is
  // visibly distinguishable. Go to Declaration / Go to Type Definition / Go to
  // Implementation in the editor context menu.
  context.subscriptions.push(vscode.languages.registerDeclarationProvider("*", {
    provideDeclaration(document) {
      return new vscode.Location(document.uri, new vscode.Range(1, 0, 1, 1))
    },
  }))
  context.subscriptions.push(vscode.languages.registerTypeDefinitionProvider("*", {
    provideTypeDefinition(document) {
      return new vscode.Location(document.uri, new vscode.Range(2, 0, 2, 1))
    },
  }))
  context.subscriptions.push(vscode.languages.registerImplementationProvider("*", {
    provideImplementation(document) {
      return new vscode.Location(document.uri, new vscode.Range(3, 0, 3, 1))
    },
  }))

  // Language-feature demo (B6g): signature help — a fixed 2-param signature so
  // typing "(" pops the parameter-hints widget with "helloWorld(name, loud)".
  context.subscriptions.push(vscode.languages.registerSignatureHelpProvider("*", {
    provideSignatureHelp() {
      const sig = new vscode.SignatureInformation("helloWorld(name: string, loud: boolean): string", "Greets the given name.")
      sig.parameters = [new vscode.ParameterInformation("name", "Who to greet"), new vscode.ParameterInformation("loud", "Shout it")]
      const help = new vscode.SignatureHelp()
      help.signatures = [sig]
      help.activeSignature = 0
      help.activeParameter = 0
      return help
    },
  }, "(", ","))

  // Language-feature demo (B6h): document symbols (outline / Ctrl+Shift+O) +
  // a clickable document link. The symbol tree is a module with one child fn.
  context.subscriptions.push(vscode.languages.registerDocumentSymbolProvider("*", {
    provideDocumentSymbols(document) {
      const mod = new vscode.DocumentSymbol("HelloModule", "demo", vscode.SymbolKind.Module,
        new vscode.Range(0, 0, 4, 0), new vscode.Range(0, 0, 0, 11))
      mod.children = [new vscode.DocumentSymbol("greet", "fn", vscode.SymbolKind.Function,
        new vscode.Range(1, 0, 1, 5), new vscode.Range(1, 0, 1, 5))]
      void document
      return [mod]
    },
  }))
  context.subscriptions.push(vscode.languages.registerDocumentLinkProvider("*", {
    provideDocumentLinks() {
      const link = new vscode.DocumentLink(new vscode.Range(0, 0, 0, 4), vscode.Uri.parse("https://example.com/hello"))
      link.tooltip = "Hello link"
      return [link]
    },
  }))

  // Language-feature demo (B6i): inlay hints + selection ranges. The hint renders
  // ": int" inline at line 0 col 6; selection-expand grows the word → line.
  context.subscriptions.push(vscode.languages.registerInlayHintsProvider("*", {
    provideInlayHints() {
      const hint = new vscode.InlayHint(new vscode.Position(0, 6), ": int", vscode.InlayHintKind.Type)
      hint.paddingLeft = true
      return [hint]
    },
  }))
  context.subscriptions.push(vscode.languages.registerSelectionRangeProvider("*", {
    provideSelectionRanges(document, positions) {
      void document
      // For each cursor: a word-sized range nested inside its whole line.
      return positions.map((p) => new vscode.SelectionRange(
        new vscode.Range(p.line, 0, p.line, 4),
        new vscode.SelectionRange(new vscode.Range(p.line, 0, p.line, 16)),
      ))
    },
  }))

  // Language-feature demo (B6j): a code lens above line 0 wired to the existing
  // sayHello command — clicking it runs the command (lens-click → host bridge).
  context.subscriptions.push(vscode.languages.registerCodeLensProvider("*", {
    provideCodeLenses() {
      return [new vscode.CodeLens(new vscode.Range(0, 0, 0, 0), {
        command: "hello-world.sayHello", title: "▶ Say Hello (lens)",
      })]
    },
  }))

  // Language-feature demo (B6k): document colors — paints a red swatch over the
  // first 4 chars of line 0, with one "Crimson" presentation in the picker.
  context.subscriptions.push(vscode.languages.registerColorProvider("*", {
    provideDocumentColors() {
      return [new vscode.ColorInformation(new vscode.Range(0, 0, 0, 4), new vscode.Color(1, 0, 0, 1))]
    },
    provideColorPresentations(color, context) {
      void color; void context
      return [new vscode.ColorPresentation("Crimson")]
    },
  }))

  // Language-feature demo (B6l): rename — F2 replaces the first 6 chars of line 0
  // with the new name (proves WorkspaceEdit carries edits, not the old no-op stub).
  context.subscriptions.push(vscode.languages.registerRenameProvider("*", {
    provideRenameEdits(document, position, newName) {
      void position
      const edit = new vscode.WorkspaceEdit()
      edit.replace(document.uri, new vscode.Range(0, 0, 0, 6), newName)
      return edit
    },
  }))

  // Language-feature demo (B6m): code action — a quickfix lightbulb that rewrites
  // the first 6 chars of line 0 to "FIXED" (proves edit-carrying actions apply).
  context.subscriptions.push(vscode.languages.registerCodeActionsProvider("*", {
    provideCodeActions(document) {
      const action = new vscode.CodeAction("Replace with FIXED", vscode.CodeActionKind.QuickFix)
      action.isPreferred = true
      const edit = new vscode.WorkspaceEdit()
      edit.replace(document.uri, new vscode.Range(0, 0, 0, 6), "FIXED")
      action.edit = edit
      return [action]
    },
  }))

  // Language-feature demo: workspace symbols (Ctrl+T). Returns a symbol named
  // after the query so the picker visibly reflects the provider chain.
  context.subscriptions.push(vscode.languages.registerWorkspaceSymbolProvider({
    provideWorkspaceSymbols(query) {
      const name = query ? `helloSym:${query}` : "helloWorldSymbol"
      // #21: the second symbol is returned WITHOUT a range (location has only a
      // uri) to exercise resolveWorkspaceSymbol — without resolution it'd drop.
      return [
        new vscode.SymbolInformation(name, vscode.SymbolKind.Function, "HelloModule",
          new vscode.Location(vscode.Uri.file("C:/hello/world.ts"), new vscode.Range(0, 0, 0, 5))),
        new vscode.SymbolInformation("helloLazySym", vscode.SymbolKind.Variable, "HelloModule",
          new vscode.Location(vscode.Uri.file("C:/hello/lazy.ts"), undefined)),
      ]
    },
    resolveWorkspaceSymbol(symbol) {
      // Fill in the range on demand (here: always line 7).
      symbol.location.range = new vscode.Range(7, 0, 7, 12)
      return symbol
    },
  }))

  // Language-feature demo (B6n): semantic tokens — marks the first 6 chars of
  // line 0 as a "keyword" (its own legend; the host remaps to the standard one).
  const semanticLegend = new vscode.SemanticTokensLegend(["keyword"], [])
  context.subscriptions.push(vscode.languages.registerDocumentSemanticTokensProvider("*", {
    provideDocumentSemanticTokens(document) {
      void document
      // [deltaLine, deltaStartChar, length, tokenType=0 (keyword), modifiers=0]
      return new vscode.SemanticTokens(new Uint32Array([0, 0, 6, 0, 0]))
    },
  }, semanticLegend))

  // Crash-containment demo: schedule an UNCAUGHT async throw. The ext-host must
  // NOT die — it contains the error (toast + System log) and keeps extensions alive.
  context.subscriptions.push(vscode.commands.registerCommand("hello-world.crashTest", () => {
    setTimeout(() => { throw new Error("intentional async crash from hello-world (containment test)") }, 50)
    return vscode.window.showInformationMessage("Scheduled an uncaught crash in 50ms — the host should survive.")
  }))

  // Native-crash isolation demo: process.abort() raises SIGABRT — a HARD kill
  // that bypasses uncaughtException, exactly like a native addon segfault. Only
  // the forked ext-host child dies; the PARENT (terminal/fs/editor) must survive
  // and respawn the child. No-op-ish in the in-process build (would kill the host).
  context.subscriptions.push(vscode.commands.registerCommand("hello-world.hardCrash", () => {
    setTimeout(() => { process.abort() }, 50)
    return vscode.window.showInformationMessage("Hard-crashing the ext-host in 50ms — the terminal should survive.")
  }))

  // Exercises window.showQuickPick (B8): pick a fruit, then echo it back via
  // showInformationMessage. Proves the QuickPick overlay + index round-trip.
  const pick = vscode.commands.registerCommand("hello-world.pickFruit", async () => {
    const choice = await vscode.window.showQuickPick(
      ["Apple", "Banana", "Cherry", "Durian"],
      { placeHolder: "Pick a fruit" },
    )
    if (choice) await vscode.window.showInformationMessage(`You picked: ${choice}`)
  })
  context.subscriptions.push(pick)

  // canPickMany + a Separator divider + a pre-`picked` item — exercises the
  // B8 review fixes (separators non-interactive, picked preselection, []-on-OK).
  const toppings = vscode.commands.registerCommand("hello-world.pickToppings", async () => {
    const chosen = await vscode.window.showQuickPick(
      [
        { label: "Cheese", picked: true },
        { label: "Pepperoni" },
        { label: "Veggies", kind: -1 },
        { label: "Tomato" },
        { label: "Onion" },
      ],
      { canPickMany: true, placeHolder: "Choose toppings" },
    )
    if (chosen) await vscode.window.showInformationMessage(`Toppings: ${chosen.map((c) => c.label).join(", ") || "(none)"}`)
  })
  context.subscriptions.push(toppings)

  // Status-bar item (B8) — visible immediately on activation; clicking it runs
  // Say Hello. Exercises createStatusBarItem + $(icon) text + command.
  const status = vscode.window.createStatusBarItem(1 /* Left */, 100)
  status.text = "$(rocket) Hello"
  status.tooltip = "hello-world status item — click to greet"
  status.command = "hello-world.sayHello"
  status.show()
  context.subscriptions.push(status)

  // B5b-2c: setDecorations — whole-line highlight on lines 1 & 3 of the active editor.
  const decorate = vscode.commands.registerCommand("hello-world.decorate", () => {
    const editor = vscode.window.activeTextEditor
    if (!editor) { void vscode.window.showInformationMessage("Open a file first to decorate."); return }
    const deco = vscode.window.createTextEditorDecorationType({ backgroundColor: "rgba(255,210,0,0.4)", isWholeLine: true })
    editor.setDecorations(deco, [
      { start: { line: 0, character: 0 }, end: { line: 0, character: 0 } },
      { start: { line: 2, character: 0 }, end: { line: 2, character: 0 } },
    ])
  })
  context.subscriptions.push(decorate)

  // B5b-2c: insertSnippet at the cursor (tab-stops $1/$0).
  const snippet = vscode.commands.registerCommand("hello-world.snippet", async () => {
    const editor = vscode.window.activeTextEditor
    if (!editor) { void vscode.window.showInformationMessage("Open a file first."); return }
    await editor.insertSnippet("/* KaminIDE $1 snippet $0 */")
  })
  context.subscriptions.push(snippet)

  // B5b-2c: selection write — select all of line 1 of the active editor.
  const select = vscode.commands.registerCommand("hello-world.selectLine", () => {
    const editor = vscode.window.activeTextEditor
    if (!editor) { void vscode.window.showInformationMessage("Open a file first."); return }
    editor.selection = new vscode.Selection(new vscode.Position(0, 0), new vscode.Position(0, 8))
  })
  context.subscriptions.push(select)

  // B9: globalState persistence across restarts. Each invocation bumps a
  // counter stored in context.globalState and echoes it; the value survives a
  // full app restart (proves real on-disk persistence, not in-memory).
  const bump = vscode.commands.registerCommand("hello-world.bumpCount", async () => {
    const n = (context.globalState.get("count") || 0) + 1
    await context.globalState.update("count", n)
    await vscode.window.showInformationMessage("globalState count: " + n)
  })
  context.subscriptions.push(bump)

  // B9 hardening: secrets stash + read-back. Proves the DPAPI relay round-trips
  // (the value on disk in secrets.json is `dpapi:<hex>` ciphertext, not the
  // plaintext typed here) and survives a restart.
  const stashSecret = vscode.commands.registerCommand("hello-world.stashSecret", async () => {
    const value = await vscode.window.showInputBox({ prompt: "Secret to stash (DPAPI-sealed)", value: "hunter2" })
    if (value === undefined) return
    await context.secrets.store("demo", value)
    const read = await context.secrets.get("demo")
    await vscode.window.showInformationMessage(`secrets round-trip: stored & read back "${read}"`)
  })
  context.subscriptions.push(stashSecret)

  // Problems panel: publish demo diagnostics on the active file (an error + a
  // warning) via a real DiagnosticCollection — proves the host→renderer
  // kamin:diag:set path feeds the Problems aggregate store, not just Monaco.
  const diagCollection = vscode.languages.createDiagnosticCollection("hello-world")
  context.subscriptions.push(diagCollection)
  const diagnose = vscode.commands.registerCommand("hello-world.diagnose", () => {
    const editor = vscode.window.activeTextEditor
    if (!editor) { void vscode.window.showInformationMessage("Open a file first to add demo diagnostics."); return }
    diagCollection.set(editor.document.uri, [
      new vscode.Diagnostic(new vscode.Range(0, 0, 0, 5), "Demo error from hello-world", vscode.DiagnosticSeverity.Error),
      new vscode.Diagnostic(new vscode.Range(2, 0, 2, 4), "Demo warning from hello-world", vscode.DiagnosticSeverity.Warning),
    ])
    void vscode.window.showInformationMessage("Published 2 demo diagnostics — see the Problems panel.")
  })
  context.subscriptions.push(diagnose)

  // createTreeView: a 2-level tree in the sidebar with live display meta. The
  // root groups are STABLE references so treeView.reveal(group) can identity-
  // match a materialized node. Leaves carry a command (fires on click); refresh
  // bumps onDidChangeTreeData + updates message/badge and reveals the first group.
  const treeEmitter = new vscode.EventEmitter()
  let refreshN = 0
  const fruits = { kind: "group", label: "Fruits" }
  const veggies = { kind: "group", label: "Veggies" }
  // Stable leaf refs so getParent + reveal can identity-match a deep node.
  const leaves = {
    Fruits: [{ kind: "leaf", label: "Fruits · one", parent: fruits }, { kind: "leaf", label: "Fruits · two", parent: fruits }],
    Veggies: [{ kind: "leaf", label: "Veggies · one", parent: veggies }, { kind: "leaf", label: "Veggies · two", parent: veggies }],
  }
  const checkedLeaves = new Set() // leaf labels the user has ticked
  const treeProvider = {
    onDidChangeTreeData: treeEmitter.event,
    getChildren(el) {
      if (!el) return [fruits, veggies]
      if (el.kind === "group") return leaves[el.label]
      return []
    },
    getParent(el) {
      return el.kind === "leaf" ? el.parent : undefined
    },
    getTreeItem(el) {
      const collapsible = el.kind === "group" ? vscode.TreeItemCollapsibleState.Collapsed : vscode.TreeItemCollapsibleState.None
      const item = new vscode.TreeItem(el.label, collapsible)
      item.iconPath = new vscode.ThemeIcon(el.kind === "group" ? "symbol-folder" : "circle-filled")
      item.tooltip = `Tree node: ${el.label}`
      if (el.kind === "leaf") {
        item.command = { command: "hello-world.sayHello", title: "Say Hello" }
        // Checkbox per leaf — state tracked in checkedLeaves, toggled below.
        item.checkboxState = checkedLeaves.has(el.label)
          ? vscode.TreeItemCheckboxState.Checked
          : vscode.TreeItemCheckboxState.Unchecked
      }
      return item
    },
  }
  // Drag-and-drop: drag a leaf onto a group (or another leaf) to move it there.
  const TREE_MIME = "application/vnd.code.tree.hellotree"
  const dragAndDropController = {
    dropMimeTypes: [TREE_MIME],
    dragMimeTypes: [TREE_MIME],
    handleDrag(source, dataTransfer) {
      dataTransfer.set(TREE_MIME, new vscode.DataTransferItem(source))
    },
    handleDrop(target, dataTransfer) {
      const item = dataTransfer.get(TREE_MIME)
      if (!item) return
      const dragged = item.value
      const destGroup = target ? (target.kind === "group" ? target : target.parent) : fruits
      for (const leaf of dragged) {
        for (const g of [fruits, veggies]) {
          const i = leaves[g.label].indexOf(leaf)
          if (i >= 0) leaves[g.label].splice(i, 1)
        }
        leaf.parent = destGroup
        leaf.label = `${destGroup.label} · ${leaf.label.split(" · ")[1]}`
        leaves[destGroup.label].push(leaf)
      }
      helloTree.message = `Moved ${dragged.length} → ${destGroup.label}`
      treeEmitter.fire()
    },
  }
  const helloTree = vscode.window.createTreeView("helloTree", { treeDataProvider: treeProvider, dragAndDropController })
  helloTree.message = "Created via createTreeView"
  helloTree.description = "demo"
  helloTree.badge = { value: 2, tooltip: "2 groups" }
  // onDidChangeCheckboxState: update the model + refresh so the tick re-renders.
  helloTree.onDidChangeCheckboxState((e) => {
    for (const [el, state] of e.items) {
      if (state === vscode.TreeItemCheckboxState.Checked) checkedLeaves.add(el.label)
      else checkedLeaves.delete(el.label)
    }
    helloTree.message = `${checkedLeaves.size} leaf(s) checked`
    treeEmitter.fire()
  })
  context.subscriptions.push(helloTree)

  const refreshTree = vscode.commands.registerCommand("hello-world.refreshTree", () => {
    refreshN++
    treeEmitter.fire()
    helloTree.message = `Refreshed ${refreshN}×`
    helloTree.badge = { value: refreshN, tooltip: `refreshed ${refreshN} times` }
    helloTree.reveal(fruits, { select: true })
  })
  context.subscriptions.push(refreshTree)

  // Deep reveal: collapse the tree, then reveal a leaf — getParent lets the view
  // expand the leaf's ancestor (Veggies) and select+focus it from collapsed.
  const revealLeaf = vscode.commands.registerCommand("hello-world.revealLeaf", () => {
    helloTree.reveal(leaves.Veggies[1], { select: true, focus: true, expand: false })
  })
  context.subscriptions.push(revealLeaf)

  // FileDecorationProvider: badge files in the explorer by extension (a stand-in
  // for git status). .py → "P" (modified colour), .md → "M" (untracked colour).
  const decoProvider = {
    provideFileDecoration(uri) {
      if (uri.fsPath.endsWith(".py")) return new vscode.FileDecoration("P", "Python (hello-world)", new vscode.ThemeColor("gitDecoration.modifiedResourceForeground"))
      if (uri.fsPath.endsWith(".md")) return new vscode.FileDecoration("M", "Markdown (hello-world)", new vscode.ThemeColor("gitDecoration.untrackedResourceForeground"))
      return undefined
    },
  }
  context.subscriptions.push(vscode.window.registerFileDecorationProvider(decoProvider))

  // B7c-2c + WebviewPanelSerializer: a webview panel with retainContextWhenHidden
  // and a persisted "opens" counter. The live interval proves retain across tab
  // switches; getState/setState + the serializer prove cross-restart restore —
  // the count survives a full app restart (panel is re-created + deserialized).
  const panelHtml = () =>
    '<!DOCTYPE html><html><body style="font-family:sans-serif;color:#ddd;padding:14px">'
    + '<h3>Retained Panel</h3>'
    + '<p>opens (persisted): <b id="o">?</b></p>'
    + '<p>alive seconds: <b id="t">0</b></p>'
    + '<p style="opacity:.6">Retained across tab switches; opens survives an app restart.</p>'
    + '<script>(function(){var a=acquireVsCodeApi();var s=a.getState()||{opens:0};s.opens++;a.setState(s);'
    + 'document.getElementById("o").textContent=String(s.opens);'
    + 'var n=0;setInterval(function(){n++;document.getElementById("t").textContent=String(n);},1000);})();</script>'
    + '</body></html>'
  const openPanel = vscode.commands.registerCommand("hello-world.openPanel", () => {
    const panel = vscode.window.createWebviewPanel("helloPanel", "Retained Panel", 1, { enableScripts: true, retainContextWhenHidden: true })
    panel.webview.html = panelHtml()
  })
  context.subscriptions.push(openPanel)

  // Restore "helloPanel" panels open at the last shutdown (cross-restart).
  context.subscriptions.push(vscode.window.registerWebviewPanelSerializer("helloPanel", {
    deserializeWebviewPanel(panel) {
      panel.webview.options = { enableScripts: true }
      panel.webview.html = panelHtml()
    },
  }))

  // B7b: a sidebar webview view. resolveWebviewView runs when the "Hello Panel"
  // activity is first shown; it sets the html and listens for iframe messages.
  const viewProvider = {
    resolveWebviewView(view) {
      view.webview.options = { enableScripts: true }
      // B7c-2: load a bundled asset through asWebviewUri (served from the
      // extension dir, the default localResourceRoot) to prove resource serving.
      const logo = view.webview.asWebviewUri(vscode.Uri.file(context.extensionPath + "/media/logo.svg"))
      view.webview.html =
        '<!DOCTYPE html><html><body style="font-family:sans-serif;color:#ddd;padding:10px">'
        + '<h3 id="h">Hello WebviewView</h3>'
        + '<img id="logo" src="' + logo.toString() + '" width="48" height="48" alt="logo">'
        + '<button onclick="acquireVsCodeApi().postMessage({ping:1})">ping host</button>'
        // B7c-2b: getState/setState round-trip — increments a persisted counter on
        // every (re)serve, proving state survives the view being hidden + remounted.
        + '<p>view loads: <b id="loads">?</b></p>'
        + '<script>(function(){var a=acquireVsCodeApi();var s=a.getState()||{loads:0};s.loads++;a.setState(s);'
        + 'document.getElementById("loads").textContent=String(s.loads);})();</script>'
        + '</body></html>'
      view.webview.onDidReceiveMessage((m) => {
        void vscode.window.showInformationMessage("WebviewView says: " + JSON.stringify(m))
      })
    },
  }
  context.subscriptions.push(vscode.window.registerWebviewViewProvider("helloView", viewProvider))
}

function deactivate() {}

module.exports = { activate, deactivate }
