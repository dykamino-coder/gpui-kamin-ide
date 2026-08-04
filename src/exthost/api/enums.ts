// All `vscode` enums. d.ts emits TS enums (forward + reverse mapping);
// `buildEnum` reproduces that shape so `MyEnum[1] === "First"` works.
//
// Names and numeric values are pinned to the d.ts §locked-spec values —
// extensions compare against literal numbers and string keys.

function buildEnum<K extends string>(members: Record<K, number>): Readonly<Record<K, number>> & Readonly<Record<number, K>> {
  const obj: Record<string | number, string | number> = { ...members }
  for (const [k, v] of Object.entries(members)) obj[v as number] = k
  return Object.freeze(obj) as never
}

export const StatusBarAlignment = buildEnum({ Left: 1, Right: 2 })
export const ViewColumn = buildEnum({ Active: -1, Beside: -2, One: 1, Two: 2, Three: 3, Four: 4, Five: 5, Six: 6, Seven: 7, Eight: 8, Nine: 9 })
export const ConfigurationTarget = buildEnum({ Global: 1, Workspace: 2, WorkspaceFolder: 3 })
export const ExtensionMode = buildEnum({ Production: 1, Development: 2, Test: 3 })
export const ExtensionKind = buildEnum({ UI: 1, Workspace: 2 })
export const EndOfLine = buildEnum({ LF: 1, CRLF: 2 })
export const DiagnosticSeverity = buildEnum({ Error: 0, Warning: 1, Information: 2, Hint: 3 })
export const DiagnosticTag = buildEnum({ Unnecessary: 1, Deprecated: 2 })
export const SymbolKind = buildEnum({
  File: 0, Module: 1, Namespace: 2, Package: 3, Class: 4, Method: 5,
  Property: 6, Field: 7, Constructor: 8, Enum: 9, Interface: 10,
  Function: 11, Variable: 12, Constant: 13, String: 14, Number: 15,
  Boolean: 16, Array: 17, Object: 18, Key: 19, Null: 20,
  EnumMember: 21, Struct: 22, Event: 23, Operator: 24, TypeParameter: 25,
})
export const SymbolTag = buildEnum({ Deprecated: 1 })
export const UIKind = buildEnum({ Desktop: 1, Web: 2 })
export const FileType = buildEnum({ Unknown: 0, File: 1, Directory: 2, SymbolicLink: 64 })
export const FileChangeType = buildEnum({ Changed: 1, Created: 2, Deleted: 3 })
export const TextDocumentSaveReason = buildEnum({ Manual: 1, AfterDelay: 2, FocusOut: 3 })
export const TextEditorRevealType = buildEnum({ Default: 0, InCenter: 1, InCenterIfOutsideViewport: 2, AtTop: 3 })
export const TextEditorCursorStyle = buildEnum({ Line: 1, Block: 2, Underline: 3, LineThin: 4, BlockOutline: 5, UnderlineThin: 6 })
export const TextEditorLineNumbersStyle = buildEnum({ Off: 0, On: 1, Relative: 2 })
export const TextEditorSelectionChangeKind = buildEnum({ Keyboard: 1, Mouse: 2, Command: 3 })
export const OverviewRulerLane = buildEnum({ Left: 1, Center: 2, Right: 4, Full: 7 })
export const DecorationRangeBehavior = buildEnum({ OpenOpen: 0, ClosedClosed: 1, OpenClosed: 2, ClosedOpen: 3 })
export const CompletionItemKind = buildEnum({
  Text: 0, Method: 1, Function: 2, Constructor: 3, Field: 4, Variable: 5,
  Class: 6, Interface: 7, Module: 8, Property: 9, Unit: 10, Value: 11,
  Enum: 12, Keyword: 13, Snippet: 14, Color: 15, File: 16, Reference: 17,
  Folder: 18, EnumMember: 19, Constant: 20, Struct: 21, Event: 22,
  Operator: 23, TypeParameter: 24, User: 25, Issue: 26,
})
export const CompletionTriggerKind = buildEnum({ Invoke: 0, TriggerCharacter: 1, TriggerForIncompleteCompletions: 2 })
export const CompletionItemTag = buildEnum({ Deprecated: 1 })
export const SignatureHelpTriggerKind = buildEnum({ Invoke: 1, TriggerCharacter: 2, ContentChange: 3 })
export const DocumentHighlightKind = buildEnum({ Text: 0, Read: 1, Write: 2 })
export const InlayHintKind = buildEnum({ Type: 1, Parameter: 2 })
export const FoldingRangeKind = buildEnum({ Comment: 1, Imports: 2, Region: 3 })
export const LogLevel = buildEnum({ Off: 0, Trace: 1, Debug: 2, Info: 3, Warning: 4, Error: 5 })
export const DebugConsoleMode = buildEnum({ Separate: 0, MergeWithParent: 1 })
export const DebugConfigurationProviderTriggerKind = buildEnum({ Initial: 1, Dynamic: 2 })
export const NotebookCellKind = buildEnum({ Markup: 1, Code: 2 })
export const NotebookControllerAffinity = buildEnum({ Default: 1, Preferred: 2 })
export const TestRunProfileKind = buildEnum({ Run: 1, Debug: 2, Coverage: 3 })
export const QuickPickItemKind = buildEnum({ Separator: -1, Default: 0 })
export const InputBoxValidationSeverity = buildEnum({ Info: 1, Warning: 2, Error: 3 })
export const TextDocumentChangeReason = buildEnum({ Undo: 1, Redo: 2 })
export const SourceControlInputBoxValidationType = buildEnum({ Error: 0, Warning: 1, Information: 2 })
export const CommentMode = buildEnum({ Editing: 0, Preview: 1 })
export const CommentThreadCollapsibleState = buildEnum({ Collapsed: 0, Expanded: 1 })
export const CommentThreadState = buildEnum({ Unresolved: 0, Resolved: 1 })
export const TaskRevealKind = buildEnum({ Always: 1, Silent: 2, Never: 3 })
export const TaskPanelKind = buildEnum({ Shared: 1, Dedicated: 2, New: 3 })
export const TaskScope = buildEnum({ Global: 1, Workspace: 2 })
export const ShellQuoting = buildEnum({ Escape: 1, Strong: 2, Weak: 3 })
export const LanguageStatusSeverity = buildEnum({ Information: 0, Warning: 1, Error: 2 })
export const ColorThemeKind = buildEnum({ Light: 1, Dark: 2, HighContrast: 3, HighContrastLight: 4 })
export const ProgressLocation = buildEnum({ SourceControl: 1, Window: 10, Notification: 15 })
export const TreeItemCollapsibleState = buildEnum({ None: 0, Collapsed: 1, Expanded: 2 })
export const TreeItemCheckboxState = buildEnum({ Unchecked: 0, Checked: 1 })
export const EnvironmentVariableMutatorType = buildEnum({ Replace: 1, Append: 2, Prepend: 3 })
export const ExtensionRuntime = buildEnum({ Node: 1, Webworker: 2 })
export const IndentAction = buildEnum({ None: 0, Indent: 1, IndentOutdent: 2, Outdent: 3 })
// 1.93+ — where a QuickInput button renders (gitlens reads `.Inline`).
export const QuickInputButtonLocation = buildEnum({ Title: 1, Inline: 2 })
// What triggered a code-action request (golang.Go's LSP client maps `.Invoke`).
export const CodeActionTriggerKind = buildEnum({ Invoke: 1, Automatic: 2 })
