import { AsyncLocalStorage } from 'node:async_hooks'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

/** Browser-owned effects. The caller must implement these; no success stubs. */
export interface WorkspaceEnvironment {
  clipboard: { readText(): Promise<string>; writeText(text: string): Promise<void> }
  message(level: 'info' | 'warning' | 'error', text: string, actions: string[]): Promise<string | undefined>
  pickPaths(options: Record<string, unknown>): Promise<string[] | undefined>
  savePath(options: Record<string, unknown>): Promise<string | undefined>
  openExternal(url: string): Promise<boolean>
  openDocument(file: string): Promise<unknown>
  reveal(file: string): Promise<unknown>
  preview?: {
    mount(input: Record<string, unknown>): unknown
    update(props: unknown): unknown
    unmount(): unknown
  }
}

let root: string | undefined
let environment: WorkspaceEnvironment | undefined
const requests = new AsyncLocalStorage<WorkspaceEnvironment>()

export function configureEnvironment(workspaceRoot: string, services: WorkspaceEnvironment): void {
  root = path.resolve(workspaceRoot)
  environment = services
}

export function withEnvironment<T>(services: WorkspaceEnvironment, run: () => T): T {
  return requests.run(services, run)
}

function services(): WorkspaceEnvironment {
  const value = requests.getStore() ?? environment
  if (!value) throw new Error('Workspace environment is not configured')
  return value
}

// Explicit build-time substitute for the VS Code services used by BridgeHost.
// Missing imports fail the bundle; unimplemented commands reject at runtime.
export const Uri = {
  file(file: string) {
    const fsPath = path.resolve(file)
    return { fsPath, toString: () => pathToFileURL(fsPath).href }
  },
  parse(value: string) {
    const url = new URL(value)
    return { fsPath: url.protocol === 'file:' ? fileURLToPath(url) : '', toString: () => url.href }
  },
}

export const workspace = {
  get workspaceFolders() {
    if (!root) throw new Error('Workspace root is not configured')
    return [{ uri: Uri.file(root) }]
  },
}

export class Position {
  constructor(readonly line: number, readonly character: number) {}
}

// Headless mode has no VSIX-native diagnostic publishers. The shared LSP tools
// merge this empty native-provider set with the real pluginLspManager results.
export const languages = { getDiagnostics: (_uri: unknown): unknown[] => [] }

export const env = {
  clipboard: {
    readText: () => services().clipboard.readText(),
    writeText: (text: string) => services().clipboard.writeText(text),
  },
  openExternal: (uri: { toString(): string }) => services().openExternal(uri.toString()),
}

export const window = {
  showInformationMessage: (text: string, ...actions: string[]) => services().message('info', text, actions),
  showWarningMessage: (text: string, ...actions: string[]) => services().message('warning', text, actions),
  showErrorMessage: (text: string, ...actions: string[]) => services().message('error', text, actions),
  async showOpenDialog(options: Record<string, unknown>) {
    return (await services().pickPaths(options))?.map(file => Uri.file(file))
  },
  async showSaveDialog(options: Record<string, unknown>) {
    const file = await services().savePath(options)
    return file ? Uri.file(file) : undefined
  },
  showTextDocument: (uri: { fsPath: string }) => services().openDocument(uri.fsPath),
}

export const commands = {
  async executeCommand(command: string, input: unknown, ..._args: unknown[]) {
    if (command === 'revealFileInOS') return services().reveal((input as { fsPath: string }).fsPath)
    if (command.startsWith('kaminBridge.preview.')) {
      const preview = services().preview
      if (!preview) throw new Error('Preview requires the browser workspace host')
      if (command === 'kaminBridge.preview.mount') return preview.mount(input as Record<string, unknown>)
      if (command === 'kaminBridge.preview.update') return preview.update(input)
      if (command === 'kaminBridge.preview.unmount') return preview.unmount()
    }
    throw new Error(`Workspace command is unavailable: ${command}`)
  },
}
