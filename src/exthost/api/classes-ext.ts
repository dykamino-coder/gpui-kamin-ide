// "Extended" classes — tasks, debug, terminal, tabs, notebooks, tests.
// Lower-frequency surface but several popular extensions touch it
// (vscode-maven, vscode-java-debug, golang.Go, GitLens).

export class Task {
  constructor(
    public definition: unknown, public scope: unknown, public name: string,
    public source: string, public execution?: unknown, public problemMatchers: string[] = [],
  ) {}
}
export class ProcessExecution { constructor(public process: string, public args: string[] = [], public options?: unknown) {} }
export class ShellExecution { constructor(public command: string, public args: string[] = [], public options?: unknown) {} }
export class CustomExecution { constructor(public callback: unknown) {} }

export class Breakpoint {}
export class SourceBreakpoint {
  constructor(public location: unknown, public enabled?: boolean, public condition?: string, public hitCondition?: string, public logMessage?: string) {}
}
export class FunctionBreakpoint {}
export class DebugAdapterExecutable { constructor(public command: string, public args?: string[], public options?: unknown) {} }
export class DebugAdapterServer { constructor(public port: number, public host?: string) {} }
export class DebugAdapterNamedPipeServer {}
export class DebugAdapterInlineImplementation {}
export class EvaluatableExpression { constructor(public range: unknown, public expression?: string) {} }

export class TerminalLink {
  constructor(public startIndex: number, public length: number, public tooltip?: string) {}
}
export class TerminalProfile { constructor(public options: unknown) {} }

export class TabInputText { constructor(public uri: unknown) {} }
export class TabInputTextDiff { constructor(public original: unknown, public modified: unknown) {} }
export class TabInputCustom {}
export class TabInputWebview {}
export class TabInputNotebook {}
export class TabInputNotebookDiff {}
export class TabInputTerminal {}

export class NotebookData { constructor(public cells: unknown[] = []) {} }
export class NotebookCellData { constructor(public kind: number, public value: string, public languageId: string) {} }
export class NotebookCellOutput { constructor(public items: unknown[] = [], public metadata?: unknown) {} }

class NotebookCellOutputItemCtor { constructor(public data: Uint8Array, public mime: string) {} }
const enc = new TextEncoder()
export const NotebookCellOutputItem = Object.assign(NotebookCellOutputItemCtor, {
  text: (v: unknown) => ({ data: enc.encode(String(v)), mime: "text/plain" }),
  json: (v: unknown) => ({ data: enc.encode(JSON.stringify(v)), mime: "application/json" }),
  stdout: (v: unknown) => ({ data: enc.encode(String(v)), mime: "application/vnd.code.notebook.stdout" }),
  stderr: (v: unknown) => ({ data: enc.encode(String(v)), mime: "application/vnd.code.notebook.stderr" }),
  error: () => ({ data: new Uint8Array(), mime: "application/vnd.code.notebook.error" }),
})
export class NotebookRange { constructor(public start: number, public end: number) {} }

class NotebookEditCtor {}
export const NotebookEdit = Object.assign(NotebookEditCtor, {
  replaceCells: () => ({}), insertCells: () => ({}), deleteCells: () => ({}),
  updateCellMetadata: () => ({}), updateNotebookMetadata: () => ({}),
})

export class TestController {}
class TestMessageCtor { constructor(public message: string) {} }
export const TestMessage = Object.assign(TestMessageCtor, {
  diff: (m: string, e: string, a: string) => ({ message: m, expected: e, actual: a }),
})
export class TestTag { constructor(public id: string) {} }
export class TestRunRequest {
  constructor(public include?: unknown[], public exclude?: unknown[], public profile?: unknown) {}
}
export class FileCoverage {}
export class StatementCoverage {}
export class BranchCoverage {}
export class DeclarationCoverage {}
export class TestCoverageCount {}
